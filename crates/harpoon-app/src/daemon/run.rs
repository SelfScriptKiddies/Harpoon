use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::{Context, Result};
use tokio::sync::{mpsc, Mutex, RwLock};
use tokio_util::sync::CancellationToken;

use harpoon_core::types::endpoint::Protocol;

use crate::config::load::load_config;
use crate::control::proto::RuleInfo;
use crate::control::server::{run_control_server, ControlState};
use crate::convert::convert;

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
