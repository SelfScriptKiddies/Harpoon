use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use harpoon_core::types::endpoint::Protocol;

use crate::config::load::load_config;
use crate::config::schema::AppConfig;
use crate::control::proto::RuleInfo;
use crate::control::server::{run_control_server, ControlState};
use crate::convert::convert;
use crate::nft;

use super::state;

pub struct DaemonOpts {
    pub config_path: PathBuf,
    pub socket_path: PathBuf,
    pub pid_file: PathBuf,
    pub daemonize: bool,
}

pub async fn run_daemon(opts: DaemonOpts) -> Result<()> {
    if opts.daemonize {
        daemonize(&opts.pid_file)?;
    } else {
        state::write_pid_file(&opts.pid_file)
            .with_context(|| format!("writing pid file: {}", opts.pid_file.display()))?;
    }

    let result = run_engine_loop(opts.config_path.clone(), &opts.socket_path).await;

    state::remove_pid_file(&opts.pid_file);
    result
}

async fn run_engine_loop(config_path: PathBuf, socket_path: &std::path::Path) -> Result<()> {
    let app_config = load_config(&config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;

    // Apply nftables rules if configured
    let (nft_active, tproxy_mark) = apply_nft_config(&app_config)?;

    // Save web settings before consuming app_config
    #[cfg(feature = "web")]
    let _web_bind = app_config.global.web_bind.clone();
    #[cfg(feature = "web")]
    let _web_password = app_config.global.web_password.clone();

    let core_config = convert(app_config).context("converting config")?;
    let rules_info = build_rules_info(&core_config);

    tracing::info!(rules = core_config.rules.len(), "starting harpoon engine");

    let engine_handle = harpoon_core::run(core_config)
        .await
        .context("starting engine")?;

    let cancel = CancellationToken::new();
    let (reload_tx, mut reload_rx) = mpsc::channel::<PathBuf>(1);

    let control_state = Arc::new(RwLock::new(ControlState {
        engine_handle: Some(engine_handle),
        config_path: config_path.clone(),
        start_time: Instant::now(),
        rules_info,
        cancel: cancel.clone(),
        recent_events: Arc::new(Mutex::new(Vec::new())),
        reload_tx,
    }));

    // Spawn control server
    let ctrl_cancel = cancel.child_token();
    let ctrl_state = control_state.clone();
    let ctrl_path = socket_path.to_path_buf();
    let ctrl_handle = tokio::spawn(async move {
        if let Err(e) = run_control_server(&ctrl_path, ctrl_state, ctrl_cancel).await {
            tracing::error!(error = %e, "control server error");
        }
    });

    // Spawn web UI if configured
    #[cfg(feature = "web")]
    if let Some(ref web_bind) = _web_bind {
        let web_addr: std::net::SocketAddr = web_bind.parse()
            .with_context(|| format!("invalid web_bind address: {web_bind}"))?;
        let web_password = _web_password.unwrap_or_else(|| {
            let generated = gen_password();
            tracing::info!("Web UI credentials — login: admin, password: {generated}");
            generated
        });
        let web_state = control_state.clone();
        let web_cancel = cancel.child_token();
        tokio::spawn(async move {
            if let Err(e) = crate::ui::web::server::run_web_server(web_addr, web_state, web_password, web_cancel).await {
                tracing::error!(error = %e, "web server error");
            }
        });
    }

    tracing::info!("harpoon is running");

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("received ctrl-c, shutting down");
                cancel.cancel();
                break;
            }
            _ = cancel.cancelled() => {
                tracing::info!("shutdown requested via control socket");
                break;
            }
            Some(new_config_path) = reload_rx.recv() => {
                tracing::info!(path = %new_config_path.display(), "reloading config");
                match reload_engine(&control_state, &new_config_path).await {
                    Ok(_) => tracing::info!("config reloaded successfully"),
                    Err(e) => tracing::error!(error = %e, "config reload failed"),
                }
            }
        }
    }

    // Shutdown engine
    {
        let mut s = control_state.write().await;
        if let Some(handle) = s.engine_handle.take() {
            let results = handle.shutdown().await;
            for r in &results {
                if let Err(e) = r {
                    tracing::error!(error = %e, "rule task error during shutdown");
                }
            }
        }
    }

    // Wait for control server to finish
    cancel.cancel();
    let _ = ctrl_handle.await;

    // Clean up nftables rules
    if nft_active {
        tracing::info!("cleaning up nftables rules");
        let _ = nft::apply::cleanup_table();
        if let Some(mark) = tproxy_mark {
            let _ = nft::apply::cleanup_tproxy_routing(mark);
        }
    }

    tracing::info!("harpoon stopped");
    Ok(())
}

async fn reload_engine(
    state: &Arc<RwLock<ControlState>>,
    config_path: &std::path::Path,
) -> Result<()> {
    let app_config = load_config(config_path)
        .with_context(|| format!("loading config from {}", config_path.display()))?;
    let core_config = convert(app_config).context("converting config")?;
    let new_rules_info = build_rules_info(&core_config);

    // Stop old engine
    {
        let mut s = state.write().await;
        if let Some(handle) = s.engine_handle.take() {
            handle.stop();
            let _ = handle.shutdown().await;
        }
    }

    // Start new engine
    let new_handle = harpoon_core::run(core_config)
        .await
        .context("starting new engine")?;

    {
        let mut s = state.write().await;
        s.engine_handle = Some(new_handle);
        s.rules_info = new_rules_info;
        s.config_path = config_path.to_path_buf();
        s.recent_events.lock().await.clear();
    }

    Ok(())
}

fn build_rules_info(config: &harpoon_core::CoreConfig) -> Vec<RuleInfo> {
    config
        .rules
        .iter()
        .map(|r| RuleInfo {
            name: r.name.clone(),
            protocol: match r.listen.protocol {
                Protocol::Tcp => "tcp".into(),
                Protocol::Udp => "udp".into(),
            },
            listen: r.listen.addr.to_string(),
            target: r.target.addr.to_string(),
            filters_count: r.filters.len(),
            has_duplicate: r.duplicate.is_some(),
            has_exporter: r.exporter.is_some(),
        })
        .collect()
}

fn apply_nft_config(app_config: &AppConfig) -> Result<(bool, Option<u32>)> {
    let nft_cfg = &app_config.global.nft;
    if !nft_cfg.enabled {
        return Ok((false, None));
    }

    if !nft::apply::check_nft_available() {
        anyhow::bail!("nftables enabled in config but 'nft' command not found");
    }

    if nft_cfg.rules.is_empty() {
        tracing::debug!("nft enabled but no rules configured");
        return Ok((false, None));
    }

    let mut nft_rules = Vec::new();
    let mut has_tproxy = false;

    for r in &nft_cfg.rules {
        let protocol = match r.protocol.to_lowercase().as_str() {
            "tcp" => nft::render::NftProtocol::Tcp,
            "udp" => nft::render::NftProtocol::Udp,
            other => anyhow::bail!("unknown nft rule protocol: {other}"),
        };

        let match_dst = r
            .match_dst
            .as_ref()
            .map(|s| s.parse())
            .transpose()
            .context("invalid match_dst address")?;

        let action = match r.action.to_lowercase().as_str() {
            "redirect" => {
                let port = r.to_port.context("redirect action requires to_port")?;
                nft::render::NftAction::Redirect { to_port: port }
            }
            "dnat" => {
                let addr_str = r.to_addr.as_ref().context("dnat action requires to_addr")?;
                let addr = addr_str.parse().context("invalid dnat to_addr")?;
                nft::render::NftAction::Dnat { to_addr: addr }
            }
            "tproxy" => {
                let port = r.to_port.context("tproxy action requires to_port")?;
                let mark = nft_cfg.tproxy_mark.unwrap_or(0x1);
                has_tproxy = true;
                nft::render::NftAction::Tproxy { to_port: port, mark }
            }
            other => anyhow::bail!("unknown nft action: {other}"),
        };

        nft_rules.push(nft::render::NftRule {
            protocol,
            match_dport: r.match_dport,
            match_dst: match_dst,
            action,
            comment: r.comment.clone(),
        });
    }

    let tproxy_mark = if has_tproxy {
        let mark = nft_cfg.tproxy_mark.unwrap_or(0x1);
        let ruleset = nft::render::render_tproxy_install(&nft_rules, mark);
        nft::apply::apply_with_rollback(&ruleset)?;
        nft::apply::setup_tproxy_routing(mark)?;
        Some(mark)
    } else {
        let ruleset = nft::render::render_install(&nft_rules);
        nft::apply::apply_with_rollback(&ruleset)?;
        None
    };

    tracing::info!(rules = nft_rules.len(), "nftables rules applied");
    Ok((true, tproxy_mark))
}

#[cfg(feature = "web")]
fn gen_password() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seed = ts ^ (std::process::id() as u128) ^ 0xDEAD_BEEF;
    format!("{:016x}", seed)
}

fn daemonize(pid_file: &std::path::Path) -> Result<()> {
    // Double-fork daemonization
    match unsafe { libc::fork() } {
        -1 => return Err(anyhow::anyhow!("first fork failed")),
        0 => {} // child continues
        _ => std::process::exit(0), // parent exits
    }

    // Create new session
    if unsafe { libc::setsid() } == -1 {
        return Err(anyhow::anyhow!("setsid failed"));
    }

    // Second fork
    match unsafe { libc::fork() } {
        -1 => return Err(anyhow::anyhow!("second fork failed")),
        0 => {} // grandchild continues
        _ => std::process::exit(0), // child exits
    }

    // Redirect stdio to /dev/null
    let devnull = std::fs::File::open("/dev/null")?;
    use std::os::unix::io::AsRawFd;
    unsafe {
        libc::dup2(devnull.as_raw_fd(), 0);
        libc::dup2(devnull.as_raw_fd(), 1);
        libc::dup2(devnull.as_raw_fd(), 2);
    }

    state::write_pid_file(pid_file)
        .with_context(|| format!("writing pid file: {}", pid_file.display()))?;

    Ok(())
}
