# Harpoon — MITM/Traffic Analysis Proxy

## What is Harpoon

Harpoon is a Linux MITM/traffic-analysis proxy written in Rust. It intercepts, redirects, duplicates, filters and captures TCP/UDP traffic. It supports TLS MITM, nftables integration, and has a pipeline-based traffic processing engine.

**Repository:** ~/programming/harpoon

## Architecture

```
crates/
  harpoon-core/     # Library — no CLI/web/config deps
    src/
      capture/        # CaptureManager + MetricsCollector
      engine/         # TCP/UDP executors, DAG executor, filter engine
      pipeline/       # DAG model, validate, compile, simulate, compat
      export/         # UDS, TCP framed, Pipe exporters
      tls/            # TLS MITM (rustls + rcgen)
      types/          # Endpoint, Rule, Pipeline, Filter, Event, Stats
  harpoon-app/      # Binary — CLI, TOML config, daemon, web UI
    src/
      config/         # AppConfig schema (Serialize+Deserialize), loader
      control/        # UDS control socket (JSON protocol)
      daemon/         # Foreground/background, PID, reload
      nft/            # nftables render + apply via subprocess
      ui/web/         # Axum web server, static file serving (include_dir)
  harpoon-web/      # Svelte 5 SPA (built separately, embedded in binary)
```

## Pipeline Model

Traffic processing is defined as a DAG of nodes:

```
Pipeline { nodes: [Node], edges: [Edge] }
         │
         ▼ compile()
ExecutionPlan:
  - Tier 0 FastForward: Source→Forward (copy_bidirectional, zero overhead)
  - Tier 1 Linear: Source→[TLS]→[Filter]→Forward (sequential)
  - Tier 2 DAG: branching via Router nodes (dag_executor)
```

**Node kinds:** Source, TlsTerminate, TlsInitiate, Filter, Forward, Duplicate, Export, Drop, Router

Legacy `Rule` structs are auto-converted to `Pipeline` via `rule_to_pipeline()`.

## Capture System

- `CaptureManager` (Arc, shared) — zero overhead when inactive
- Per-rule on-demand: `start(rule, max_packets, max_payload, timeout)`
- Ring buffer `VecDeque<CapturedPacket>` with eviction
- `CapturedPacket`: timestamp_ms, rule_name, direction (c2s/s2c), src/dst SocketAddr, payload Vec<u8>
- Broadcast channel for WebSocket live streaming
- Preserved across engine reloads

## Exporter Protocol

Length-prefixed binary frames:
```
[4 bytes: frame_len (BE u32)]
[1 byte: version (0x01)]
[1 byte: event_kind]
[8 bytes: timestamp_ms (BE u64)]
[2 bytes + N bytes: rule_name (len-prefixed)]
[2 bytes + N bytes: detail (len-prefixed)]
```

Three exporter types:
- `Uds { path }` — Unix domain socket
- `TcpFramed { addr }` — TCP with length-prefix framing
- `Pipe { command, args }` — spawn process, write to stdin ← **use this for Red Eye**

## Web API (requires Bearer token auth)

**Data:**
- `GET /api/status` — { running, uptime_secs, rules_count, config_path }
- `GET /api/stats` — per-rule stats array
- `GET /api/rules` / `GET /api/rules/full` — rule summaries / full AppRule objects
- `GET /api/events` — recent events (last 500)
- `GET /api/metrics/global` — time-series metrics (bytes/sec, packets/sec, drops/sec)
- `GET /api/metrics/rule?rule=X` — per-rule metrics

**Rule CRUD:**
- `POST /api/rules/create` / `update` / `delete`

**Pipeline CRUD:**
- `POST /api/pipelines/create` / `update` / `delete`
- `POST /api/pipelines/validate` — returns { valid, tier, errors }
- `POST /api/pipelines/simulate` — walk DAG with sample payload, returns step trace

**Capture:**
- `POST /api/capture/start` — { rule, max_packets, max_payload_size, timeout_secs }
- `POST /api/capture/stop` — { rule }
- `GET /api/capture/packets?rule=X&offset=0&limit=100` — hex + text payload
- `GET /api/capture/ws` — WebSocket live packet stream (JSON)

**Config:**
- `GET /api/config/toml` — raw TOML
- `POST /api/reload` / `POST /api/stop`

## Integration with Red Eye

### Option 1: Pipe Exporter (recommended)
Configure a rule with pipe exporter in TOML:
```toml
[[rules]]
name = "analyzed-traffic"
protocol = "tcp"
listen = "0.0.0.0:8080"
target = "10.0.0.1:80"

[rules.exporter]
kind = "pipe"
command = "red-eye"
args = ["--stdin", "--format", "harpoon-framed"]
```
Harpoon spawns Red Eye as a subprocess, writes framed events to its stdin.

### Option 2: TCP Framed Exporter
Red Eye listens on a TCP port, Harpoon connects and streams:
```toml
[rules.exporter]
kind = "tcp"
addr = "127.0.0.1:4000"
```

### Option 3: Capture API
Red Eye polls or WebSocket-connects to Harpoon's capture API:
```
POST /api/capture/start { "rule": "web-proxy", "timeout_secs": 0 }
GET  /api/capture/ws  → WebSocket stream of { timestamp_ms, direction, src, dst, payload_hex, payload_text }
```

## Key Types for Integration

```rust
// Captured packet (from capture/manager.rs)
pub struct CapturedPacket {
    pub timestamp_ms: u64,
    pub rule_name: String,
    pub direction: PacketDirection,  // ClientToServer | ServerToClient
    pub src: SocketAddr,
    pub dst: SocketAddr,
    pub payload_len: usize,
    pub payload: Vec<u8>,  // truncated to max_payload_size
}

// Metric point (from capture/metrics.rs)
pub struct MetricPoint {
    pub timestamp_ms: u64,
    pub bytes_in_rate: u64,   // bytes/sec client→server
    pub bytes_out_rate: u64,  // bytes/sec server→client
    pub packets_in_rate: u64,
    pub packets_out_rate: u64,
    pub tcp_connections: u64,
    pub udp_sessions: u64,
    pub drops_rate: u64,
}
```

## Build

```bash
# Rust only (no web UI)
cargo build --release

# With web UI
cd crates/harpoon-web && npm install && cd ../..
./scripts/build-web-local.sh
cargo build --release --features web

# All features
cargo build --release --features "tls,regex-filter,web,transparent-udp"
```

## Config Example

```toml
[global]
web_bind = "127.0.0.1:8888"
web_password = "secret"

[[rules]]
name = "web-proxy"
protocol = "tcp"
listen = "0.0.0.0:8080"
target = "10.0.0.1:80"

[[rules.filters]]
kind = "substr"
pattern = "blocked"
direction = "c2s"
action = "drop"
```
