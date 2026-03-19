# Configuration Reference

Harpoon uses TOML config files. All settings are documented below.

## Global Settings

```toml
[global]
buffer_size = 8192              # TCP read buffer size (bytes)
udp_max_datagram = 65507        # Max UDP datagram size (bytes)
shutdown_timeout_secs = 5       # Graceful shutdown deadline
web_bind = "127.0.0.1:8888"    # Web UI bind address (requires --features web)
```

## Web UI

Requires building with `--features web`.

```toml
[global]
web_bind = "127.0.0.1:8888"
web_password = "my-secret"      # optional — auto-generated if omitted
```

| Field | Type | Description |
|---|---|---|
| `web_bind` | `string` | Address and port for the web server. Omit to disable. |
| `web_password` | `string` | Password for web UI login (username is always `admin`). If omitted, a random password is generated and printed to the log on startup. |

### Authentication

The web UI requires authentication. On login page enter:
- **Username:** `admin`
- **Password:** from `web_password` in config, or auto-generated (check startup log)

A session token is returned on successful login and stored in the browser. All API endpoints (except `/api/auth/login`) require `Authorization: Bearer <token>` header.

### API endpoints

| Endpoint | Method | Auth | Description |
|---|---|---|---|
| `/` | GET | No | HTML dashboard |
| `/api/auth/login` | POST | No | Login, returns `{ "token": "..." }` |
| `/api/status` | GET | Yes | Daemon status |
| `/api/stats` | GET | Yes | Per-rule traffic statistics |
| `/api/rules` | GET | Yes | Active rule list |
| `/api/events` | GET | Yes | Recent events (last 200) |
| `/api/reload` | POST | Yes | Reload configuration |
| `/api/stop` | POST | Yes | Shutdown daemon |

### Pages

The dashboard includes 6 pages: Overview, Rules (with detail inspector), Sessions, Events (with pause/clear), Config, System.

The web server starts only when the `web` feature is compiled in **and** `web_bind` is set in config.

## Rules

Each rule defines a proxy endpoint.

```toml
[[rules]]
name = "my-rule"                # Unique rule name
protocol = "tcp"                # "tcp" or "udp"
listen = "0.0.0.0:8080"        # Listen address
target = "10.0.0.1:80"         # Upstream target address
duplicate = "10.0.0.2:8080"    # Optional: duplicate traffic target
idle_timeout_secs = 30          # UDP session idle timeout (default 30)
udp_source_mode = "proxy"       # "proxy" (default) or "preserve" (requires transparent-udp feature)
```

### Filters

Filters inspect payload and decide whether to pass, drop, or tap-only.

```toml
[[rules.filters]]
kind = "substr"                 # "substr", "bsubstr", or "regex" (requires regex-filter feature)
pattern = "blocked"             # Pattern to match
direction = "c2s"               # "c2s", "s2c", or "both" (default)
action = "drop"                 # "pass" (default), "drop", or "tap-only"
```

For `bsubstr`, the pattern is a hex string:

```toml
[[rules.filters]]
kind = "bsubstr"
pattern = "deadbeef"
action = "drop"
```

### Exporter

Export events to an external sink.

```toml
[rules.exporter]
kind = "uds"                    # "uds" or "tcp"
path = "/tmp/harpoon-export.sock"   # For UDS
# addr = "127.0.0.1:4000"      # For TCP framed
```

The export protocol is length-prefixed binary:
- 4 bytes: frame length (big-endian u32)
- 1 byte: version (0x01)
- 1 byte: event kind
- 8 bytes: timestamp (ms since epoch, big-endian u64)
- 2 bytes + N bytes: rule name (length-prefixed)
- 2 bytes + N bytes: event detail (length-prefixed)

### TLS MITM

Requires `--features tls`.

```toml
[rules.tls]
mode = "mitm"                   # "passthrough", "terminate", or "mitm"
ca_cert = "/etc/harpoon/ca.pem"
ca_key = "/etc/harpoon/ca-key.pem"
```

Modes:
- **passthrough** — no TLS processing, forward raw bytes
- **terminate** — accept TLS from client, connect to upstream in plaintext
- **mitm** — accept TLS from client, establish TLS to upstream, access plaintext for filters

Generate a CA for testing:

```bash
openssl req -x509 -newkey rsa:2048 -keyout ca-key.pem -out ca.pem -days 365 -nodes -subj '/CN=Harpoon CA'
```

## nftables

Optional kernel-level traffic steering. See [nftables.md](nftables.md).

```toml
[global.nft]
enabled = true
tproxy_mark = 1                 # fwmark for TPROXY routing

[[global.nft.rules]]
protocol = "tcp"
match_dport = 80
action = "redirect"             # "redirect", "dnat", or "tproxy"
to_port = 8080
comment = "redirect HTTP to proxy"

[[global.nft.rules]]
protocol = "udp"
match_dport = 53
match_dst = "10.0.0.1"          # Optional destination IP filter
action = "dnat"
to_addr = "10.0.0.5:5353"

[[global.nft.rules]]
protocol = "tcp"
match_dport = 443
action = "tproxy"
to_port = 8443
```

## CLI Options

```
harpoon run [OPTIONS]
  -c, --config <PATH>       Config file path [default: config.toml]
  -d, --daemon              Run as background daemon
      --pid-file <PATH>     PID file path [default: /tmp/harpoon.pid]

harpoon stop                Stop the running daemon
harpoon status              Show daemon status
harpoon stats               Show traffic statistics
harpoon rules               List active rules
harpoon events [-n COUNT]   Show recent events
harpoon reload [-c PATH]    Reload configuration

Global options:
  --socket <PATH>           Control socket path [default: /tmp/harpoon.sock]
```
