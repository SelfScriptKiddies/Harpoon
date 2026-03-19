use std::process::Command;

use anyhow::{bail, Context, Result};

pub fn check_nft_available() -> bool {
    Command::new("nft")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

pub fn apply_ruleset(ruleset: &str) -> Result<()> {
    tracing::debug!(ruleset = %ruleset, "applying nft ruleset");

    let mut child = Command::new("nft")
        .arg("-f")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("failed to spawn nft")?;

    use std::io::Write;
    if let Some(ref mut stdin) = child.stdin {
        stdin
            .write_all(ruleset.as_bytes())
            .context("writing ruleset to nft stdin")?;
    }

    let output = child.wait_with_output().context("waiting for nft")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("nft failed (exit {}): {}", output.status, stderr.trim());
    }

    tracing::info!("nft ruleset applied successfully");
    Ok(())
}

pub fn cleanup_table() -> Result<()> {
    let ruleset = super::render::render_cleanup();
    // Cleanup is best-effort; table might not exist
    let _ = apply_ruleset_quiet(&ruleset);
    Ok(())
}

fn apply_ruleset_quiet(ruleset: &str) -> Result<()> {
    let mut child = Command::new("nft")
        .arg("-f")
        .arg("-")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .context("failed to spawn nft")?;

    use std::io::Write;
    if let Some(ref mut stdin) = child.stdin {
        let _ = stdin.write_all(ruleset.as_bytes());
    }

    let _ = child.wait();
    Ok(())
}

/// Apply ruleset with rollback on failure
pub fn apply_with_rollback(ruleset: &str) -> Result<()> {
    // Save cleanup ruleset before applying
    let cleanup = super::render::render_cleanup();

    match apply_ruleset(ruleset) {
        Ok(()) => Ok(()),
        Err(e) => {
            tracing::error!(error = %e, "nft apply failed, rolling back");
            let _ = apply_ruleset_quiet(&cleanup);
            Err(e)
        }
    }
}

/// Set up ip rule for TPROXY mark-based routing
pub fn setup_tproxy_routing(mark: u32) -> Result<()> {
    let status = Command::new("ip")
        .args(["rule", "add", "fwmark", &format!("0x{mark:x}"), "lookup", "100"])
        .status()
        .context("failed to add ip rule for tproxy")?;

    if !status.success() {
        bail!("ip rule add failed");
    }

    let status = Command::new("ip")
        .args(["route", "add", "local", "0.0.0.0/0", "dev", "lo", "table", "100"])
        .status()
        .context("failed to add ip route for tproxy")?;

    if !status.success() {
        // Route might already exist
        tracing::debug!("ip route add failed (might already exist)");
    }

    Ok(())
}

/// Clean up TPROXY routing rules
pub fn cleanup_tproxy_routing(mark: u32) -> Result<()> {
    let _ = Command::new("ip")
        .args(["rule", "del", "fwmark", &format!("0x{mark:x}"), "lookup", "100"])
        .status();

    let _ = Command::new("ip")
        .args(["route", "del", "local", "0.0.0.0/0", "dev", "lo", "table", "100"])
        .status();

    Ok(())
}
