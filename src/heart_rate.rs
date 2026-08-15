//! Local HTTP receiver for samples from compatible heart-rate senders.

use std::{
    future::IntoFuture,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    routing::post,
};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::{
    analysis::GameSnapshot,
    runtime::{EventLevel, SharedState, ToastLevel},
};

pub const BASE_PORT: u16 = 49_670;
pub const PORT_COUNT: u16 = 5;
pub const PROTOCOL_VERSION: u16 = 1;
pub const API_PATH: &str = "/api/v1/heart-rate";
pub const SERVICE_ID: &str = "heart-rate-receiver";
pub const ONLINE_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_REQUEST_BODY_BYTES: usize = 4 * 1024;
const NEGOTIATION_WARNING_AFTER: Duration = Duration::from_secs(10);
const SUPERVISOR_TICK: Duration = Duration::from_millis(100);
const BIND_RETRY_DELAY: Duration = Duration::from_secs(1);
const GRACEFUL_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HeartRateRequest {
    pub protocol_version: u16,
    #[serde(deserialize_with = "deserialize_nullable_heart_rate")]
    pub heart_rate: Option<u16>,
}

fn deserialize_nullable_heart_rate<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<u16>::deserialize(deserializer)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HeartRateResponse {
    pub service: &'static str,
    pub protocol_version: u16,
    pub accepted: bool,
}

#[derive(Debug, Default)]
struct HeartRateState {
    server_running: bool,
    last_received: Option<Instant>,
    value: Option<u16>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SharedHeartRate(std::sync::Arc<RwLock<HeartRateState>>);

impl SharedHeartRate {
    fn set_server_running(&self, running: bool) {
        let mut state = self.0.write();
        state.server_running = running;
        if !running {
            state.last_received = None;
            state.value = None;
        }
    }

    fn record(&self, value: Option<u16>) {
        let mut state = self.0.write();
        state.last_received = Some(Instant::now());
        state.value = value;
    }

    fn online_at(&self, now: Instant) -> bool {
        let state = self.0.read();
        state.server_running
            && state
                .last_received
                .is_some_and(|received| now.saturating_duration_since(received) < ONLINE_TIMEOUT)
    }

    pub(crate) fn apply_to(&self, snapshot: &mut GameSnapshot, enabled: bool) {
        let state = self.0.read();
        let online = enabled
            && state.server_running
            && state.last_received.is_some_and(|received| {
                Instant::now().saturating_duration_since(received) < ONLINE_TIMEOUT
            });
        snapshot.has_heart_rate = online;
        snapshot.heart_rate = if online { state.value.unwrap_or(0) } else { 0 };
    }
}

#[derive(Clone)]
struct HttpState {
    heart_rate: SharedHeartRate,
    config: Arc<RwLock<crate::runtime::LiveConfig>>,
    accepting: Arc<AtomicBool>,
}

pub fn spawn(shared: SharedState) -> thread::JoinHandle<()> {
    thread::Builder::new()
        .name("ecliptica-heart-rate-server".to_owned())
        .spawn(move || {
            while !shared.shutdown.load(Ordering::Relaxed)
                && !shared.config.read().value.heart_rate_enabled
            {
                thread::sleep(SUPERVISOR_TICK);
            }
            if shared.shutdown.load(Ordering::Relaxed) {
                return;
            }

            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(error) => {
                    tracing::error!(%error, "failed to create heart-rate runtime");
                    shared.toast_event(
                        EventLevel::Error,
                        ToastLevel::Error,
                        shared.text(crate::i18n::text::HEART_RATE_SERVER_FAILED),
                    );
                    return;
                }
            };
            runtime.block_on(run(shared));
        })
        .expect("failed to start heart-rate server")
}

async fn run(shared: SharedState) {
    let mut bind_warning_emitted = false;
    while !shared.shutdown.load(Ordering::Relaxed) {
        if !shared.config.read().value.heart_rate_enabled {
            shared.heart_rate.set_server_running(false);
            shared.refresh_heart_rate();
            tokio::time::sleep(SUPERVISOR_TICK).await;
            continue;
        }

        let Some((listener, port)) = bind_first_available().await else {
            if !bind_warning_emitted {
                shared.toast_event(
                    EventLevel::Warning,
                    ToastLevel::Warning,
                    shared.text(crate::i18n::text::HEART_RATE_NO_PORT),
                );
                bind_warning_emitted = true;
            }
            tokio::time::sleep(BIND_RETRY_DELAY).await;
            continue;
        };
        bind_warning_emitted = false;
        shared.heart_rate.set_server_running(true);
        shared.refresh_heart_rate();
        tracing::info!(port, "heart-rate receiver ready");

        let http_state = HttpState {
            heart_rate: shared.heart_rate.clone(),
            config: shared.config.clone(),
            accepting: Arc::new(AtomicBool::new(true)),
        };
        let accepting = http_state.accepting.clone();
        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
        let mut shutdown_tx = Some(shutdown_tx);
        let mut server = Box::pin(
            axum::serve(listener, router(http_state))
                .with_graceful_shutdown(async move {
                    let _ = shutdown_rx.await;
                })
                .into_future(),
        );
        let listening_since = Instant::now();
        let mut was_online = false;
        let mut waiting_warning_emitted = false;

        loop {
            tokio::select! {
                result = &mut server => {
                    if let Err(error) = result {
                        tracing::error!(%error, "heart-rate receiver stopped unexpectedly");
                        shared.toast_event(
                            EventLevel::Error,
                            ToastLevel::Error,
                            shared.text(crate::i18n::text::HEART_RATE_SERVER_FAILED),
                        );
                    }
                    break;
                }
                _ = tokio::time::sleep(SUPERVISOR_TICK) => {
                    let now = Instant::now();
                    let online = shared.heart_rate.online_at(now);
                    shared.refresh_heart_rate();
                    if online && !was_online {
                        shared.toast_event(
                            EventLevel::Info,
                            ToastLevel::Success,
                            shared.text(crate::i18n::text::HEART_RATE_CONNECTED),
                        );
                        waiting_warning_emitted = false;
                    } else if !online && was_online {
                        shared.toast_event(
                            EventLevel::Warning,
                            ToastLevel::Warning,
                            shared.text(crate::i18n::text::HEART_RATE_DISCONNECTED),
                        );
                        waiting_warning_emitted = true;
                    } else if !online
                        && !waiting_warning_emitted
                        && now.saturating_duration_since(listening_since) >= NEGOTIATION_WARNING_AFTER
                    {
                        shared.toast_event(
                            EventLevel::Warning,
                            ToastLevel::Warning,
                            shared.text(crate::i18n::text::HEART_RATE_WAITING),
                        );
                        waiting_warning_emitted = true;
                    }
                    was_online = online;

                    let stopping = shared.shutdown.load(Ordering::Relaxed)
                        || !shared.config.read().value.heart_rate_enabled;
                    if stopping {
                        accepting.store(false, Ordering::Release);
                        if let Some(tx) = shutdown_tx.take() {
                            let _ = tx.send(());
                        }
                        if tokio::time::timeout(GRACEFUL_SHUTDOWN_TIMEOUT, &mut server).await.is_err() {
                            tracing::warn!("heart-rate HTTP server exceeded graceful shutdown timeout");
                        }
                        break;
                    }
                }
            }
        }
        shared.heart_rate.set_server_running(false);
        shared.refresh_heart_rate();
    }
}

async fn bind_first_available() -> Option<(tokio::net::TcpListener, u16)> {
    for port in BASE_PORT..BASE_PORT + PORT_COUNT {
        // This inexpensive third-party probe avoids needless async bind attempts.
        // The actual bind remains authoritative because another process can win
        // the port between the probe and this call.
        if !portpicker::is_free(port) {
            continue;
        }
        let address = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), port);
        if let Ok(listener) = tokio::net::TcpListener::bind(address).await {
            return Some((listener, port));
        }
    }
    None
}

fn router(state: HttpState) -> Router {
    Router::new()
        .route(API_PATH, post(post_heart_rate))
        .layer(DefaultBodyLimit::max(MAX_REQUEST_BODY_BYTES))
        .with_state(state)
}

async fn post_heart_rate(
    State(state): State<HttpState>,
    Json(request): Json<HeartRateRequest>,
) -> Result<Json<HeartRateResponse>, StatusCode> {
    if !state.accepting.load(Ordering::Acquire) || !state.config.read().value.heart_rate_enabled {
        return Err(StatusCode::SERVICE_UNAVAILABLE);
    }
    if request.protocol_version != PROTOCOL_VERSION {
        return Err(StatusCode::BAD_REQUEST);
    }
    state.heart_rate.record(request.heart_rate);
    Ok(Json(HeartRateResponse {
        service: SERVICE_ID,
        protocol_version: PROTOCOL_VERSION,
        accepted: true,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn online_timeout_and_null_sample_follow_metric_contract() {
        let shared = SharedHeartRate::default();
        shared.set_server_running(true);
        shared.record(None);
        let mut snapshot = GameSnapshot::default();
        shared.apply_to(&mut snapshot, true);
        assert!(snapshot.has_heart_rate);
        assert_eq!(snapshot.heart_rate, 0);

        shared.0.write().last_received =
            Some(Instant::now() - ONLINE_TIMEOUT - Duration::from_millis(1));
        shared.apply_to(&mut snapshot, true);
        assert!(!snapshot.has_heart_rate);
        assert_eq!(snapshot.heart_rate, 0);
    }

    #[test]
    fn disabled_metric_is_unavailable_even_after_a_valid_sample() {
        let shared = SharedHeartRate::default();
        shared.set_server_running(true);
        shared.record(Some(72));
        let mut snapshot = GameSnapshot::default();
        shared.apply_to(&mut snapshot, false);
        assert!(!snapshot.has_heart_rate);
    }

    #[test]
    fn protocol_payload_accepts_value_and_null() {
        let value: HeartRateRequest =
            serde_json::from_str(r#"{"protocol_version":1,"heart_rate":88}"#).unwrap();
        assert_eq!(value.protocol_version, 1);
        assert_eq!(value.heart_rate, Some(88));
        let missing: HeartRateRequest =
            serde_json::from_str(r#"{"protocol_version":1,"heart_rate":null}"#).unwrap();
        assert_eq!(missing.heart_rate, None);
    }

    #[test]
    fn protocol_handler_rejects_wrong_version_and_disabled_session() {
        let config = Arc::new(RwLock::new(crate::runtime::LiveConfig {
            value: crate::config::AppConfig {
                heart_rate_enabled: true,
                ..crate::config::AppConfig::default()
            },
            revision: 0,
        }));
        let state = HttpState {
            heart_rate: SharedHeartRate::default(),
            config: config.clone(),
            accepting: Arc::new(AtomicBool::new(true)),
        };
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();

        let accepted = runtime
            .block_on(post_heart_rate(
                State(state.clone()),
                Json(HeartRateRequest {
                    protocol_version: PROTOCOL_VERSION,
                    heart_rate: Some(90),
                }),
            ))
            .unwrap();
        assert_eq!(
            accepted.0,
            HeartRateResponse {
                service: SERVICE_ID,
                protocol_version: PROTOCOL_VERSION,
                accepted: true,
            }
        );

        let wrong_version = runtime.block_on(post_heart_rate(
            State(state.clone()),
            Json(HeartRateRequest {
                protocol_version: 2,
                heart_rate: Some(90),
            }),
        ));
        assert_eq!(wrong_version.unwrap_err(), StatusCode::BAD_REQUEST);

        config.write().value.heart_rate_enabled = false;
        let disabled = runtime.block_on(post_heart_rate(
            State(state),
            Json(HeartRateRequest {
                protocol_version: PROTOCOL_VERSION,
                heart_rate: Some(90),
            }),
        ));
        assert_eq!(disabled.unwrap_err(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn protocol_rejects_unknown_fields() {
        assert!(
            serde_json::from_str::<HeartRateRequest>(
                r#"{"protocol_version":1,"heart_rate":72,"extra":true}"#
            )
            .is_err()
        );
        assert!(
            serde_json::from_str::<HeartRateRequest>(r#"{"protocol_version":1}"#).is_err(),
            "heart_rate is required even though its explicit value may be null"
        );
    }
}
