use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::State;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, Json};
use axum::routing::{get, post};
use axum::Router;
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;

use crate::control::proto::{EventInfo, RuleInfo, RuleStatsInfo, StatusInfo};
use crate::control::server::ControlState;

const INDEX_HTML: &str = include_str!("assets/index.html");

pub struct WebState {
    pub control: Arc<RwLock<ControlState>>,
    pub password: String,
    pub tokens: Mutex<HashSet<String>>,
}

pub async fn run_web_server(
    bind_addr: std::net::SocketAddr,
    control: Arc<RwLock<ControlState>>,
    password: String,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let state = Arc::new(WebState {
        control,
        password,
        tokens: Mutex::new(HashSet::new()),
    });

    let app = Router::new()
        // Public
        .route("/api/auth/login", post(api_login))
        // Protected
        .route("/api/status", get(api_status))
        .route("/api/stats", get(api_stats))
        .route("/api/rules", get(api_rules))
        .route("/api/events", get(api_events))
        .route("/api/reload", post(api_reload))
        .route("/api/stop", post(api_stop))
        .route("/", get(index_page))
        .layer(CorsLayer::permissive())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(bind_addr).await?;
    tracing::info!(%bind_addr, "web UI listening");

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            cancel.cancelled().await;
        })
        .await?;

    Ok(())
}

fn check_auth(headers: &HeaderMap, tokens: &HashSet<String>) -> bool {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|t| tokens.contains(t))
        .unwrap_or(false)
}

fn gen_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{:x}{:x}", ts, std::process::id())
}

#[derive(serde::Deserialize)]
struct LoginReq {
    username: String,
    password: String,
}

#[derive(serde::Serialize)]
struct LoginResp {
    token: String,
}

async fn api_login(
    State(state): State<Arc<WebState>>,
    Json(req): Json<LoginReq>,
) -> Result<Json<LoginResp>, StatusCode> {
    if req.username == "admin" && req.password == state.password {
        let token = gen_token();
        state.tokens.lock().await.insert(token.clone());
        Ok(Json(LoginResp { token }))
    } else {
        Err(StatusCode::UNAUTHORIZED)
    }
}

macro_rules! require_auth {
    ($state:expr, $headers:expr) => {{
        let tokens = $state.tokens.lock().await;
        if !check_auth(&$headers, &tokens) {
            return Err(StatusCode::UNAUTHORIZED);
        }
    }};
}

async fn api_status(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<StatusInfo>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    let uptime = s.start_time.elapsed().as_secs();
    Ok(Json(StatusInfo {
        running: s.engine_handle.is_some(),
        uptime_secs: uptime,
        rules_count: s.rules_info.len(),
        config_path: s.config_path.display().to_string(),
    }))
}

async fn api_stats(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<RuleStatsInfo>>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    match &s.engine_handle {
        Some(handle) => {
            let stats: Vec<RuleStatsInfo> = handle
                .stats_snapshot()
                .into_iter()
                .map(RuleStatsInfo::from)
                .collect();
            Ok(Json(stats))
        }
        None => Ok(Json(vec![])),
    }
}

async fn api_rules(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<RuleInfo>>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    Ok(Json(s.rules_info.clone()))
}

async fn api_events(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<EventInfo>>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    let events = s.recent_events.lock().await;
    let n = 200.min(events.len());
    let tail = events[events.len() - n..].to_vec();
    Ok(Json(tail))
}

async fn api_reload(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    let path = s.config_path.clone();
    match s.reload_tx.try_send(path) {
        Ok(_) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(_) => Ok(Json(serde_json::json!({"ok": false, "error": "reload already in progress"}))),
    }
}

async fn api_stop(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    s.cancel.cancel();
    Ok(Json(serde_json::json!({"ok": true})))
}

async fn index_page() -> Html<&'static str> {
    Html(INDEX_HTML)
}
