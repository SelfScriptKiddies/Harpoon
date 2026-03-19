use std::collections::HashSet;
use std::sync::Arc;

use axum::extract::{Path as AxumPath, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use axum::routing::{get, post};
use axum::Router;
use include_dir::{include_dir, Dir};
use tokio::sync::{Mutex, RwLock};
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;

use crate::control::proto::{EventInfo, RuleInfo, RuleStatsInfo, StatusInfo};
use crate::control::server::ControlState;

static STATIC_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/ui/web/static");

pub struct WebState {
    pub control: Arc<RwLock<ControlState>>,
    pub password: String,
    pub tokens: Mutex<HashSet<String>>,
    pub capture: Arc<harpoon_core::capture::CaptureManager>,
}

pub async fn run_web_server(
    bind_addr: std::net::SocketAddr,
    control: Arc<RwLock<ControlState>>,
    password: String,
    capture: Arc<harpoon_core::capture::CaptureManager>,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let state = Arc::new(WebState {
        control,
        password,
        tokens: Mutex::new(HashSet::new()),
        capture,
    });

    let app = Router::new()
        // Public
        .route("/api/auth/login", post(api_login))
        // Protected API
        .route("/api/status", get(api_status))
        .route("/api/stats", get(api_stats))
        .route("/api/rules", get(api_rules))
        .route("/api/rules/full", get(api_rules_full))
        .route("/api/rules/create", post(api_rule_create))
        .route("/api/rules/update", post(api_rule_update))
        .route("/api/rules/delete", post(api_rule_delete))
        .route("/api/pipelines", get(api_pipelines))
        .route("/api/pipelines/create", post(api_pipeline_create))
        .route("/api/pipelines/update", post(api_pipeline_update))
        .route("/api/pipelines/delete", post(api_pipeline_delete))
        .route("/api/pipelines/validate", post(api_pipeline_validate))
        .route("/api/events", get(api_events))
        .route("/api/config/toml", get(api_config_toml))
        .route("/api/nft/status", get(api_nft_status))
        .route("/api/nft/preview", get(api_nft_preview))
        .route("/api/nft/apply", post(api_nft_apply))
        .route("/api/nft/rollback", post(api_nft_rollback))
        .route("/api/capture/start", post(api_capture_start))
        .route("/api/capture/stop", post(api_capture_stop))
        .route("/api/capture/packets", get(api_capture_packets))
        .route("/api/capture/sessions", get(api_capture_sessions))
        .route("/api/capture/ws", get(api_capture_ws))
        .route("/api/reload", post(api_reload))
        .route("/api/stop", post(api_stop))
        // Static files
        .route("/", get(index_page))
        .route("/{*path}", get(static_file))
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

// --- Static file serving ---

async fn index_page() -> impl IntoResponse {
    serve_static("index.html")
}

async fn static_file(AxumPath(path): AxumPath<String>) -> impl IntoResponse {
    serve_static(&path)
}

fn serve_static(path: &str) -> Response {
    match STATIC_DIR.get_file(path) {
        Some(file) => {
            let mime = match path.rsplit('.').next() {
                Some("html") => "text/html; charset=utf-8",
                Some("css") => "text/css; charset=utf-8",
                Some("js") => "application/javascript; charset=utf-8",
                Some("json") => "application/json",
                Some("svg") => "image/svg+xml",
                Some("png") => "image/png",
                _ => "application/octet-stream",
            };
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, HeaderValue::from_static(mime))
                .body(axum::body::Body::from(file.contents().to_vec()))
                .unwrap()
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(axum::body::Body::from("Not found"))
            .unwrap(),
    }
}

// --- Auth ---

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

// --- Data API ---

async fn api_status(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<StatusInfo>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    Ok(Json(StatusInfo {
        running: s.engine_handle.is_some(),
        uptime_secs: s.start_time.elapsed().as_secs(),
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
        Some(handle) => Ok(Json(
            handle.stats_snapshot().into_iter().map(RuleStatsInfo::from).collect(),
        )),
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

async fn api_rules_full(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::config::schema::AppRule>>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    match &s.app_config {
        Some(cfg) => Ok(Json(cfg.rules.clone())),
        None => Ok(Json(vec![])),
    }
}

async fn api_events(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<EventInfo>>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    let events = s.recent_events.lock().await;
    let n = 500.min(events.len());
    Ok(Json(events[events.len() - n..].to_vec()))
}

async fn api_config_toml(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    match &s.app_config {
        Some(cfg) => {
            let toml_str = toml::to_string_pretty(cfg).unwrap_or_else(|e| format!("# Error: {e}"));
            Ok(Json(serde_json::json!({ "toml": toml_str })))
        }
        None => Ok(Json(serde_json::json!({ "toml": "# No config loaded" }))),
    }
}

// --- nftables API ---

async fn api_nft_status(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    let available = crate::nft::apply::check_nft_available();
    Ok(Json(serde_json::json!({ "available": available })))
}

async fn api_nft_preview(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    match &s.app_config {
        Some(cfg) if cfg.global.nft.enabled => {
            let nft_rules = build_nft_rules(&cfg.global.nft);
            let has_tproxy = nft_rules.iter().any(|r| matches!(r.action, crate::nft::render::NftAction::Tproxy { .. }));
            let ruleset = if has_tproxy {
                let mark = cfg.global.nft.tproxy_mark.unwrap_or(0x1);
                crate::nft::render::render_tproxy_install(&nft_rules, mark)
            } else {
                crate::nft::render::render_install(&nft_rules)
            };
            Ok(Json(serde_json::json!({ "ruleset": ruleset })))
        }
        _ => Ok(Json(serde_json::json!({ "ruleset": "# nft not enabled in config" }))),
    }
}

async fn api_nft_apply(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    match &s.app_config {
        Some(cfg) if cfg.global.nft.enabled => {
            let nft_rules = build_nft_rules(&cfg.global.nft);
            let ruleset = crate::nft::render::render_install(&nft_rules);
            match crate::nft::apply::apply_with_rollback(&ruleset) {
                Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
                Err(e) => Ok(Json(serde_json::json!({ "ok": false, "error": e.to_string() }))),
            }
        }
        _ => Ok(Json(serde_json::json!({ "ok": false, "error": "nft not enabled" }))),
    }
}

async fn api_nft_rollback(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    match crate::nft::apply::cleanup_table() {
        Ok(_) => Ok(Json(serde_json::json!({ "ok": true }))),
        Err(e) => Ok(Json(serde_json::json!({ "ok": false, "error": e.to_string() }))),
    }
}

fn build_nft_rules(nft_cfg: &crate::config::schema::NftConfig) -> Vec<crate::nft::render::NftRule> {
    nft_cfg
        .rules
        .iter()
        .filter_map(|r| {
            let protocol = match r.protocol.to_lowercase().as_str() {
                "tcp" => crate::nft::render::NftProtocol::Tcp,
                "udp" => crate::nft::render::NftProtocol::Udp,
                _ => return None,
            };
            let match_dst = r.match_dst.as_ref().and_then(|s| s.parse().ok());
            let action = match r.action.to_lowercase().as_str() {
                "redirect" => crate::nft::render::NftAction::Redirect { to_port: r.to_port? },
                "dnat" => crate::nft::render::NftAction::Dnat {
                    to_addr: r.to_addr.as_ref()?.parse().ok()?,
                },
                "tproxy" => crate::nft::render::NftAction::Tproxy {
                    to_port: r.to_port?,
                    mark: nft_cfg.tproxy_mark.unwrap_or(0x1),
                },
                _ => return None,
            };
            Some(crate::nft::render::NftRule {
                protocol,
                match_dport: r.match_dport,
                match_dst,
                action,
                comment: r.comment.clone(),
            })
        })
        .collect()
}

// --- Actions ---

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

// --- Rule CRUD ---

async fn api_rule_create(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(new_rule): Json<crate::config::schema::AppRule>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    modify_rules(&state, |rules| {
        if rules.iter().any(|r| r.name == new_rule.name) {
            return Err("rule with this name already exists".into());
        }
        rules.push(new_rule);
        Ok(())
    })
    .await
}

#[derive(serde::Deserialize)]
struct RuleUpdateReq {
    original_name: String,
    rule: crate::config::schema::AppRule,
}

async fn api_rule_update(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(req): Json<RuleUpdateReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    modify_rules(&state, |rules| {
        let idx = rules
            .iter()
            .position(|r| r.name == req.original_name)
            .ok_or_else(|| format!("rule '{}' not found", req.original_name))?;
        rules[idx] = req.rule;
        Ok(())
    })
    .await
}

#[derive(serde::Deserialize)]
struct RuleDeleteReq {
    name: String,
}

async fn api_rule_delete(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(req): Json<RuleDeleteReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    modify_rules(&state, |rules| {
        let idx = rules
            .iter()
            .position(|r| r.name == req.name)
            .ok_or_else(|| format!("rule '{}' not found", req.name))?;
        rules.remove(idx);
        Ok(())
    })
    .await
}

async fn modify_rules<F>(
    state: &Arc<WebState>,
    f: F,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    F: FnOnce(&mut Vec<crate::config::schema::AppRule>) -> Result<(), String>,
{
    let (mut app_config, config_path) = {
        let s = state.control.read().await;
        let cfg = s.app_config.clone().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        (cfg, s.config_path.clone())
    };

    if let Err(msg) = f(&mut app_config.rules) {
        return Ok(Json(serde_json::json!({"ok": false, "error": msg})));
    }

    if let Err(e) = crate::convert::convert(app_config.clone()) {
        return Ok(Json(serde_json::json!({"ok": false, "error": format!("validation: {e}")})));
    }

    if let Err(e) = crate::config::load::save_config(&config_path, &app_config) {
        return Ok(Json(serde_json::json!({"ok": false, "error": format!("save: {e}")})));
    }

    if let Err(e) =
        crate::daemon::run::apply_app_config(&state.control, app_config, &config_path).await
    {
        return Ok(Json(serde_json::json!({"ok": false, "error": format!("apply: {e}")})));
    }

    Ok(Json(serde_json::json!({"ok": true})))
}

// --- Pipeline CRUD ---

async fn api_pipelines(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<crate::config::schema::AppPipeline>>, StatusCode> {
    require_auth!(state, headers);
    let s = state.control.read().await;
    match &s.app_config {
        Some(cfg) => Ok(Json(cfg.pipelines.clone())),
        None => Ok(Json(vec![])),
    }
}

async fn api_pipeline_create(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(new_pipeline): Json<crate::config::schema::AppPipeline>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    modify_config(&state, |cfg| {
        if cfg.pipelines.iter().any(|p| p.id == new_pipeline.id) {
            return Err(format!("pipeline '{}' already exists", new_pipeline.id));
        }
        cfg.pipelines.push(new_pipeline);
        Ok(())
    })
    .await
}

#[derive(serde::Deserialize)]
struct PipelineUpdateReq {
    id: String,
    pipeline: crate::config::schema::AppPipeline,
}

async fn api_pipeline_update(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(req): Json<PipelineUpdateReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    modify_config(&state, |cfg| {
        let idx = cfg.pipelines.iter().position(|p| p.id == req.id)
            .ok_or_else(|| format!("pipeline '{}' not found", req.id))?;
        cfg.pipelines[idx] = req.pipeline;
        Ok(())
    })
    .await
}

#[derive(serde::Deserialize)]
struct PipelineDeleteReq {
    id: String,
}

async fn api_pipeline_delete(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(req): Json<PipelineDeleteReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    modify_config(&state, |cfg| {
        let idx = cfg.pipelines.iter().position(|p| p.id == req.id)
            .ok_or_else(|| format!("pipeline '{}' not found", req.id))?;
        cfg.pipelines.remove(idx);
        Ok(())
    })
    .await
}

async fn api_pipeline_validate(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(pipeline): Json<crate::config::schema::AppPipeline>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    match crate::convert::convert_pipeline_public(pipeline) {
        Ok(core_pipeline) => {
            match harpoon_core::pipeline::compile::compile(core_pipeline) {
                Ok(plan) => Ok(Json(serde_json::json!({
                    "valid": true, "tier": plan.tier(), "errors": [], "warnings": [],
                }))),
                Err(e) => Ok(Json(serde_json::json!({
                    "valid": false, "errors": [format!("{e}")],
                }))),
            }
        }
        Err(e) => Ok(Json(serde_json::json!({
            "valid": false, "errors": [format!("{e}")],
        }))),
    }
}

/// Generic config modifier: mutate AppConfig → validate → save → apply.
async fn modify_config<F>(
    state: &Arc<WebState>,
    f: F,
) -> Result<Json<serde_json::Value>, StatusCode>
where
    F: FnOnce(&mut crate::config::schema::AppConfig) -> Result<(), String>,
{
    let (mut app_config, config_path) = {
        let s = state.control.read().await;
        let cfg = s.app_config.clone().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;
        (cfg, s.config_path.clone())
    };

    if let Err(msg) = f(&mut app_config) {
        return Ok(Json(serde_json::json!({"ok": false, "error": msg})));
    }

    if let Err(e) = crate::convert::convert(app_config.clone()) {
        return Ok(Json(serde_json::json!({"ok": false, "error": format!("validation: {e}")})));
    }

    if let Err(e) = crate::config::load::save_config(&config_path, &app_config) {
        return Ok(Json(serde_json::json!({"ok": false, "error": format!("save: {e}")})));
    }

    if let Err(e) = crate::daemon::run::apply_app_config(&state.control, app_config, &config_path).await {
        return Ok(Json(serde_json::json!({"ok": false, "error": format!("apply: {e}")})));
    }

    Ok(Json(serde_json::json!({"ok": true})))
}

// --- Capture API ---

#[derive(serde::Deserialize)]
struct CaptureStartReq {
    rule: String,
    #[serde(default = "default_max_packets")]
    max_packets: usize,
    #[serde(default = "default_max_payload")]
    max_payload_size: usize,
    #[serde(default = "default_timeout")]
    timeout_secs: u64,
}
fn default_max_packets() -> usize { 1000 }
fn default_max_payload() -> usize { 4096 }
fn default_timeout() -> u64 { 300 }

async fn api_capture_start(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(req): Json<CaptureStartReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    match state.capture.start(req.rule, req.max_packets, req.max_payload_size, req.timeout_secs).await {
        Ok(()) => Ok(Json(serde_json::json!({"ok": true}))),
        Err(e) => Ok(Json(serde_json::json!({"ok": false, "error": e}))),
    }
}

#[derive(serde::Deserialize)]
struct CaptureStopReq { rule: String }

async fn api_capture_stop(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    Json(req): Json<CaptureStopReq>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    match state.capture.stop(&req.rule).await {
        Ok(packets) => Ok(Json(serde_json::json!({"ok": true, "packets_captured": packets.len()}))),
        Err(e) => Ok(Json(serde_json::json!({"ok": false, "error": e}))),
    }
}

#[derive(serde::Deserialize)]
struct CapturePacketsQuery {
    rule: String,
    #[serde(default)]
    offset: usize,
    #[serde(default = "default_limit")]
    limit: usize,
}
fn default_limit() -> usize { 100 }

async fn api_capture_packets(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
    axum::extract::Query(q): axum::extract::Query<CapturePacketsQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    let packets = state.capture.get_packets(&q.rule, q.offset, q.limit).await;
    let json_packets: Vec<serde_json::Value> = packets.iter().map(|p| {
        serde_json::json!({
            "timestamp_ms": p.timestamp_ms,
            "rule": p.rule_name,
            "direction": p.direction.as_str(),
            "src": p.src.to_string(),
            "dst": p.dst.to_string(),
            "payload_len": p.payload_len,
            "payload_hex": hex_encode(&p.payload),
            "payload_text": String::from_utf8_lossy(&p.payload),
        })
    }).collect();
    Ok(Json(serde_json::json!({ "packets": json_packets })))
}

async fn api_capture_sessions(
    State(state): State<Arc<WebState>>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    require_auth!(state, headers);
    let sessions = state.capture.list_sessions().await;
    let json: Vec<serde_json::Value> = sessions.iter().map(|s| {
        serde_json::json!({
            "rule": s.rule_name,
            "packets_captured": s.packets_captured,
            "max_packets": s.max_packets,
            "elapsed_secs": s.elapsed_secs,
            "timeout_secs": s.timeout_secs,
        })
    }).collect();
    Ok(Json(serde_json::json!({ "sessions": json })))
}

async fn api_capture_ws(
    State(state): State<Arc<WebState>>,
    ws: axum::extract::WebSocketUpgrade,
) -> impl IntoResponse {
    let capture = state.capture.clone();
    ws.on_upgrade(move |mut socket| async move {
        let mut rx = capture.subscribe();
        loop {
            match rx.recv().await {
                Ok(packet) => {
                    let msg = serde_json::json!({
                        "timestamp_ms": packet.timestamp_ms,
                        "rule": packet.rule_name,
                        "direction": packet.direction.as_str(),
                        "src": packet.src.to_string(),
                        "dst": packet.dst.to_string(),
                        "payload_len": packet.payload_len,
                        "payload_hex": hex_encode(&packet.payload),
                        "payload_text": String::from_utf8_lossy(&packet.payload),
                    });
                    if socket.send(axum::extract::ws::Message::Text(msg.to_string().into())).await.is_err() {
                        break;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

fn hex_encode(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect()
}
