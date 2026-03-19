use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Instant, UNIX_EPOCH};

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::{broadcast, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use harpoon_core::types::event::{Event, EventKind};
use harpoon_core::EngineHandle;

use super::proto::*;

const MAX_EVENTS_BUFFER: usize = 1000;

pub struct ControlState {
    pub engine_handle: Option<EngineHandle>,
    pub config_path: PathBuf,
    pub start_time: Instant,
    pub rules_info: Vec<RuleInfo>,
    pub cancel: CancellationToken,
    pub recent_events: Arc<Mutex<Vec<EventInfo>>>,
    pub reload_tx: tokio::sync::mpsc::Sender<PathBuf>,
    pub app_config: Option<crate::config::schema::AppConfig>,
}

pub async fn run_control_server(
    socket_path: &Path,
    state: Arc<RwLock<ControlState>>,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    // Remove stale socket
    let _ = std::fs::remove_file(socket_path);

    let listener = UnixListener::bind(socket_path)?;
    tracing::info!(path = %socket_path.display(), "control socket listening");

    // Spawn event collector
    {
        let state = state.clone();
        let cancel = cancel.child_token();
        tokio::spawn(async move {
            collect_events(state, cancel).await;
        });
    }

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (stream, _) = match result {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(error = %e, "control accept error");
                        continue;
                    }
                };

                let state = state.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_client(stream, state).await {
                        tracing::debug!(error = %e, "control client error");
                    }
                });
            }
            _ = cancel.cancelled() => {
                tracing::info!("control server shutting down");
                break;
            }
        }
    }

    let _ = std::fs::remove_file(socket_path);
    Ok(())
}

async fn collect_events(state: Arc<RwLock<ControlState>>, cancel: CancellationToken) {
    let mut event_rx = {
        let s = state.read().await;
        match &s.engine_handle {
            Some(h) => h.subscribe_events(),
            None => return,
        }
    };

    loop {
        tokio::select! {
            result = event_rx.recv() => {
                match result {
                    Ok(event) => {
                        let info = event_to_info(&event);
                        let s = state.read().await;
                        let mut events = s.recent_events.lock().await;
                        events.push(info);
                        if events.len() > MAX_EVENTS_BUFFER {
                            let excess = events.len() - MAX_EVENTS_BUFFER;
                            events.drain(0..excess);
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::debug!(skipped = n, "event collector lagged");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = cancel.cancelled() => break,
        }
    }
}

async fn handle_client(
    mut stream: tokio::net::UnixStream,
    state: Arc<RwLock<ControlState>>,
) -> anyhow::Result<()> {
    loop {
        // Read length prefix (4 bytes BE)
        let mut len_buf = [0u8; 4];
        match stream.read_exact(&mut len_buf).await {
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::UnexpectedEof => return Ok(()),
            Err(e) => return Err(e.into()),
        }
        let msg_len = u32::from_be_bytes(len_buf) as usize;
        if msg_len > 1024 * 1024 {
            return Err(anyhow::anyhow!("message too large: {msg_len}"));
        }

        let mut msg_buf = vec![0u8; msg_len];
        stream.read_exact(&mut msg_buf).await?;

        let request: Request = serde_json::from_slice(&msg_buf)?;
        let response = process_request(request, &state).await;

        let resp_bytes = serde_json::to_vec(&response)?;
        stream.write_all(&(resp_bytes.len() as u32).to_be_bytes()).await?;
        stream.write_all(&resp_bytes).await?;
    }
}

async fn process_request(
    request: Request,
    state: &Arc<RwLock<ControlState>>,
) -> Response {
    match request {
        Request::Ping => Response::Pong,

        Request::Stop => {
            let s = state.read().await;
            s.cancel.cancel();
            Response::Ok
        }

        Request::Status => {
            let s = state.read().await;
            let uptime = s.start_time.elapsed().as_secs();
            Response::Status(StatusInfo {
                running: s.engine_handle.is_some(),
                uptime_secs: uptime,
                rules_count: s.rules_info.len(),
                config_path: s.config_path.display().to_string(),
            })
        }

        Request::Stats => {
            let s = state.read().await;
            match &s.engine_handle {
                Some(handle) => {
                    let snapshots: Vec<RuleStatsInfo> = handle
                        .stats_snapshot()
                        .into_iter()
                        .map(RuleStatsInfo::from)
                        .collect();
                    Response::Stats(snapshots)
                }
                None => Response::Error {
                    message: "engine not running".into(),
                },
            }
        }

        Request::RulesList => {
            let s = state.read().await;
            Response::Rules(s.rules_info.clone())
        }

        Request::Events { count } => {
            let s = state.read().await;
            let events = s.recent_events.lock().await;
            let n = count.unwrap_or(50).min(events.len());
            let tail = events[events.len() - n..].to_vec();
            Response::Events(tail)
        }

        Request::Reload { config_path } => {
            let s = state.read().await;
            let path = config_path
                .map(PathBuf::from)
                .unwrap_or_else(|| s.config_path.clone());
            match s.reload_tx.try_send(path) {
                Ok(_) => Response::Ok,
                Err(_) => Response::Error {
                    message: "reload already in progress".into(),
                },
            }
        }
    }
}

fn event_to_info(event: &Event) -> EventInfo {
    let timestamp_ms = event
        .timestamp
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    let (kind, detail) = match &event.kind {
        EventKind::IncomingData { rule, src, len } => {
            ("incoming_data".into(), format!("rule={rule} src={src} len={len}"))
        }
        EventKind::OutgoingData { rule, dst, len } => {
            ("outgoing_data".into(), format!("rule={rule} dst={dst} len={len}"))
        }
        EventKind::FilterMatch { rule, filter_index } => {
            ("filter_match".into(), format!("rule={rule} idx={filter_index}"))
        }
        EventKind::FilterDrop { rule, filter_index } => {
            ("filter_drop".into(), format!("rule={rule} idx={filter_index}"))
        }
        EventKind::UdpSessionCreated { rule, client } => {
            ("udp_session_created".into(), format!("rule={rule} client={client}"))
        }
        EventKind::UdpSessionTimeout { rule, client } => {
            ("udp_session_timeout".into(), format!("rule={rule} client={client}"))
        }
        EventKind::TcpConnectionOpened { rule, client } => {
            ("tcp_conn_opened".into(), format!("rule={rule} client={client}"))
        }
        EventKind::TcpConnectionClosed { rule, client } => {
            ("tcp_conn_closed".into(), format!("rule={rule} client={client}"))
        }
        EventKind::ExporterError { rule, detail } => {
            ("exporter_error".into(), format!("rule={rule} {detail}"))
        }
        EventKind::RuleActivated { rule } => {
            ("rule_activated".into(), format!("rule={rule}"))
        }
    };

    EventInfo {
        timestamp_ms,
        kind,
        detail,
    }
}
