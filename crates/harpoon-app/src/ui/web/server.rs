use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tower_http::cors::CorsLayer;

use crate::control::proto::{RuleInfo, RuleStatsInfo, StatusInfo};
use crate::control::server::ControlState;

type AppState = Arc<RwLock<ControlState>>;

pub async fn run_web_server(
    bind_addr: std::net::SocketAddr,
    state: AppState,
    cancel: CancellationToken,
) -> anyhow::Result<()> {
    let app = Router::new()
        .route("/api/status", get(api_status))
        .route("/api/stats", get(api_stats))
        .route("/api/rules", get(api_rules))
        .route("/api/events", get(api_events))
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

async fn api_status(State(state): State<AppState>) -> Json<StatusInfo> {
    let s = state.read().await;
    let uptime = s.start_time.elapsed().as_secs();
    Json(StatusInfo {
        running: s.engine_handle.is_some(),
        uptime_secs: uptime,
        rules_count: s.rules_info.len(),
        config_path: s.config_path.display().to_string(),
    })
}

async fn api_stats(
    State(state): State<AppState>,
) -> Result<Json<Vec<RuleStatsInfo>>, StatusCode> {
    let s = state.read().await;
    match &s.engine_handle {
        Some(handle) => {
            let stats: Vec<RuleStatsInfo> = handle
                .stats_snapshot()
                .into_iter()
                .map(RuleStatsInfo::from)
                .collect();
            Ok(Json(stats))
        }
        None => Err(StatusCode::SERVICE_UNAVAILABLE),
    }
}

async fn api_rules(State(state): State<AppState>) -> Json<Vec<RuleInfo>> {
    let s = state.read().await;
    Json(s.rules_info.clone())
}

async fn api_events(
    State(state): State<AppState>,
) -> Json<Vec<crate::control::proto::EventInfo>> {
    let s = state.read().await;
    let events = s.recent_events.lock().await;
    let n = 100.min(events.len());
    let tail = events[events.len() - n..].to_vec();
    Json(tail)
}

async fn index_page() -> axum::response::Html<&'static str> {
    axum::response::Html(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Harpoon</title>
    <style>
        body { font-family: monospace; margin: 2em; background: #1a1a2e; color: #e0e0e0; }
        h1 { color: #0f3460; }
        .section { margin: 1em 0; padding: 1em; background: #16213e; border-radius: 8px; }
        table { border-collapse: collapse; width: 100%; }
        th, td { padding: 0.4em 1em; text-align: left; border-bottom: 1px solid #333; }
        th { color: #e94560; }
        .status { color: #0f3460; font-weight: bold; }
        #refresh { cursor: pointer; padding: 0.5em 1em; background: #e94560; color: white; border: none; border-radius: 4px; }
    </style>
</head>
<body>
    <h1>Harpoon Dashboard</h1>
    <button id="refresh" onclick="refresh()">Refresh</button>

    <div class="section" id="status-section"><h2>Status</h2><div id="status">Loading...</div></div>
    <div class="section" id="rules-section"><h2>Rules</h2><div id="rules">Loading...</div></div>
    <div class="section" id="stats-section"><h2>Stats</h2><div id="stats">Loading...</div></div>
    <div class="section" id="events-section"><h2>Recent Events</h2><div id="events">Loading...</div></div>

    <script>
    async function refresh() {
        try {
            const status = await (await fetch('/api/status')).json();
            document.getElementById('status').innerHTML =
                `Running: ${status.running} | Uptime: ${status.uptime_secs}s | Rules: ${status.rules_count} | Config: ${status.config_path}`;

            const rules = await (await fetch('/api/rules')).json();
            let rt = '<table><tr><th>Name</th><th>Proto</th><th>Listen</th><th>Target</th><th>Filters</th></tr>';
            rules.forEach(r => { rt += `<tr><td>${r.name}</td><td>${r.protocol}</td><td>${r.listen}</td><td>${r.target}</td><td>${r.filters_count}</td></tr>`; });
            rt += '</table>';
            document.getElementById('rules').innerHTML = rt;

            const stats = await (await fetch('/api/stats')).json();
            let st = '<table><tr><th>Rule</th><th>Bytes C→S</th><th>Bytes S→C</th><th>Pkts C→S</th><th>Pkts S→C</th><th>TCP</th><th>UDP</th><th>Dropped</th></tr>';
            stats.forEach(s => { st += `<tr><td>${s.rule_name}</td><td>${s.bytes_client_to_server}</td><td>${s.bytes_server_to_client}</td><td>${s.packets_client_to_server}</td><td>${s.packets_server_to_client}</td><td>${s.active_tcp_connections}</td><td>${s.active_udp_sessions}</td><td>${s.dropped_packets}</td></tr>`; });
            st += '</table>';
            document.getElementById('stats').innerHTML = st;

            const events = await (await fetch('/api/events')).json();
            let et = '<table><tr><th>Time</th><th>Kind</th><th>Detail</th></tr>';
            events.slice(-20).reverse().forEach(e => { et += `<tr><td>${new Date(e.timestamp_ms).toLocaleTimeString()}</td><td>${e.kind}</td><td>${e.detail}</td></tr>`; });
            et += '</table>';
            document.getElementById('events').innerHTML = et;
        } catch(e) { console.error(e); }
    }
    refresh();
    setInterval(refresh, 3000);
    </script>
</body>
</html>"#,
    )
}
