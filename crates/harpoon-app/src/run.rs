use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::config::load::load_config;
use crate::convert::convert;

pub async fn foreground(config_path: PathBuf) -> Result<()> {
    let app_config =
        load_config(&config_path).with_context(|| format!("loading config from {}", config_path.display()))?;

    let core_config = convert(app_config).context("converting config")?;

    tracing::info!(rules = core_config.rules.len(), "starting harpoon engine");

    let handle = harpoon_core::run(core_config)
        .await
        .context("starting engine")?;

    tracing::info!("harpoon is running, press Ctrl+C to stop");

    tokio::signal::ctrl_c()
        .await
        .context("waiting for ctrl-c")?;

    tracing::info!("received ctrl-c, shutting down");
    let results = handle.shutdown().await;

    for r in &results {
        if let Err(e) = r {
            tracing::error!(error = %e, "rule task error during shutdown");
        }
    }

    tracing::info!("harpoon stopped");
    Ok(())
}
