mod cli;
mod config;
mod control;
mod convert;
mod daemon;
mod nft;
mod run;

use clap::Parser;

use cli::Commands;
use control::client::ControlClient;
use control::proto::{Request, Response};
use daemon::run::DaemonOpts;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = cli::Cli::parse();
    let socket_path = cli.socket.clone();

    match cli.command {
        Commands::Run {
            config,
            daemon: daemonize,
            pid_file,
        } => {
            init_tracing();
            daemon::run::run_daemon(DaemonOpts {
                config_path: config,
                socket_path,
                pid_file,
                daemonize,
            })
            .await?;
        }

        Commands::Stop => {
            let mut client = ControlClient::connect(&socket_path).await?;
            match client.send(Request::Stop).await? {
                Response::Ok => println!("Daemon stopping"),
                Response::Error { message } => eprintln!("Error: {message}"),
                _ => eprintln!("Unexpected response"),
            }
        }

        Commands::Status => {
            let mut client = ControlClient::connect(&socket_path).await?;
            match client.send(Request::Status).await? {
                Response::Status(info) => {
                    println!("Running:     {}", info.running);
                    println!("Uptime:      {}s", info.uptime_secs);
                    println!("Rules:       {}", info.rules_count);
                    println!("Config:      {}", info.config_path);
                }
                Response::Error { message } => eprintln!("Error: {message}"),
                _ => eprintln!("Unexpected response"),
            }
        }

        Commands::Stats => {
            let mut client = ControlClient::connect(&socket_path).await?;
            match client.send(Request::Stats).await? {
                Response::Stats(stats) => {
                    if stats.is_empty() {
                        println!("No rules active");
                        return Ok(());
                    }
                    for s in &stats {
                        println!("--- {} ---", s.rule_name);
                        println!("  bytes  c->s: {}", s.bytes_client_to_server);
                        println!("  bytes  s->c: {}", s.bytes_server_to_client);
                        println!("  pkts   c->s: {}", s.packets_client_to_server);
                        println!("  pkts   s->c: {}", s.packets_server_to_client);
                        println!("  tcp conns:   {}", s.active_tcp_connections);
                        println!("  udp sess:    {}", s.active_udp_sessions);
                        println!("  dropped:     {}", s.dropped_packets);
                        println!("  filter hits: {}", s.filter_matches);
                    }
                }
                Response::Error { message } => eprintln!("Error: {message}"),
                _ => eprintln!("Unexpected response"),
            }
        }

        Commands::Rules => {
            let mut client = ControlClient::connect(&socket_path).await?;
            match client.send(Request::RulesList).await? {
                Response::Rules(rules) => {
                    if rules.is_empty() {
                        println!("No rules configured");
                        return Ok(());
                    }
                    for r in &rules {
                        println!(
                            "{:<20} {:<5} {:<22} -> {:<22} filters={} dup={} exp={}",
                            r.name,
                            r.protocol,
                            r.listen,
                            r.target,
                            r.filters_count,
                            r.has_duplicate,
                            r.has_exporter,
                        );
                    }
                }
                Response::Error { message } => eprintln!("Error: {message}"),
                _ => eprintln!("Unexpected response"),
            }
        }

        Commands::Events { count } => {
            let mut client = ControlClient::connect(&socket_path).await?;
            match client.send(Request::Events { count: Some(count) }).await? {
                Response::Events(events) => {
                    if events.is_empty() {
                        println!("No recent events");
                        return Ok(());
                    }
                    for e in &events {
                        println!("[{}] {}: {}", e.timestamp_ms, e.kind, e.detail);
                    }
                }
                Response::Error { message } => eprintln!("Error: {message}"),
                _ => eprintln!("Unexpected response"),
            }
        }

        Commands::Reload { config } => {
            let mut client = ControlClient::connect(&socket_path).await?;
            let config_path = config.map(|p| p.display().to_string());
            match client
                .send(Request::Reload {
                    config_path,
                })
                .await?
            {
                Response::Ok => println!("Reload initiated"),
                Response::Error { message } => eprintln!("Error: {message}"),
                _ => eprintln!("Unexpected response"),
            }
        }
    }

    Ok(())
}

fn init_tracing() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();
}
