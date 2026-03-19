# nftables Integration

Harpoon can use nftables for kernel-level traffic steering. This enables:

- Intercepting traffic to already-occupied ports
- Transparent proxy with original destination preservation
- TPROXY-based capture

## Requirements

- Linux with nftables support
- `nft` command available in PATH
- Root privileges (or `CAP_NET_ADMIN`)

## How It Works

Harpoon generates an nftables ruleset as text and applies it via `nft -f -` subprocess. All rules live in a dedicated `table ip harpoon` to avoid conflicts with existing firewall configuration.

On shutdown, the table is deleted automatically.

## Supported Actions

### REDIRECT

Redirects traffic arriving on a specific port to a local Harpoon listener.

```toml
[global.nft]
enabled = true

[[global.nft.rules]]
protocol = "tcp"
match_dport = 80
action = "redirect"
to_port = 8080
comment = "redirect HTTP to proxy"
```

Generated nftables:
```
table ip harpoon {
  chain harpoon_prerouting {
    type nat hook prerouting priority dstnat; policy accept;
    tcp dport 80 redirect to :8080 comment "redirect HTTP to proxy"
  }
  chain harpoon_output {
    type nat hook output priority dstnat; policy accept;
    tcp dport 80 redirect to :8080 comment "redirect HTTP to proxy"
  }
}
```

### DNAT

Rewrites the destination address of matching traffic.

```toml
[[global.nft.rules]]
protocol = "udp"
match_dport = 53
match_dst = "10.0.0.1"
action = "dnat"
to_addr = "10.0.0.5:5353"
```

### TPROXY

Transparent proxy mode. Requires additional routing setup (handled automatically by Harpoon).

```toml
[global.nft]
enabled = true
tproxy_mark = 1

[[global.nft.rules]]
protocol = "tcp"
match_dport = 443
action = "tproxy"
to_port = 8443
```

This generates mangle chain rules with `tproxy` target and sets up:
- `ip rule add fwmark 0x1 lookup 100`
- `ip route add local 0.0.0.0/0 dev lo table 100`

Both are cleaned up on shutdown.

## Rollback

If `nft -f` fails, Harpoon attempts to delete the `harpoon` table to avoid leaving partial rules. The engine will not start if nftables rules fail to apply.

## Without nftables

If `nft.enabled = false` (default) or `nft` is not installed, Harpoon works as a standard userspace proxy. No kernel hooks are installed.

## Transparent UDP Source-Preserving

When using TPROXY for UDP and the `transparent-udp` feature:

```toml
[[rules]]
name = "dns-transparent"
protocol = "udp"
listen = "0.0.0.0:5353"
target = "8.8.8.8:53"
udp_source_mode = "preserve"
```

The upstream DNS server will see the original client's IP address as the source, not Harpoon's IP. This requires:
- `CAP_NET_ADMIN`
- Build with `--features transparent-udp`
- nftables TPROXY rules for the return path
