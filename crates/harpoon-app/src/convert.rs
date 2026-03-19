use std::net::SocketAddr;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};

use harpoon_core::config::CoreConfig;
use harpoon_core::types::endpoint::{Endpoint, Protocol};
use harpoon_core::types::filter::{Direction, Filter, FilterAction, FilterKind};
use harpoon_core::types::rule::{
    DuplicateTarget, ExporterConfig, ExporterKind, Rule, TlsConfig, TlsMode, UdpSourceMode,
};

use crate::config::schema::{AppConfig, AppRule};

pub fn convert(app: AppConfig) -> Result<CoreConfig> {
    let mut rules = Vec::new();

    for r in app.rules {
        let rule = convert_rule(r)?;
        rules.push(rule);
    }

    let mut config = CoreConfig {
        rules,
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
        idle_timeout_secs,
    })
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
}
