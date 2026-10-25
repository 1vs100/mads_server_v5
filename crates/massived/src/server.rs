//! TCP listeners and connection handling.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinSet;
use tracing::{Instrument, debug, error, info, info_span, warn};

use crate::capture::{Capture, Direction};
use crate::config::{Advertise, Config};
use crate::handler::{Handler, SetupError};
use crate::http;

#[derive(Debug, Error)]
pub enum ServeError {
    #[error(transparent)]
    Setup(#[from] SetupError),
    #[error(transparent)]
    Capture(#[from] crate::capture::CaptureError),
    #[error("cannot listen on {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        source: std::io::Error,
    },
    #[error("cannot serve on listener {addr:?}: {source}")]
    Adopt {
        addr: Option<SocketAddr>,
        source: std::io::Error,
    },
    #[error("no listen address given")]
    NoListener,
}

#[derive(Debug)]
struct Shared {
    handler: Handler,
    capture: Option<Capture>,
    idle_timeout: Duration,
    close_after_response: bool,
    next_connection: AtomicU64,
}

/// Listeners are bound; [`Server::serve`] starts accepting.
#[derive(Debug)]
pub struct Server {
    shared: Arc<Shared>,
    listeners: Vec<TcpListener>,
    capture_task: Option<tokio::task::JoinHandle<()>>,
}

impl Server {
    pub async fn bind(
        config: &Config,
        listen: &[SocketAddr],
        advertise: &Advertise,
    ) -> Result<Self, ServeError> {
        let mut listeners = Vec::new();
        for &addr in listen {
            let listener = TcpListener::bind(addr)
                .await
                .map_err(|source| ServeError::Bind { addr, source })?;
            listeners.push(listener);
        }
        let handler = Handler::new(config, advertise)?;
        Self::with_listeners(config, handler, listeners).await
    }

    /// Serves on listeners bound by the caller, with content loaded up front
    /// by [`Handler::new`]. Must run inside a Tokio runtime.
    pub async fn with_std_listeners(
        config: &Config,
        handler: Handler,
        listeners: Vec<std::net::TcpListener>,
    ) -> Result<Self, ServeError> {
        let listeners = listeners
            .into_iter()
            .map(|listener| {
                let addr = listener.local_addr().ok();
                listener
                    .set_nonblocking(true)
                    .and_then(|()| TcpListener::from_std(listener))
                    .map_err(|source| ServeError::Adopt { addr, source })
            })
            .collect::<Result<Vec<_>, _>>()?;
        Self::with_listeners(config, handler, listeners).await
    }

    async fn with_listeners(
        config: &Config,
        handler: Handler,
        listeners: Vec<TcpListener>,
    ) -> Result<Self, ServeError> {
        if listeners.is_empty() {
            return Err(ServeError::NoListener);
        }
        info!(
            zones = handler.content().zone_count(),
            announced_zones = handler.content().announced_zones().len(),
            assets = handler.content().asset_count(),
            "content_loaded"
        );
        let (capture, capture_task) = match &config.capture {
            Some(c) => {
                let (capture, task) = Capture::open(&c.path).await?;
                info!(path = %c.path.display(), "capture_enabled");
                (Some(capture), Some(task))
            }
            None => (None, None),
        };
        for listener in &listeners {
            if let Ok(addr) = listener.local_addr() {
                info!(%addr, "listening");
            }
        }
        Ok(Self {
            shared: Arc::new(Shared {
                handler,
                capture,
                idle_timeout: Duration::from_secs(config.server.idle_timeout_seconds),
                close_after_response: config.server.close_after_response,
                next_connection: AtomicU64::new(1),
            }),
            listeners,
            capture_task,
        })
    }

    #[must_use]
    pub fn local_addrs(&self) -> Vec<SocketAddr> {
        self.listeners
            .iter()
            .filter_map(|l| l.local_addr().ok())
            .collect()
    }

    /// Accepts connections until `shutdown` completes, then waits for open
    /// connections and flushes the capture.
    pub async fn serve(self, shutdown: impl Future<Output = ()>) {
        let mut accept_tasks = JoinSet::new();
        let (conn_tx, mut conn_rx) = tokio::sync::mpsc::channel::<(TcpStream, SocketAddr)>(64);
        for listener in self.listeners {
            let tx = conn_tx.clone();
            accept_tasks.spawn(async move {
                loop {
                    match listener.accept().await {
                        Ok(conn) => {
                            if tx.send(conn).await.is_err() {
                                return;
                            }
                        }
                        Err(e) => {
                            warn!(error = %e, "accept_failed");
                            tokio::time::sleep(Duration::from_millis(100)).await;
                        }
                    }
                }
            });
        }
        drop(conn_tx);

        let mut connections = JoinSet::new();
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                conn = conn_rx.recv() => {
                    let Some((stream, peer)) = conn else { break };
                    let shared = Arc::clone(&self.shared);
                    connections.spawn(shared.connection(stream, peer));
                }
                Some(result) = connections.join_next(), if !connections.is_empty() => {
                    if let Err(e) = result {
                        error!(error = %e, "connection_task_failed");
                    }
                }
            }
        }
        accept_tasks.abort_all();
        info!(open_connections = connections.len(), "shutting_down");
        while connections.join_next().await.is_some() {}
        let Self {
            shared,
            capture_task,
            ..
        } = self;
        drop(shared);
        if let Some(task) = capture_task
            && let Err(e) = task.await
        {
            error!(error = %e, "capture_task_failed");
        }
    }
}

impl Shared {
    async fn connection(self: Arc<Self>, mut stream: TcpStream, peer: SocketAddr) {
        let id = self.next_connection.fetch_add(1, Ordering::Relaxed);
        let span = info_span!("connection", id, %peer);
        async move {
            debug!("connection_opened");
            let mut buf = Vec::new();
            let mut served = 0u32;
            loop {
                let read = tokio::time::timeout(
                    self.idle_timeout,
                    http::read_request(&mut stream, &mut buf),
                )
                .await;
                let request = match read {
                    Err(_) => {
                        debug!(served, "idle_timeout");
                        break;
                    }
                    Ok(Ok(None)) => break,
                    Ok(Err(e)) => {
                        warn!(error = %e, buffered = %massive_proto::field::Hex(&buf), "http_error");
                        if let Some(c) = &self.capture {
                            c.record(id, peer, Direction::In, &buf);
                        }
                        break;
                    }
                    Ok(Ok(Some(request))) => request,
                };
                if served > 0 {
                    info!(request_number = served + 1, "connection_reused");
                }
                if let Some(c) = &self.capture {
                    c.record(id, peer, Direction::In, &request.raw);
                }
                let response = self.exchange(&request);
                let is_download = request.method == "GET";
                let status = response.status.code();
                let body_len = response.body.len();
                let bytes = response.to_bytes(self.close_after_response);
                if let Some(c) = &self.capture {
                    c.record(id, peer, Direction::Out, &bytes);
                }
                let started = Instant::now();
                let written = stream.write_all(&bytes).await;
                let elapsed_ms = started.elapsed().as_millis();
                match written {
                    Ok(()) if is_download => {
                        info!(target = %request.target, status, body_len, elapsed_ms, "asset_sent");
                    }
                    Ok(()) => {}
                    Err(e) if is_download => {
                        warn!(target = %request.target, status, body_len, elapsed_ms, error = %e, "asset_send_failed");
                        break;
                    }
                    Err(e) => {
                        warn!(error = %e, "write_failed");
                        break;
                    }
                }
                served += 1;
                if self.close_after_response {
                    break;
                }
            }
            if let Err(e) = stream.shutdown().await {
                debug!(error = %e, "shutdown_failed");
            }
            debug!(served, "connection_closed");
        }
        .instrument(span)
        .await;
    }

    fn exchange(&self, request: &http::Request) -> http::Response {
        let span = info_span!("http", method = %request.method, path = %request.path());
        let _guard = span.enter();
        debug!(
            target = %request.target,
            version = request.minor_version,
            headers = %HeaderList(&request.headers),
            body_len = request.body.len(),
            "http_request"
        );
        let started = Instant::now();
        let response = self.handler.handle(request, started);
        debug!(
            status = response.status.code(),
            body_len = response.body.len(),
            elapsed_us = started.elapsed().as_micros(),
            "http_response"
        );
        response
    }
}

struct HeaderList<'a>(&'a [http::Header]);

impl std::fmt::Display for HeaderList<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for (i, h) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" | ")?;
            }
            write!(f, "{}: {}", h.name, String::from_utf8_lossy(&h.value))?;
        }
        Ok(())
    }
}
