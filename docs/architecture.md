# Architecture

## Crate Structure

```
harpoon-core (library)          harpoon-app (binary)
├── config.rs                   ├── main.rs
├── error.rs                    ├── cli.rs (clap)
├── types/                      ├── config/ (TOML schema, loader)
│   ├── endpoint.rs             ├── convert.rs (AppConfig → CoreConfig)
│   ├── rule.rs                 ├── control/ (UDS protocol, server, client)
│   ├── filter.rs               ├── daemon/ (foreground/background, PID, state)
│   ├── event.rs                ├── nft/ (nftables render + apply)
│   └── stats.rs                └── ui/web/ (axum REST API + dashboard)
├── engine/
│   ├── mod.rs (orchestrator)
│   ├── tcp.rs
│   ├── udp.rs
│   ├── udp_transparent.rs
│   └── filter.rs
├── tls/
│   ├── cert.rs (CA, leaf cert generation)
│   └── mitm.rs (TLS MITM handler)
└── export/
    └── sink.rs (UDS, TCP framed)
```

## Key Design Decisions

### Library / Binary Separation

`harpoon-core` has zero knowledge of CLI, TOML, web frameworks, or daemonization. It accepts a `CoreConfig` struct and returns an `EngineHandle`. This allows embedding in other Rust projects.

`harpoon-app` owns all user-facing logic: parsing TOML, CLI argument handling, nftables subprocess management, daemon lifecycle, web UI.

### Async Runtime

Uses tokio multi-threaded runtime. The binary owns `#[tokio::main]`; the library spawns tasks onto the current runtime.

### TCP Proxy

Each accepted connection spawns a task. Two paths exist:

1. **Fast path** — no filters and no duplicate configured: uses `tokio::io::copy_bidirectional` for zero-copy bidirectional proxying. Stats update on connection close.
2. **Filter path** — manual read/write loop with per-chunk filter evaluation, stats tracking, and optional duplication.

### UDP Relay

Session-based model. Each unique client address gets a session with:
- A dedicated upstream socket (`connect()`ed to target)
- A spawned receive task for the reverse path
- An idle timeout tracked via `Instant::now()`

Session table uses `DashMap` for lock-free concurrent access (replacing the original `Mutex<HashMap>`). Cleanup runs every 5 seconds, evicting expired sessions.

### TLS MITM

On TLS connection accept:
1. Peek at ClientHello to extract SNI (without consuming the stream)
2. Generate a leaf certificate for that SNI signed by the configured CA
3. Cache `ServerConfig` per SNI for reuse
4. Accept TLS from client, optionally establish TLS to upstream
5. Proxy plaintext through the filter pipeline

### Control Plane

JSON-over-UDS with length-prefix framing (4 bytes big-endian + JSON payload). The daemon runs a control server that handles concurrent clients. CLI commands connect as clients.

### Event System

Events flow through `tokio::sync::broadcast` channel. The control server collects events into a bounded ring buffer (1000 entries). Exporters receive events via `mpsc` channel with backpressure tracking (`export_drops` stat counter).

### nftables Integration

Generates nftables ruleset as text, applies via `nft -f -` subprocess. Supports REDIRECT, DNAT, and TPROXY actions. Rollback on failure: deletes the `harpoon` table. TPROXY requires additional `ip rule` and `ip route` setup for fwmark-based routing.

### Transparent UDP Source-Preserving

Uses `IP_TRANSPARENT` and `IP_FREEBIND` socket options to bind the upstream socket to the client's source address. The upstream server sees the real client IP. Requires `CAP_NET_ADMIN`. Behind the `transparent-udp` feature flag.

## Feature Flags

| Feature | Crate | Description |
|---|---|---|
| `regex-filter` | core | Regex payload filter via `regex` crate |
| `tls` | core | TLS MITM via rustls + rcgen |
| `transparent-udp` | core | UDP source-preserving via IP_TRANSPARENT |
| `web` | app | Web UI via axum |
| `regex-filter` | app | Forwards to core |
| `tls` | app | Forwards to core |
| `transparent-udp` | app | Forwards to core |
