//! Protocol handling, independent of sockets.
//!
//! Every observation the reverse engineering status asks for is logged here
//! as an event whose message is the event name (see the README).

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use massive_proto::crypto::MacKey;
use massive_proto::field::{Field, Hex, Tree, Value};
use massive_proto::frame::Frame;
use massive_proto::ids::{Minutes, Seconds, ServerOptions, ServerTime, Service, ZoneName};
use massive_proto::request::{
    EnterZoneRequest, ImpressionUpdateRequest, OpenSessionRequest, OpenSessionSettings, Request,
    StatisticsRequest,
};
use massive_proto::response::{
    Acknowledgement, EnterZoneResponse, LocateServiceResponse, OpenSessionResponse, Response,
    ServiceHost, ServicePort,
};
use massive_proto::{MessageType, ProtocolVersion};
use thiserror::Error;
use tracing::{debug, error, info, warn};

use crate::config::{Advertise, Config, MacPolicy, ServerTimeSource};
use crate::content::Content;
use crate::http::{self, Status};
use crate::session::{Observation, SessionMatch, Sessions};

pub const PROTOCOL_CONTENT_TYPE: &str = massive_proto::transport::CONTENT_TYPE;
const ASSET_CONTENT_TYPE: &str = "application/octet-stream";

#[derive(Debug, Error)]
pub enum SetupError {
    #[error(transparent)]
    Content(#[from] crate::content::ContentError),
}

#[derive(Debug)]
pub struct Handler {
    key: MacKey,
    mac_policy: MacPolicy,
    server_time: ServerTimeSource,
    locate: LocateServiceResponse,
    content: Content,
    zone_names: Vec<ZoneName>,
    sessions: Sessions,
}

impl Handler {
    pub fn new(config: &Config, advertise: &Advertise) -> Result<Self, SetupError> {
        let key = config
            .protocol
            .key
            .as_ref()
            .map_or_else(MacKey::retail, |k| MacKey::new(k.as_bytes().to_vec()));
        if key != MacKey::retail() {
            warn!(
                "configured key differs from the retail literal; the client verifies responses with the literal"
            );
        }
        let Advertise { host, port } = advertise.clone();
        let l = &config.locate;
        let locate = LocateServiceResponse {
            server_time: ServerTime(0),
            hosts: Service::ALL
                .map(|service| ServiceHost {
                    service,
                    host: host.clone(),
                })
                .to_vec(),
            ports: Service::ALL
                .map(|service| ServicePort { service, port })
                .to_vec(),
            auto_reporting_time: l.auto_reporting_time_seconds.map(Seconds),
            server_options: l.server_options.map(ServerOptions),
            statistic_period: l.statistic_period_minutes.map(Minutes),
            heartbeat_period: l.heartbeat_period_minutes.map(Minutes),
            logging_level: l.logging_level,
            value_57: l.value_57,
        };
        let content = Content::load(config, advertise)?;
        let zone_names = content.announced_zones().to_vec();
        Ok(Self {
            key,
            mac_policy: config.protocol.mac_policy,
            server_time: config.protocol.server_time,
            locate,
            content,
            zone_names,
            sessions: Sessions::default(),
        })
    }

    #[must_use]
    pub fn content(&self) -> &Content {
        &self.content
    }

    #[must_use]
    pub fn sessions(&self) -> &Sessions {
        &self.sessions
    }

    pub fn handle(&self, request: &http::Request, now: Instant) -> http::Response {
        match request.method.as_str() {
            "POST" => self.handle_post(request.path(), &request.body, now),
            "GET" => self.handle_get(request),
            other => {
                warn!(method = other, "unsupported_method");
                http::Response::empty(Status::MethodNotAllowed)
            }
        }
    }

    fn handle_get(&self, request: &http::Request) -> http::Response {
        let text = |name| {
            request
                .header(name)
                .map(|h| String::from_utf8_lossy(h).into_owned())
        };
        let host = text("host");
        let user_agent = text("user-agent");
        // The response is always the whole file; a range shows a resumed download.
        let range = text("range");
        let Some(asset) = self.content.asset_by_path(request.path()) else {
            warn!(target = %request.target, host, user_agent, range, "asset_not_found");
            return http::Response::empty(Status::NotFound);
        };
        info!(
            asset_id = %asset.id,
            bytes = asset.body.len(),
            target = %request.target,
            target_form = %request.target_form(),
            host,
            user_agent,
            range,
            "asset_request"
        );
        http::Response::new(Status::Ok, ASSET_CONTENT_TYPE, asset.body.to_vec())
    }

    fn handle_post(&self, path: &str, body: &[u8], now: Instant) -> http::Response {
        let path_type = MessageType::from_path(path);
        let frame = match Frame::parse(body) {
            Ok(frame) => frame,
            Err(e) => {
                warn!(path, error = %e, bytes = %Hex(body), "decode_failed");
                return http::Response::empty(Status::BadRequest);
            }
        };
        if frame.version() != ProtocolVersion::RETAIL {
            warn!(version = %frame.version(), "unexpected_version");
        }
        if frame.message_type() != path_type {
            warn!(
                path,
                type_byte = frame.type_byte(),
                "path_does_not_match_message_type"
            );
        }
        if let Some(partial) = frame.fields_partial() {
            debug!(fields = %Tree(&partial.fields), "fields");
            report_opaque(&partial.fields);
            if let Some(e) = partial.error {
                warn!(error = %e, bytes = %Hex(body), "decode_failed");
                return http::Response::empty(Status::BadRequest);
            }
        }
        let frame = match frame.verify(&self.key) {
            Ok(frame) => frame,
            Err(mismatch) => {
                warn!(expected = %mismatch.expected, found = %mismatch.found, "mac_mismatch");
                match self.mac_policy {
                    MacPolicy::Enforce => return http::Response::empty(Status::BadRequest),
                    MacPolicy::Report => mismatch.frame.waive_verification(),
                }
            }
        };
        let request = match Request::decode(&frame) {
            Ok(request) => request,
            Err(e) => {
                warn!(error = %e, bytes = %Hex(body), "decode_failed");
                return http::Response::empty(Status::BadRequest);
            }
        };
        let response = self.respond(&request, now);
        match response.encode(&self.key) {
            Ok(bytes) => {
                self_check(&bytes);
                http::Response::new(Status::Ok, PROTOCOL_CONTENT_TYPE, bytes)
            }
            Err(e) => {
                error!(error = %e, response = ?response.message_type(), "response_encoding_failed");
                http::Response::empty(Status::InternalServerError)
            }
        }
    }

    fn respond(&self, request: &Request, now: Instant) -> Response {
        let observation = request
            .session()
            .zip(request.time())
            .map(|(reference, time)| {
                let obs = self
                    .sessions
                    .observe(request.message_type(), reference, time, now);
                log_observation(request, &obs);
                obs
            });
        match request {
            Request::LocateService(r) => {
                info!(
                    sku_name = %r.sku_name,
                    sku_version = %r.sku_version,
                    device_token = r.device_token.as_ref().map(ToString::to_string),
                    "locate_service"
                );
                let mut locate = self.locate.clone();
                locate.server_time = self.server_time();
                Response::LocateService(locate)
            }
            Request::OpenSession(r) => Response::OpenSession(self.open_session(r, now)),
            Request::EnterZone(r) => Response::EnterZone(self.enter_zone(r, observation.as_ref())),
            Request::ImpressionUpdate(r) => {
                log_impressions(r);
                Response::Acknowledgement(Acknowledgement::ImpressionUpdate)
            }
            Request::Statistics(r) => {
                log_statistics(r);
                Response::Acknowledgement(Acknowledgement::Statistics)
            }
            Request::Heartbeat(_) => Response::Acknowledgement(Acknowledgement::Heartbeat),
            Request::PlayerUpdate(r) => {
                info!(players = r.players.len(), "player_update");
                Response::Acknowledgement(Acknowledgement::PlayerUpdate)
            }
            Request::CloseSession(_) => {
                if let Some(SessionMatch::Known(id)) = observation.map(|o| o.session) {
                    self.sessions.close(id);
                    info!(session_id = %id, open_sessions = self.sessions.len(), "close_session");
                }
                Response::Acknowledgement(Acknowledgement::CloseSession)
            }
        }
    }

    fn open_session(&self, r: &OpenSessionRequest, now: Instant) -> OpenSessionResponse {
        let retail_key = MacKey::retail();
        if r.key_string.as_str().as_bytes() != retail_key.as_bytes() {
            warn!(key_string = %r.key_string, "key_string_differs");
        }
        if r.settings != OpenSessionSettings::RETAIL {
            info!(settings = ?r.settings, "open_session_settings_differ");
        }
        let (session_id, massive_player_id) =
            self.sessions.open(r.device_token.clone(), r.time, now);
        info!(
            session_id = %session_id,
            massive_player_id = %massive_player_id,
            sku_name = %r.sku_name,
            sku_version = %r.sku_version,
            build = %r.build,
            device_token = r.device_token.as_ref().map(ToString::to_string),
            players = r.players.len(),
            client_time = %r.time,
            "open_session"
        );
        OpenSessionResponse {
            massive_player_id,
            session_id,
            zones: self.zone_names.clone(),
        }
    }

    fn enter_zone(&self, r: &EnterZoneRequest, obs: Option<&Observation>) -> EnterZoneResponse {
        if let Some(SessionMatch::Known(id)) = obs.map(|o| o.session) {
            self.sessions.enter_zone(id, r.zone.clone());
        }
        let response = self.content.zone(&r.zone).cloned().unwrap_or_else(|| {
            warn!(zone = %r.zone, "unknown_zone");
            EnterZoneResponse {
                maos: Vec::new(),
                orders: Vec::new(),
            }
        });
        info!(
            zone = %r.zone,
            maos = response.maos.len(),
            orders = response.orders.len(),
            bandwidth = ?r.bandwidth,
            players = r.players.len(),
            "enter_zone"
        );
        for order in &response.orders {
            for asset in &order.assets {
                info!(
                    zone = %r.zone,
                    order_id = order.order_id.get(),
                    asset_id = asset.asset_id.get(),
                    url = asset.url.as_str(),
                    bytes = asset.size.get(),
                    "asset_offered"
                );
            }
        }
        response
    }

    fn server_time(&self) -> ServerTime {
        let since_epoch = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default();
        ServerTime(match self.server_time {
            ServerTimeSource::UnixMilliseconds => {
                u64::try_from(since_epoch.as_millis()).unwrap_or(u64::MAX)
            }
            ServerTimeSource::UnixSeconds => since_epoch.as_secs(),
            ServerTimeSource::Fixed(v) => v,
        })
    }
}

fn log_observation(request: &Request, obs: &Observation) {
    let message_type = request.message_type();
    match obs.session {
        SessionMatch::Known(id) => {
            info!(
                message_type = %message_type,
                session_id = %id,
                client_time = request.time().map(massive_proto::ids::ClientTime::get),
                since_previous_ms = obs.since_previous.map(|d| d.as_millis()),
                zone = obs.zone.as_ref().map(ToString::to_string),
                "request"
            );
            if let Some(rate) = obs.time_rate {
                info!(
                    session_id = %id,
                    ticks = %rate.ticks,
                    elapsed_ms = rate.elapsed.as_millis(),
                    ticks_per_second = rate.ticks_per_second,
                    "client_time"
                );
            }
        }
        SessionMatch::Unknown => {
            warn!(message_type = %message_type, session = ?request.session(), "unknown_session");
        }
        SessionMatch::PlayerIdMismatch { id, expected } => {
            warn!(
                message_type = %message_type,
                session_id = %id,
                expected_player_id = %expected,
                session = ?request.session(),
                "player_id_mismatch"
            );
        }
    }
}

fn log_impressions(r: &ImpressionUpdateRequest) {
    let names: Vec<&str> = r
        .impressions
        .iter()
        .map(massive_proto::ids::IeName::as_str)
        .collect();
    info!(
        records = r.impressions.len(),
        value_21 = r.counters.value_21,
        value_29 = r.counters.value_29,
        names = ?names,
        players = r.players.len(),
        "impression_update"
    );
}

fn log_statistics(r: &StatisticsRequest) {
    info!(
        cache_lookups = r.counters.cache_lookups,
        cache_hits = r.counters.cache_hits,
        cache_mismatches = r.counters.cache_mismatches,
        "statistics"
    );
}

/// Reports block fields the schema does not know.
fn report_opaque(fields: &[Field]) {
    for field in fields {
        match &field.value {
            Value::Opaque(bytes) => {
                warn!(tag = %field.tag, offset = field.offset, bytes = %Hex(bytes), "unknown_field");
            }
            Value::Block(children) => report_opaque(children),
            _ => {}
        }
    }
}

/// Parses an outgoing response with the client's rules and the literal key.
/// A failure means the server would send something the client rejects.
fn self_check(bytes: &[u8]) {
    let result = Frame::parse(bytes)
        .map_err(|e| e.to_string())
        .and_then(|f| f.verify(&MacKey::retail()).map_err(|e| e.to_string()))
        .and_then(|f| Response::decode(&f).map_err(|e| e.to_string()));
    if let Err(reason) = result {
        error!(reason, bytes = %Hex(bytes), "response_fails_client_rules");
    }
}
