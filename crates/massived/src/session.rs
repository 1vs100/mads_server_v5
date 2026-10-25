//! Session registry and per-session observations.

use std::collections::HashMap;
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use massive_proto::MessageType;
use massive_proto::crypto::DeviceToken;
use massive_proto::ids::{ClientTime, MassivePlayerId, SessionId, ZoneName};
use massive_proto::request::SessionRef;

/// Shortest wall-clock span over which a client time rate is reported.
const MIN_RATE_SPAN: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy)]
struct TimeSample {
    client: ClientTime,
    at: Instant,
}

#[derive(Debug)]
struct Session {
    player_id: MassivePlayerId,
    device_token: Option<DeviceToken>,
    origin: TimeSample,
    last_by_type: HashMap<MessageType, Instant>,
    zone: Option<ZoneName>,
}

#[derive(Debug)]
struct Inner {
    next_id: u64,
    sessions: HashMap<SessionId, Session>,
}

#[derive(Debug)]
pub struct Sessions {
    inner: Mutex<Inner>,
    seed: RandomState,
}

impl Default for Sessions {
    fn default() -> Self {
        Self {
            inner: Mutex::new(Inner {
                next_id: 1,
                sessions: HashMap::new(),
            }),
            seed: RandomState::new(),
        }
    }
}

/// How a request's session values relate to the registry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionMatch {
    Known(SessionId),
    /// No session with this id.
    Unknown,
    PlayerIdMismatch {
        id: SessionId,
        expected: MassivePlayerId,
    },
}

/// Client time progress relative to the session's OpenSession request.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimeRate {
    pub ticks: i128,
    pub elapsed: Duration,
    pub ticks_per_second: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    pub session: SessionMatch,
    pub since_previous: Option<Duration>,
    pub time_rate: Option<TimeRate>,
    pub zone: Option<ZoneName>,
    pub device_token: Option<DeviceToken>,
}

impl Sessions {
    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        // A panic while holding the lock leaves only counters behind; continue with them.
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn new_player_id(&self, id: SessionId) -> MassivePlayerId {
        let mut h = self.seed.build_hasher();
        h.write_u64(id.get());
        MassivePlayerId(h.finish().max(1))
    }

    pub fn open(
        &self,
        device_token: Option<DeviceToken>,
        time: ClientTime,
        now: Instant,
    ) -> (SessionId, MassivePlayerId) {
        let mut inner = self.lock();
        let id = SessionId(inner.next_id);
        inner.next_id += 1;
        let player_id = self.new_player_id(id);
        inner.sessions.insert(
            id,
            Session {
                player_id,
                device_token,
                origin: TimeSample {
                    client: time,
                    at: now,
                },
                last_by_type: HashMap::from([(MessageType::OpenSessionRequest, now)]),
                zone: None,
            },
        );
        (id, player_id)
    }

    /// Records a request of `message_type` for the referenced session.
    pub fn observe(
        &self,
        message_type: MessageType,
        reference: SessionRef,
        time: ClientTime,
        now: Instant,
    ) -> Observation {
        let mut inner = self.lock();
        let found = inner
            .sessions
            .get(&reference.id)
            .map(|s| (reference.id, s.player_id));
        let session = match found {
            None => SessionMatch::Unknown,
            Some((id, player_id)) if player_id != reference.player_id => {
                SessionMatch::PlayerIdMismatch {
                    id,
                    expected: player_id,
                }
            }
            Some((id, _)) => SessionMatch::Known(id),
        };
        let SessionMatch::Known(id) = session else {
            return Observation {
                session,
                since_previous: None,
                time_rate: None,
                zone: None,
                device_token: None,
            };
        };
        let Some(s) = inner.sessions.get_mut(&id) else {
            unreachable!("session {id} was found under the same lock")
        };
        let since_previous = s
            .last_by_type
            .insert(message_type, now)
            .map(|prev| now.duration_since(prev));
        let elapsed = now.duration_since(s.origin.at);
        let time_rate = (elapsed >= MIN_RATE_SPAN).then(|| {
            let ticks = i128::from(time.get()) - i128::from(s.origin.client.get());
            #[expect(clippy::cast_precision_loss, reason = "an estimate for display")]
            let ticks_per_second = ticks as f64 / elapsed.as_secs_f64();
            TimeRate {
                ticks,
                elapsed,
                ticks_per_second,
            }
        });
        Observation {
            session,
            since_previous,
            time_rate,
            zone: s.zone.clone(),
            device_token: s.device_token.clone(),
        }
    }

    pub fn enter_zone(&self, id: SessionId, zone: ZoneName) {
        if let Some(s) = self.lock().sessions.get_mut(&id) {
            s.zone = Some(zone);
        }
    }

    pub fn close(&self, id: SessionId) -> bool {
        self.lock().sessions.remove(&id).is_some()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.lock().sessions.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_and_interval_are_reported() {
        let sessions = Sessions::default();
        let t0 = Instant::now();
        let (id, player_id) = sessions.open(None, ClientTime(1000), t0);
        let reference = SessionRef { player_id, id };
        let first = sessions.observe(
            MessageType::HeartbeatRequest,
            reference,
            ClientTime(1500),
            t0 + Duration::from_secs(1),
        );
        assert_eq!(first.session, SessionMatch::Known(id));
        assert_eq!(first.since_previous, None);
        assert_eq!(first.time_rate, None);
        let second = sessions.observe(
            MessageType::HeartbeatRequest,
            reference,
            ClientTime(5000),
            t0 + Duration::from_secs(4),
        );
        assert_eq!(second.since_previous, Some(Duration::from_secs(3)));
        let rate = second.time_rate.unwrap();
        assert_eq!(rate.ticks, 4000);
        assert!((rate.ticks_per_second - 1000.0).abs() < 1e-9);
    }

    #[test]
    fn mismatched_token_is_reported() {
        let sessions = Sessions::default();
        let (id, player_id) = sessions.open(None, ClientTime(0), Instant::now());
        let wrong = MassivePlayerId(player_id.get() ^ 1);
        let obs = sessions.observe(
            MessageType::HeartbeatRequest,
            SessionRef {
                player_id: wrong,
                id,
            },
            ClientTime(0),
            Instant::now(),
        );
        assert_eq!(
            obs.session,
            SessionMatch::PlayerIdMismatch {
                id,
                expected: player_id
            }
        );
    }

    #[test]
    fn close_session_matches_and_closes() {
        let sessions = Sessions::default();
        let (id, player_id) = sessions.open(None, ClientTime(0), Instant::now());
        let obs = sessions.observe(
            MessageType::CloseSessionRequest,
            SessionRef { player_id, id },
            ClientTime(0),
            Instant::now(),
        );
        assert_eq!(obs.session, SessionMatch::Known(id));
        assert!(sessions.close(id));
        assert!(sessions.is_empty());
    }
}
