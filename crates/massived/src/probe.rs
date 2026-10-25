//! Client simulator.
//!
//! Plays the request sequence of the retail client against a server and
//! checks every response with the client's rules, including the download
//! size and digest checks. Used to verify a deployment without a console.

use std::net::SocketAddr;
use std::time::Instant;

use massive_proto::crypto::{AssetDigest, DeviceToken, MacAddress, MacKey};
use massive_proto::ids::{
    BuildString, ClientTime, IeName, KeyString, MassivePlayerId, PlayerId, PlayerSlot, SessionId,
    SkuName, SkuVersion, ZoneName,
};
use massive_proto::request::{
    CloseSessionRequest, EnterZoneRequest, HeartbeatRequest, ImpressionCounters,
    ImpressionUpdateRequest, LocateServiceRequest, OpenSessionRequest, OpenSessionSettings,
    PlayerBlock, PlayerUpdateRequest, StatisticsCounters, StatisticsRequest,
};
use massive_proto::response::{EnterZoneResponse, LocateServiceResponse, OpenSessionResponse};
use massive_proto::transport::{self, Method};
use massive_proto::wire::EncodeError;
use massive_proto::{DecodeError, Frame, MessageType, Request, Response};
use thiserror::Error;
use tokio::net::TcpStream;
use tracing::{Instrument, info, info_span};

use crate::content::url_path;
use crate::http::{self, HttpError};

#[derive(Debug, Clone)]
pub struct ProbeOptions {
    pub target: SocketAddr,
    pub zone: ZoneName,
    pub mac_address: MacAddress,
    pub sku_name: SkuName,
    pub sku_version: SkuVersion,
}

#[derive(Debug, Error)]
pub enum ProbeError {
    #[error("{step}: cannot connect to {addr}: {source}")]
    Connect {
        step: &'static str,
        addr: SocketAddr,
        source: std::io::Error,
    },
    #[error("{step}: {source}")]
    Http {
        step: &'static str,
        source: HttpError,
    },
    #[error("{step}: HTTP status {code}")]
    Status { step: &'static str, code: u16 },
    #[error("{step}: {source}")]
    Frame {
        step: &'static str,
        source: massive_proto::frame::FrameError,
    },
    #[error("{step}: response MAC does not verify with the retail key")]
    Mac { step: &'static str },
    #[error("{step}: the client would reject the response: {source}")]
    Decode {
        step: &'static str,
        source: DecodeError,
    },
    #[error("{step}: expected {expected}, received {found}")]
    UnexpectedResponse {
        step: &'static str,
        expected: MessageType,
        found: MessageType,
    },
    #[error("{step}: {source}")]
    Encode {
        step: &'static str,
        source: EncodeError,
    },
    #[error("zone {zone} was not announced in the OpenSession response")]
    ZoneNotAnnounced { zone: ZoneName },
    #[error("asset {asset_id}: download is {found} bytes, the record says {expected}")]
    DownloadSize {
        asset_id: u32,
        expected: u32,
        found: usize,
    },
    #[error("asset {asset_id}: SHA-1 of the download does not match the record")]
    DownloadDigest { asset_id: u32 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProbeReport {
    pub locate: LocateServiceResponse,
    pub session: OpenSessionResponse,
    pub zone: EnterZoneResponse,
    pub downloads: usize,
}

struct Client {
    target: SocketAddr,
    key: MacKey,
    started: Instant,
}

impl Client {
    fn time(&self) -> ClientTime {
        ClientTime(u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX))
    }

    async fn send(
        &self,
        step: &'static str,
        method: Method,
        target: &str,
        body: &[u8],
    ) -> Result<Vec<u8>, ProbeError> {
        let mut stream =
            TcpStream::connect(self.target)
                .await
                .map_err(|source| ProbeError::Connect {
                    step,
                    addr: self.target,
                    source,
                })?;
        let mut request = transport::request_head(
            method,
            target,
            &self.target.ip().to_string(),
            self.target.port(),
            body.len(),
        )
        .into_bytes();
        request.extend_from_slice(body);
        let response = http::exchange(&mut stream, &request)
            .await
            .map_err(|source| ProbeError::Http { step, source })?;
        if response.code != 200 {
            return Err(ProbeError::Status {
                step,
                code: response.code,
            });
        }
        Ok(response.body)
    }

    async fn call(&self, step: &'static str, request: &Request) -> Result<Response, ProbeError> {
        let body = request
            .encode(&self.key)
            .map_err(|source| ProbeError::Encode { step, source })?;
        let path = request.message_type().path().unwrap_or_default();
        let reply = self.send(step, Method::Post, path, &body).await?;
        let frame = Frame::parse(&reply).map_err(|source| ProbeError::Frame { step, source })?;
        let frame = frame
            .verify(&MacKey::retail())
            .map_err(|_| ProbeError::Mac { step })?;
        let response =
            Response::decode(&frame).map_err(|source| ProbeError::Decode { step, source })?;
        let expected = request
            .message_type()
            .response()
            .unwrap_or(MessageType::LocateServiceResponse);
        if response.message_type() != expected {
            return Err(ProbeError::UnexpectedResponse {
                step,
                expected,
                found: response.message_type(),
            });
        }
        info!(step, response = %response.message_type(), "probe_step_passed");
        Ok(response)
    }
}

/// Runs the sequence. Returns at the first step the retail client would fail.
pub async fn run(options: &ProbeOptions) -> Result<ProbeReport, ProbeError> {
    let span = info_span!("probe", target = %options.target, zone = %options.zone);
    run_inner(options).instrument(span).await
}

#[expect(clippy::too_many_lines, reason = "one linear request sequence")]
async fn run_inner(options: &ProbeOptions) -> Result<ProbeReport, ProbeError> {
    let client = Client {
        target: options.target,
        key: MacKey::retail(),
        started: Instant::now(),
    };
    let device_token = DeviceToken::from_mac_address(options.mac_address);
    let player = PlayerBlock {
        slot: PlayerSlot(0),
        player_id: PlayerId([0x11; 16]),
    };

    let Response::LocateService(locate) = client
        .call(
            "locate_service",
            &Request::LocateService(LocateServiceRequest {
                sku_name: options.sku_name.clone(),
                sku_version: options.sku_version.clone(),
                device_token: Some(device_token.clone()),
            }),
        )
        .await?
    else {
        unreachable!("call checks the response type")
    };

    let key_string = KeyString::new(String::from_utf8_lossy(MacKey::RETAIL).into_owned())
        .unwrap_or_else(|_| unreachable!("the retail key is printable ASCII"));
    let build = BuildString::new(transport::BUILD_STRING)
        .unwrap_or_else(|_| unreachable!("the build string is printable ASCII"));
    let Response::OpenSession(session) = client
        .call(
            "open_session",
            &Request::OpenSession(OpenSessionRequest {
                sku_name: options.sku_name.clone(),
                sku_version: options.sku_version.clone(),
                settings: OpenSessionSettings::RETAIL,
                key_string,
                device_token: Some(device_token),
                build,
                players: vec![player],
                time: client.time(),
            }),
        )
        .await?
    else {
        unreachable!("call checks the response type")
    };
    if !session.zones.contains(&options.zone) {
        return Err(ProbeError::ZoneNotAnnounced {
            zone: options.zone.clone(),
        });
    }
    let (player_id, id): (MassivePlayerId, SessionId) =
        (session.massive_player_id, session.session_id);

    client
        .call(
            "player_update",
            &Request::PlayerUpdate(PlayerUpdateRequest {
                massive_player_id: player_id,
                session_id: id,
                players: vec![player],
                time: client.time(),
            }),
        )
        .await?;

    let Response::EnterZone(zone) = client
        .call(
            "enter_zone",
            &Request::EnterZone(EnterZoneRequest {
                zone: options.zone.clone(),
                massive_player_id: player_id,
                session_id: id,
                players: vec![player],
                bandwidth: None,
                time: client.time(),
            }),
        )
        .await?
    else {
        unreachable!("call checks the response type")
    };

    let mut downloads = 0;
    for asset in zone.orders.iter().flat_map(|o| &o.assets) {
        let body = client
            .send("download", Method::Get, url_path(asset.url.as_str()), &[])
            .await?;
        if body.len() != asset.size.get() as usize {
            return Err(ProbeError::DownloadSize {
                asset_id: asset.asset_id.get(),
                expected: asset.size.get(),
                found: body.len(),
            });
        }
        if AssetDigest::of(&body) != asset.digest {
            return Err(ProbeError::DownloadDigest {
                asset_id: asset.asset_id.get(),
            });
        }
        info!(asset_id = %asset.asset_id, bytes = body.len(), "probe_download_passed");
        downloads += 1;
    }

    let impressions: Vec<IeName> = zone
        .maos
        .iter()
        .map(|m| m.ie_name.clone())
        .take(1)
        .collect();
    let records = u16::try_from(impressions.len()).unwrap_or(u16::MAX);
    client
        .call(
            "impression_update",
            &Request::ImpressionUpdate(ImpressionUpdateRequest {
                massive_player_id: player_id,
                session_id: id,
                impressions,
                players: vec![player],
                counters: ImpressionCounters {
                    value_21: records,
                    value_29: 0,
                },
                time: client.time(),
            }),
        )
        .await?;
    client
        .call(
            "heartbeat",
            &Request::Heartbeat(HeartbeatRequest {
                massive_player_id: player_id,
                session_id: id,
                time: client.time(),
            }),
        )
        .await?;
    client
        .call(
            "statistics",
            &Request::Statistics(StatisticsRequest {
                session_id: id,
                massive_player_id: player_id,
                counters: StatisticsCounters {
                    cache_lookups: 0,
                    cache_hits: 0,
                    cache_mismatches: 0,
                },
                time: client.time(),
            }),
        )
        .await?;
    client
        .call(
            "close_session",
            &Request::CloseSession(CloseSessionRequest {
                massive_player_id: player_id,
                session_id: id,
                time: client.time(),
            }),
        )
        .await?;

    Ok(ProbeReport {
        locate,
        session,
        zone,
        downloads,
    })
}
