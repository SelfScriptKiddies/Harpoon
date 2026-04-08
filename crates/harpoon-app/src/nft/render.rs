use std::fmt::Write;
use std::net::SocketAddr;

const TABLE_NAME: &str = "harpoon";
const CHAIN_PREROUTING: &str = "harpoon_prerouting";
const CHAIN_OUTPUT: &str = "harpoon_output";

#[derive(Debug, Clone)]
pub enum NftAction {
    Redirect { to_port: u16 },
    Dnat { to_addr: SocketAddr },
    Tproxy { to_port: u16, mark: u32 },
}

#[derive(Debug, Clone)]
pub struct NftRule {
    pub protocol: NftProtocol,
    pub match_dport: u16,
    pub match_dst: Option<std::net::IpAddr>,
    pub action: NftAction,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub enum NftProtocol {
    Tcp,
    Udp,
}

impl NftProtocol {
    fn as_str(&self) -> &'static str {
        match self {
            NftProtocol::Tcp => "tcp",
            NftProtocol::Udp => "udp",
        }
    }
}

pub fn render_install(rules: &[NftRule]) -> String {
    let mut out = String::new();

    // Flush and recreate table
    writeln!(out, "table ip {TABLE_NAME} {{").unwrap();

    // Prerouting chain for REDIRECT/DNAT/TPROXY
    writeln!(
        out,
        "  chain {CHAIN_PREROUTING} {{"
    )
    .unwrap();
    writeln!(
        out,
        "    type nat hook prerouting priority dstnat; policy accept;"
    )
    .unwrap();

    for rule in rules {
        let line = render_rule(rule);
        writeln!(out, "    {line}").unwrap();
    }

    writeln!(out, "  }}").unwrap();

    // Output chain for local redirect (connections originating from this host)
    writeln!(out, "  chain {CHAIN_OUTPUT} {{").unwrap();
    writeln!(
        out,
        "    type nat hook output priority dstnat; policy accept;"
    )
    .unwrap();

    for rule in rules {
        if matches!(rule.action, NftAction::Redirect { .. }) {
            let line = render_rule(rule);
            writeln!(out, "    {line}").unwrap();
        }
    }

    writeln!(out, "  }}").unwrap();
    writeln!(out, "}}").unwrap();

    out
}

pub fn render_tproxy_install(rules: &[NftRule], mark: u32) -> String {
    let mut out = String::new();

    writeln!(out, "table ip {TABLE_NAME} {{").unwrap();

    // Mangle chain for TPROXY
    writeln!(out, "  chain harpoon_tproxy {{").unwrap();
    writeln!(
        out,
        "    type filter hook prerouting priority mangle; policy accept;"
    )
    .unwrap();

    for rule in rules {
        if let NftAction::Tproxy { to_port, mark: _ } = &rule.action {
            let proto = rule.protocol.as_str();
            let dport = rule.match_dport;

            let dst_match = rule
                .match_dst
                .map(|d| format!("ip daddr {d} "))
                .unwrap_or_default();

            writeln!(
                out,
                "    {dst_match}{proto} dport {dport} tproxy to :{to_port} meta mark set 0x{mark:x}"
            )
            .unwrap();
        }
    }

    writeln!(out, "  }}").unwrap();

    // Routing rule to intercept marked packets
    writeln!(out, "  chain harpoon_mark {{").unwrap();
    writeln!(
        out,
        "    type route hook output priority mangle; policy accept;"
    )
    .unwrap();

    for rule in rules {
        if let NftAction::Tproxy { .. } = &rule.action {
            let proto = rule.protocol.as_str();
            let dport = rule.match_dport;

            let dst_match = rule
                .match_dst
                .map(|d| format!("ip daddr {d} "))
                .unwrap_or_default();

            writeln!(
                out,
                "    {dst_match}{proto} dport {dport} meta mark set 0x{mark:x}"
            )
            .unwrap();
        }
    }

    writeln!(out, "  }}").unwrap();
    writeln!(out, "}}").unwrap();

    out
}

pub fn render_cleanup() -> String {
    format!("delete table ip {TABLE_NAME}\n")
}

fn render_rule(rule: &NftRule) -> String {
    let proto = rule.protocol.as_str();
    let dport = rule.match_dport;

    let dst_match = rule
        .match_dst
        .map(|d| format!("ip daddr {d} "))
        .unwrap_or_default();

    let comment = rule
        .comment
        .as_ref()
        .map(|c| {
            let sanitized = c.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " ").replace('\r', "");
            format!(" comment \"{sanitized}\"")
        })
        .unwrap_or_default();

    let action = match &rule.action {
        NftAction::Redirect { to_port } => format!("redirect to :{to_port}"),
        NftAction::Dnat { to_addr } => format!("dnat to {to_addr}"),
        NftAction::Tproxy { to_port, mark } => {
            format!("tproxy to :{to_port} meta mark set 0x{mark:x}")
        }
    };

    format!("{dst_match}{proto} dport {dport} {action}{comment}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_redirect_rule() {
        let rules = vec![NftRule {
            protocol: NftProtocol::Tcp,
            match_dport: 80,
            match_dst: None,
            action: NftAction::Redirect { to_port: 8080 },
            comment: Some("web-proxy".into()),
        }];

        let out = render_install(&rules);
        assert!(out.contains("tcp dport 80 redirect to :8080"));
        assert!(out.contains("comment \"web-proxy\""));
        assert!(out.contains("table ip harpoon"));
    }

    #[test]
    fn test_render_dnat_rule() {
        let rules = vec![NftRule {
            protocol: NftProtocol::Udp,
            match_dport: 53,
            match_dst: Some("10.0.0.1".parse().unwrap()),
            action: NftAction::Dnat {
                to_addr: "10.0.0.5:5353".parse().unwrap(),
            },
            comment: None,
        }];

        let out = render_install(&rules);
        assert!(out.contains("ip daddr 10.0.0.1 udp dport 53 dnat to 10.0.0.5:5353"));
    }

    #[test]
    fn test_render_cleanup() {
        let out = render_cleanup();
        assert!(out.contains("delete table ip harpoon"));
    }

    #[test]
    fn test_render_tproxy() {
        let rules = vec![NftRule {
            protocol: NftProtocol::Tcp,
            match_dport: 443,
            match_dst: None,
            action: NftAction::Tproxy {
                to_port: 8443,
                mark: 0x1,
            },
            comment: None,
        }];

        let out = render_tproxy_install(&rules, 0x1);
        assert!(out.contains("tcp dport 443 tproxy to :8443"));
        assert!(out.contains("meta mark set 0x1"));
    }
}
