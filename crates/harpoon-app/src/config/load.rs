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
