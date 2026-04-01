use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use harpoon_core::config::CoreConfig;
use harpoon_core::types::endpoint::{Endpoint, Protocol};
use harpoon_core::types::filter::{Direction, Filter, FilterAction, FilterKind};
use harpoon_core::types::pipeline as pipe;
use harpoon_core::types::rule::{
    DuplicateTarget, ExporterConfig, ExporterKind, Rule, TlsConfig, TlsMode, UdpSourceMode,
};

use crate::config::schema::{AppConfig, AppPipeline, AppRule};

pub fn convert(app: AppConfig) -> Result<CoreConfig> {
    let mut rules = Vec::new();
    for r in app.rules {
        let expanded = expand_range_rule(r)?;
        for er in expanded {
            rules.push(convert_rule(er)?);
        }
    }

    let mut pipelines = Vec::new();
    for p in app.pipelines {
        pipelines.push(convert_pipeline(p)?);
    }

    let mut config = CoreConfig {
        rules,
        pipelines,
        ..CoreConfig::default()
    };

    if let Some(bs) = app.global.buffer_size {
        config.buffer_size = bs;
    }
    if let Some(md) = app.global.udp_max_datagram {
        config.udp_max_datagram = md;
    }
    if let Some(st) = app.global.shutdown_timeout_secs {
        config.shutdown_timeout_secs = st;
    }

    Ok(config)
}

/// Parse "ip:port" or "ip:port_start-port_end" into (ip, port_start, port_end).
fn parse_addr_range(s: &str) -> Result<(String, u16, u16)> {
    // Find the last colon to split host and port part
    let colon_idx = s.rfind(':').context("address must contain ':'")?;
    let host = &s[..colon_idx];
    let port_part = &s[colon_idx + 1..];

    if let Some(dash_idx) = port_part.find('-') {
        let start: u16 = port_part[..dash_idx].parse().context("invalid port range start")?;
        let end: u16 = port_part[dash_idx + 1..].parse().context("invalid port range end")?;
        if end < start {
            bail!("port range end ({end}) must be >= start ({start})");
        }
        Ok((host.to_string(), start, end))
    } else {
        let port: u16 = port_part.parse().context("invalid port")?;
        Ok((host.to_string(), port, port))
    }
}

fn is_range_addr(s: &str) -> bool {
    s.rfind(':')
        .map(|i| s[i + 1..].contains('-'))
        .unwrap_or(false)
}

/// Expand a rule with port ranges into multiple rules.
/// "0.0.0.0:3000-3010" → "90.90.90.90:4000-4010" becomes 11 rules.
/// Single target port → fan-in (all listen ports forward to one target).
fn expand_range_rule(r: AppRule) -> Result<Vec<AppRule>> {
    let listen_is_range = is_range_addr(&r.listen);
    let target_is_range = is_range_addr(&r.target);

    if !listen_is_range && !target_is_range {
        return Ok(vec![r]);
    }

    let (listen_host, listen_start, listen_end) = parse_addr_range(&r.listen)
        .with_context(|| format!("invalid listen range in rule '{}'", r.name))?;
    let (target_host, target_start, target_end) = parse_addr_range(&r.target)
        .with_context(|| format!("invalid target range in rule '{}'", r.name))?;

    let listen_count = (listen_end - listen_start + 1) as usize;
    let target_count = (target_end - target_start + 1) as usize;

    // Validate range compatibility
    if target_count > 1 && listen_count != target_count {
        bail!(
            "port range mismatch in rule '{}': listen has {} ports ({}–{}), target has {} ports ({}–{})",
            r.name, listen_count, listen_start, listen_end, target_count, target_start, target_end
        );
    }

    let mut expanded = Vec::with_capacity(listen_count);
    for i in 0..listen_count {
        let listen_port = listen_start + i as u16;
        let target_port = if target_count == 1 {
            target_start // fan-in: all → one target
        } else {
            target_start + i as u16
        };

        let suffix = if listen_count > 1 {
            format!("/{listen_port}")
        } else {
            String::new()
        };

        expanded.push(AppRule {
            name: format!("{}{suffix}", r.name),
            protocol: r.protocol.clone(),
            listen: format!("{listen_host}:{listen_port}"),
            target: format!("{target_host}:{target_port}"),
            filters: r.filters.clone(),
            duplicate: r.duplicate.clone(),
            exporter: r.exporter.clone(),
            tls: r.tls.clone(),
            udp_source_mode: r.udp_source_mode.clone(),
            idle_timeout_secs: r.idle_timeout_secs,
        });
    }

    Ok(expanded)
}

fn convert_rule(r: AppRule) -> Result<Rule> {
    let protocol = match r.protocol.to_lowercase().as_str() {
        "tcp" => Protocol::Tcp,
        "udp" => Protocol::Udp,
        other => bail!("unknown protocol '{}' in rule '{}'", other, r.name),
    };

    let listen_addr: SocketAddr = r
        .listen
        .parse()
        .with_context(|| format!("invalid listen address '{}' in rule '{}'", r.listen, r.name))?;

    let target_addr: SocketAddr = r
        .target
        .parse()
        .with_context(|| format!("invalid target address '{}' in rule '{}'", r.target, r.name))?;

    let filters = r
        .filters
        .into_iter()
        .map(|f| convert_filter(f, &r.name))
        .collect::<Result<Vec<_>>>()?;

    let duplicate = match r.duplicate {
        Some(ref addr_str) => {
            let addr: SocketAddr = addr_str
                .parse()
                .with_context(|| format!("invalid duplicate address '{}' in rule '{}'", addr_str, r.name))?;
            Some(DuplicateTarget {
                endpoint: Endpoint { addr, protocol },
            })
        }
        None => None,
    };

    let exporter = match r.exporter {
        Some(ref exp) => {
            let kind = match exp.kind.to_lowercase().as_str() {
                "uds" | "unix" => {
                    let path = exp
                        .path
                        .as_ref()
                        .with_context(|| format!("uds exporter requires 'path' in rule '{}'", r.name))?;
                    ExporterKind::Uds {
                        path: PathBuf::from(path),
                    }
                }
                "tcp" | "tcp_framed" => {
                    let addr_str = exp
                        .addr
                        .as_ref()
                        .with_context(|| format!("tcp exporter requires 'addr' in rule '{}'", r.name))?;
                    let addr: SocketAddr = addr_str.parse().with_context(|| {
                        format!("invalid exporter addr '{}' in rule '{}'", addr_str, r.name)
                    })?;
                    ExporterKind::TcpFramed { addr }
                }
                other => bail!("unknown exporter kind '{}' in rule '{}'", other, r.name),
            };
            Some(ExporterConfig { kind })
        }
        None => None,
    };

    let tls = match r.tls {
        Some(ref tls_cfg) => {
            let mode = match tls_cfg.mode.to_lowercase().as_str() {
                "passthrough" => TlsMode::Passthrough,
                "terminate" => TlsMode::Terminate,
                "mitm" => TlsMode::Mitm,
                other => bail!("unknown TLS mode '{}' in rule '{}'", other, r.name),
            };
            Some(TlsConfig {
                mode,
                ca_cert_path: PathBuf::from(&tls_cfg.ca_cert),
                ca_key_path: PathBuf::from(&tls_cfg.ca_key),
            })
        }
        None => None,
    };

    let udp_source_mode = match r.udp_source_mode.as_deref() {
        Some("preserve") => UdpSourceMode::Preserve,
        Some("proxy") | None => UdpSourceMode::Proxy,
        Some(other) => bail!("unknown udp_source_mode '{}' in rule '{}'", other, r.name),
    };

    let idle_timeout_secs = r.idle_timeout_secs.unwrap_or(Rule::default_idle_timeout());

    Ok(Rule {
        name: r.name,
        listen: Endpoint {
            addr: listen_addr,
            protocol,
        },
        target: Endpoint {
            addr: target_addr,
            protocol,
        },
        filters,
        duplicate,
        exporter,
        tls,
        udp_source_mode,
        http2: false,
        idle_timeout_secs,
    })
}

pub fn convert_pipeline_public(p: AppPipeline) -> Result<pipe::Pipeline> {
    convert_pipeline(p)
}

fn convert_pipeline(p: AppPipeline) -> Result<pipe::Pipeline> {
    let mut nodes = Vec::new();
    for n in &p.nodes {
        let kind = convert_pipeline_node(n)
            .with_context(|| format!("node {} in pipeline '{}'", n.id, p.name))?;
        nodes.push(pipe::Node {
            id: n.id,
            label: n.label.clone().unwrap_or_else(|| n.kind.clone()),
            kind,
        });
    }

    let edges = p
        .edges
        .iter()
        .enumerate()
        .map(|(i, e)| pipe::Edge {
            id: i as u32 + 1,
            from_node: e.from,
            to_node: e.to,
            from_port: e.port.clone(),
        })
        .collect();

    Ok(pipe::Pipeline {
        id: p.id,
        name: p.name,
        nodes,
        edges,
    })
}

fn convert_pipeline_node(n: &crate::config::schema::AppPipelineNode) -> Result<pipe::NodeKind> {
    let c = &n.config;
    match n.kind.as_str() {
        "source" => {
            let proto = match c.get("protocol").and_then(|v| v.as_str()).unwrap_or("tcp") {
                "tcp" => Protocol::Tcp,
                "udp" => Protocol::Udp,
                other => bail!("unknown protocol: {other}"),
            };
            let listen: SocketAddr = c.get("listen").and_then(|v| v.as_str()).unwrap_or("0.0.0.0:0")
                .parse().context("invalid listen address")?;
            let udp_mode = match c.get("udp_source_mode").and_then(|v| v.as_str()) {
                Some("preserve") => UdpSourceMode::Preserve,
                _ => UdpSourceMode::Proxy,
            };
            let timeout = c.get("idle_timeout_secs").and_then(|v| v.as_u64()).unwrap_or(30);
            Ok(pipe::NodeKind::Source(pipe::SourceConfig {
                endpoint: Endpoint { addr: listen, protocol: proto },
                udp_source_mode: udp_mode,
                idle_timeout_secs: timeout,
            }))
        }
        "forward" => {
            let target: SocketAddr = c.get("target").and_then(|v| v.as_str()).unwrap_or("0.0.0.0:0")
                .parse().context("invalid target address")?;
            let proto = c.get("protocol").and_then(|v| v.as_str()).map(|p| match p {
                "udp" => Protocol::Udp, _ => Protocol::Tcp,
            }).unwrap_or(Protocol::Tcp);
            let nodelay = c.get("tcp_nodelay").and_then(|v| v.as_bool()).unwrap_or(true);
            Ok(pipe::NodeKind::Forward(pipe::ForwardConfig {
                endpoint: Endpoint { addr: target, protocol: proto },
                tcp_nodelay: nodelay,
            }))
        }
        "tls_terminate" => {
            let cert = c.get("ca_cert").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let key = c.get("ca_key").and_then(|v| v.as_str()).unwrap_or("").to_string();
            Ok(pipe::NodeKind::TlsTerminate(pipe::TlsTerminateConfig {
                ca_cert_path: PathBuf::from(cert),
                ca_key_path: PathBuf::from(key),
            }))
        }
        "tls_initiate" => {
            let verify = c.get("verify_certs").and_then(|v| v.as_bool()).unwrap_or(true);
            Ok(pipe::NodeKind::TlsInitiate(pipe::TlsInitiateConfig { verify_certs: verify }))
        }
        "filter" => {
            let kind_str = c.get("kind").and_then(|v| v.as_str()).unwrap_or("substr");
            let pattern = c.get("pattern").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let dir = match c.get("direction").and_then(|v| v.as_str()) {
                Some("c2s") => Direction::ClientToServer,
                Some("s2c") => Direction::ServerToClient,
                _ => Direction::Both,
            };
            let action = match c.get("action").and_then(|v| v.as_str()) {
                Some("pass") => FilterAction::Pass,
                Some("tap-only") | Some("tap_only") => FilterAction::TapOnly,
                Some("drop_connection") => FilterAction::DropConnection,
                _ => FilterAction::Drop,
            };
            let filter_kind = match kind_str {
                "substr" => FilterKind::Substr(pattern),
                "bsubstr" => {
                    let bytes = hex_decode(&pattern).context("invalid hex in bsubstr")?;
                    FilterKind::BinarySubstr(bytes)
                }
                #[cfg(feature = "regex-filter")]
                "regex" => FilterKind::Regex(pattern),
                #[cfg(not(feature = "regex-filter"))]
                "regex" => bail!("regex feature not enabled"),
                other => bail!("unknown filter kind: {other}"),
            };
            Ok(pipe::NodeKind::Filter(pipe::FilterNodeConfig {
                filters: vec![Filter { kind: filter_kind, direction: dir, action_on_match: action }],
            }))
        }
        "duplicate" => {
            let target: SocketAddr = c.get("target").and_then(|v| v.as_str()).unwrap_or("0.0.0.0:0")
                .parse().context("invalid duplicate target")?;
            Ok(pipe::NodeKind::Duplicate(pipe::DuplicateConfig {
                endpoint: Endpoint::tcp(target),
            }))
        }
        "export" => {
            let exp_kind = c.get("kind").and_then(|v| v.as_str()).unwrap_or("tcp");
            let ek = match exp_kind {
                "uds" | "unix" => {
                    let path = c.get("path").and_then(|v| v.as_str()).unwrap_or("").to_string();
                    ExporterKind::Uds { path: PathBuf::from(path) }
                }
                _ => {
                    let addr: SocketAddr = c.get("addr").and_then(|v| v.as_str()).unwrap_or("127.0.0.1:4000")
                        .parse().context("invalid exporter addr")?;
                    ExporterKind::TcpFramed { addr }
                }
            };
            Ok(pipe::NodeKind::Export(pipe::ExportNodeConfig {
                exporter: ExporterConfig { kind: ek },
            }))
        }
        "drop" => Ok(pipe::NodeKind::Drop),
        "router" => {
            let kind_str = c.get("kind").and_then(|v| v.as_str()).unwrap_or("substr");
            let pattern = c.get("pattern").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let dir = match c.get("direction").and_then(|v| v.as_str()) {
                Some("c2s") => Direction::ClientToServer,
                Some("s2c") => Direction::ServerToClient,
                _ => Direction::Both,
            };
            let fk = match kind_str {
                "substr" => FilterKind::Substr(pattern),
                "bsubstr" => FilterKind::BinarySubstr(hex_decode(&pattern)?),
                _ => FilterKind::Substr(pattern),
            };
            Ok(pipe::NodeKind::Router(pipe::RouterConfig {
                filter: Filter { kind: fk, direction: dir, action_on_match: FilterAction::Drop },
            }))
        }
        other => bail!("unknown pipeline node kind: {other}"),
    }
}

fn convert_filter(
    f: crate::config::schema::AppFilter,
    rule_name: &str,
) -> Result<Filter> {
    let direction = match f.direction.as_deref() {
        Some("c2s") | Some("client_to_server") => Direction::ClientToServer,
        Some("s2c") | Some("server_to_client") => Direction::ServerToClient,
        Some("both") | None => Direction::Both,
        Some(other) => bail!("unknown filter direction '{}' in rule '{}'", other, rule_name),
    };

    let action_on_match = match f.action.as_deref() {
        Some("pass") | None => FilterAction::Pass,
        Some("drop") => FilterAction::Drop,
        Some("drop_connection") => FilterAction::DropConnection,
        Some("tap") | Some("tap-only") | Some("tap_only") => FilterAction::TapOnly,
        Some(other) => bail!("unknown filter action '{}' in rule '{}'", other, rule_name),
    };

    let kind = match f.kind.to_lowercase().as_str() {
        "substr" => FilterKind::Substr(f.pattern),
        "bsubstr" | "binary_substr" => {
            let bytes = hex_decode(&f.pattern)
                .with_context(|| format!("invalid hex pattern in bsubstr filter, rule '{}'", rule_name))?;
            FilterKind::BinarySubstr(bytes)
        }
        #[cfg(feature = "regex-filter")]
        "regex" => FilterKind::Regex(f.pattern),
        #[cfg(not(feature = "regex-filter"))]
        "regex" => bail!(
            "regex filter requested in rule '{}' but regex-filter feature is not enabled",
            rule_name
        ),
        other => bail!("unknown filter kind '{}' in rule '{}'", other, rule_name),
    };

    Ok(Filter {
        kind,
        direction,
        action_on_match,
    })
}

fn hex_decode(s: &str) -> Result<Vec<u8>> {
    let s = s.trim();
    if s.len() % 2 != 0 {
        bail!("hex string has odd length");
    }
    (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(&s[i..i + 2], 16)
                .with_context(|| format!("invalid hex byte at position {i}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::schema::*;

    #[test]
    fn test_convert_basic_tcp_rule() {
        let app = AppConfig {
            global: GlobalConfig::default(),
            pipelines: vec![],
            rules: vec![AppRule {
                name: "test".into(),
                protocol: "tcp".into(),
                listen: "127.0.0.1:8080".into(),
                target: "127.0.0.1:9090".into(),
                filters: vec![],
                duplicate: None,
                exporter: None,
                tls: None,
                udp_source_mode: None,
                idle_timeout_secs: None,
            }],
        };

        let core = convert(app).unwrap();
        assert_eq!(core.rules.len(), 1);
        assert_eq!(core.rules[0].name, "test");
        assert_eq!(core.rules[0].listen.protocol, Protocol::Tcp);
    }

    #[test]
    fn test_convert_with_filter() {
        let app = AppConfig {
            global: GlobalConfig::default(),
            pipelines: vec![],
            rules: vec![AppRule {
                name: "filtered".into(),
                protocol: "udp".into(),
                listen: "0.0.0.0:5353".into(),
                target: "8.8.8.8:53".into(),
                filters: vec![AppFilter {
                    kind: "substr".into(),
                    pattern: "blocked".into(),
                    direction: Some("c2s".into()),
                    action: Some("drop".into()),
                }],
                duplicate: None,
                exporter: None,
                tls: None,
                udp_source_mode: None,
                idle_timeout_secs: Some(60),
            }],
        };

        let core = convert(app).unwrap();
        assert_eq!(core.rules[0].filters.len(), 1);
        assert_eq!(core.rules[0].filters[0].direction, Direction::ClientToServer);
        assert_eq!(core.rules[0].filters[0].action_on_match, FilterAction::Drop);
        assert_eq!(core.rules[0].idle_timeout_secs, 60);
    }

    #[test]
    fn test_hex_decode() {
        assert_eq!(hex_decode("deadbeef").unwrap(), vec![0xde, 0xad, 0xbe, 0xef]);
        assert!(hex_decode("xyz").is_err());
    }

    #[test]
    fn test_range_expansion_1to1() {
        let r = AppRule {
            name: "range".into(),
            protocol: "tcp".into(),
            listen: "0.0.0.0:3000-3002".into(),
            target: "10.0.0.1:4000-4002".into(),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: None,
            idle_timeout_secs: None,
        };
        let expanded = expand_range_rule(r).unwrap();
        assert_eq!(expanded.len(), 3);
        assert_eq!(expanded[0].name, "range/3000");
        assert_eq!(expanded[0].listen, "0.0.0.0:3000");
        assert_eq!(expanded[0].target, "10.0.0.1:4000");
        assert_eq!(expanded[1].listen, "0.0.0.0:3001");
        assert_eq!(expanded[1].target, "10.0.0.1:4001");
        assert_eq!(expanded[2].listen, "0.0.0.0:3002");
        assert_eq!(expanded[2].target, "10.0.0.1:4002");
    }

    #[test]
    fn test_range_expansion_fan_in() {
        let r = AppRule {
            name: "fanin".into(),
            protocol: "udp".into(),
            listen: "0.0.0.0:5000-5002".into(),
            target: "10.0.0.1:9000".into(),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: None,
            idle_timeout_secs: Some(30),
        };
        let expanded = expand_range_rule(r).unwrap();
        assert_eq!(expanded.len(), 3);
        // All target the same port
        assert_eq!(expanded[0].target, "10.0.0.1:9000");
        assert_eq!(expanded[1].target, "10.0.0.1:9000");
        assert_eq!(expanded[2].target, "10.0.0.1:9000");
    }

    #[test]
    fn test_range_mismatch_error() {
        let r = AppRule {
            name: "bad".into(),
            protocol: "tcp".into(),
            listen: "0.0.0.0:3000-3005".into(),
            target: "10.0.0.1:4000-4002".into(),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: None,
            idle_timeout_secs: None,
        };
        assert!(expand_range_rule(r).is_err());
    }

    #[test]
    fn test_no_range_passthrough() {
        let r = AppRule {
            name: "normal".into(),
            protocol: "tcp".into(),
            listen: "0.0.0.0:8080".into(),
            target: "10.0.0.1:80".into(),
            filters: vec![],
            duplicate: None,
            exporter: None,
            tls: None,
            udp_source_mode: None,
            idle_timeout_secs: None,
        };
        let expanded = expand_range_rule(r).unwrap();
        assert_eq!(expanded.len(), 1);
        assert_eq!(expanded[0].name, "normal");
    }

    #[test]
    fn test_full_range_conversion() {
        let app = AppConfig {
            global: GlobalConfig::default(),
            pipelines: vec![],
            rules: vec![AppRule {
                name: "game".into(),
                protocol: "tcp".into(),
                listen: "0.0.0.0:3000-3002".into(),
                target: "90.90.90.90:4000-4002".into(),
                filters: vec![],
                duplicate: None,
                exporter: None,
                tls: None,
                udp_source_mode: None,
                idle_timeout_secs: None,
            }],
        };
        let core = convert(app).unwrap();
        assert_eq!(core.rules.len(), 3);
        assert_eq!(core.rules[0].name, "game/3000");
        assert_eq!(core.rules[0].listen.addr.port(), 3000);
        assert_eq!(core.rules[0].target.addr.port(), 4000);
        assert_eq!(core.rules[2].listen.addr.port(), 3002);
        assert_eq!(core.rules[2].target.addr.port(), 4002);
    }
}
