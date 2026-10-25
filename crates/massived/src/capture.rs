//! Capture of raw HTTP exchanges as JSON lines.
//!
//! One line per message with the exact bytes, so a capture can be decoded
//! again later with `massived decode --capture`.

use std::io::BufRead as _;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::{error, warn};

const QUEUE_DEPTH: usize = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    /// Client to server.
    In,
    /// Server to client.
    Out,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Record {
    pub time_ms: u64,
    pub connection: u64,
    pub peer: SocketAddr,
    pub direction: Direction,
    /// Lowercase hex of the HTTP message.
    pub bytes: String,
}

#[derive(Debug, Clone)]
pub struct Capture {
    tx: mpsc::Sender<Record>,
}

#[derive(Debug, Error)]
pub enum CaptureError {
    #[error("cannot open capture file {path}: {source}")]
    Open {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("cannot read capture file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}:{line}: {source}")]
    Parse {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
    #[error("{path}:{line}: bytes are not hex: {source}")]
    Hex {
        path: PathBuf,
        line: usize,
        source: hex::FromHexError,
    },
}

impl Capture {
    /// Opens `path` for appending and starts the writer task. The task ends
    /// when every [`Capture`] clone is dropped.
    pub async fn open(path: &Path) -> Result<(Self, JoinHandle<()>), CaptureError> {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .await
            .map_err(|source| CaptureError::Open {
                path: path.to_owned(),
                source,
            })?;
        let (tx, mut rx) = mpsc::channel::<Record>(QUEUE_DEPTH);
        let path = path.to_owned();
        let task = tokio::spawn(async move {
            while let Some(record) = rx.recv().await {
                let mut line = match serde_json::to_vec(&record) {
                    Ok(line) => line,
                    Err(e) => {
                        error!(error = %e, "capture record not serializable");
                        continue;
                    }
                };
                line.push(b'\n');
                if let Err(e) = file.write_all(&line).await {
                    error!(path = %path.display(), error = %e, "capture write failed; capture stopped");
                    return;
                }
            }
            if let Err(e) = file.flush().await {
                error!(path = %path.display(), error = %e, "capture flush failed");
            }
        });
        Ok((Self { tx }, task))
    }

    pub fn record(&self, connection: u64, peer: SocketAddr, direction: Direction, bytes: &[u8]) {
        let record = Record {
            time_ms: unix_millis(),
            connection,
            peer,
            direction,
            bytes: hex::encode(bytes),
        };
        if self.tx.try_send(record).is_err() {
            warn!(connection, "capture queue full or closed; record dropped");
        }
    }
}

fn unix_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| u64::try_from(d.as_millis()).unwrap_or(u64::MAX))
}

/// Reads all records of a capture file with their decoded bytes.
pub fn read(path: &Path) -> Result<Vec<(Record, Vec<u8>)>, CaptureError> {
    let file = std::fs::File::open(path).map_err(|source| CaptureError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut out = Vec::new();
    for (index, line) in std::io::BufReader::new(file).lines().enumerate() {
        let line_no = index + 1;
        let line = line.map_err(|source| CaptureError::Read {
            path: path.to_owned(),
            source,
        })?;
        if line.trim().is_empty() {
            continue;
        }
        let record: Record = serde_json::from_str(&line).map_err(|source| CaptureError::Parse {
            path: path.to_owned(),
            line: line_no,
            source,
        })?;
        let bytes = hex::decode(&record.bytes).map_err(|source| CaptureError::Hex {
            path: path.to_owned(),
            line: line_no,
            source,
        })?;
        out.push((record, bytes));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn written_records_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("capture.jsonl");
        let (capture, task) = Capture::open(&path).await.unwrap();
        let peer: SocketAddr = "127.0.0.1:9".parse().unwrap();
        capture.record(1, peer, Direction::In, b"POST /");
        capture.record(1, peer, Direction::Out, b"HTTP/1.1 200 OK");
        drop(capture);
        task.await.unwrap();
        let records = read(&path).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].1, b"POST /");
        assert_eq!(records[1].0.direction, Direction::Out);
    }
}
