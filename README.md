# Harpoon

MITM/traffic-analysis proxy for Linux, written in Rust.

Harpoon intercepts, redirects, duplicates and filters TCP/UDP traffic. It works as a standalone userspace proxy and optionally integrates with `nftables` for transparent interception.

## Features

- **TCP proxy** — forward, duplicate, filter, TLS MITM
- **UDP relay** — session table with idle timeouts, datagram boundary preservation
- **Payload filters** — substring, binary pattern, regex (feature-gated)
- **Traffic duplication** — mirror to a secondary endpoint
- **TLS MITM** — terminate, re-encrypt, access plaintext for filters (feature-gated)
- **nftables integration** — REDIRECT, DNAT, TPROXY via subprocess `nft`
- **Transparent UDP source-preserving** — upstream sees original client IP (feature-gated)
- **Export** — events/data to Unix domain socket or TCP framed endpoint
- **Control plane** — Unix domain socket, CLI commands, hot config reload
- **Web UI** — dashboard with stats, rules, events (feature-gated)
- **Daemon mode** — background daemonization, PID file, signal handling
- **Library + binary** — `harpoon-core` embeddable in other Rust projects

## Architecture

```
harpoon/
  crates/
    harpoon-core/    # Library — engine, types, filters, exporters
    harpoon-app/     # Binary — CLI, config, daemon, nft, web UI
```

`harpoon-core` accepts a strictly typed `CoreConfig` and knows nothing about CLI, TOML, or web frameworks. `harpoon-app` handles all user-facing concerns and converts `AppConfig` → `CoreConfig`.

## Quick Start

```bash
# Build
cargo build --release

# Run with config
./target/release/harpoon run -c config.toml

# Run as daemon
./target/release/harpoon run -c config.toml -d

# Check status
./target/release/harpoon status

# View stats
./target/release/harpoon stats

# List rules
./target/release/harpoon rules

# View recent events
./target/release/harpoon events

# Reload config without restart
./target/release/harpoon reload

# Stop daemon
./target/release/harpoon stop
```

## Configuration

TOML config file. See [docs/config.md](docs/config.md) for the full reference.

Minimal example:

```toml
[global]
buffer_size = 8192

[[rules]]
name = "web-proxy"
protocol = "tcp"
listen = "0.0.0.0:8080"
target = "10.0.0.1:80"

[[rules]]
name = "dns-relay"
protocol = "udp"
listen = "0.0.0.0:5353"
target = "8.8.8.8:53"
idle_timeout_secs = 30
```

## Web UI

Harpoon includes a built-in web dashboard. It requires the `web` feature at build time.

**1. Build with the `web` feature:**

```bash
cargo build --release --features web
```

**2. Add `web_bind` to your config:**

```toml
[global]
web_bind = "127.0.0.1:8888"
```

**3. Start harpoon:**

```bash
./target/release/harpoon run -c config.toml
```

**4. Open in browser:**

```
http://127.0.0.1:8888
```

The dashboard auto-refreshes every 3 seconds and shows:
- Status (uptime, rule count, config path)
- Active rules (protocol, listen/target addresses, filter count)
- Live stats (bytes, packets, connections, sessions, drops)
- Recent events (timestamps, event types, details)

REST API endpoints are also available:
- `GET /api/status` — daemon status
- `GET /api/stats` — per-rule traffic stats
- `GET /api/rules` — active rule list
- `GET /api/events` — recent events (last 100)

By default `web_bind` should point to `127.0.0.1` for security. To expose externally, use `0.0.0.0:8888` (not recommended without additional access control).

## Optional Features

Build with features for additional capabilities:

```bash
# TLS MITM support
cargo build --release --features tls

# Regex payload filters
cargo build --release --features regex-filter

# Web UI dashboard
cargo build --release --features web

# Transparent UDP source-preserving (requires CAP_NET_ADMIN)
cargo build --release --features transparent-udp

# All features
cargo build --release --features "tls,regex-filter,web,transparent-udp"
```

## Documentation

- [Configuration Reference](docs/config.md) — all TOML settings, filters, TLS, nftables, web UI
- [Architecture Overview](docs/architecture.md) — crate structure, design decisions, feature flags
- [nftables Integration](docs/nftables.md) — REDIRECT, DNAT, TPROXY setup
- [Example Config](docs/example-config.toml) — annotated config with all options

## Testing

```bash
cargo test
```

## Requirements

- Linux (nftables features require `nft` command)
- Rust 1.75+
- `CAP_NET_ADMIN` for transparent UDP source-preserving mode

## License

MIT
