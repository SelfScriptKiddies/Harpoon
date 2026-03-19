use std::path::Path;

use anyhow::{Context, Result};

use super::schema::AppConfig;

pub fn load_config(path: &Path) -> Result<AppConfig> {
    let contents =
        std::fs::read_to_string(path).with_context(|| format!("reading config: {}", path.display()))?;
    let config: AppConfig =
        toml::from_str(&contents).with_context(|| format!("parsing config: {}", path.display()))?;
    Ok(config)
}

#[allow(dead_code)]
pub fn save_config(path: &Path, config: &AppConfig) -> Result<()> {
    let contents =
        toml::to_string_pretty(config).context("serializing config to TOML")?;
    std::fs::write(path, contents)
        .with_context(|| format!("writing config: {}", path.display()))?;
    Ok(())
}
