use std::path::PathBuf;

use clap::{Parser, Subcommand};

use crate::daemon::state;

#[derive(Parser)]
#[command(name = "harpoon", about = "MITM/traffic-analysis proxy for Linux")]
pub struct Cli {
    /// Path to control socket
    #[arg(long, default_value = state::DEFAULT_SOCKET_PATH, global = true)]
    pub socket: PathBuf,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Run the proxy engine
    Run {
        /// Path to config file
        #[arg(short, long, default_value = "config.toml")]
        config: PathBuf,

        /// Run as background daemon
        #[arg(short, long)]
        daemon: bool,

        /// Path to PID file
        #[arg(long, default_value = state::DEFAULT_PID_FILE)]
        pid_file: PathBuf,
    },

    /// Stop the running daemon
    Stop,

    /// Show daemon status
    Status,

    /// Show traffic statistics
    Stats,

    /// List active rules
    Rules,

    /// Show recent events
    Events {
        /// Number of events to show
        #[arg(short, long, default_value = "50")]
        count: usize,
    },

    /// Reload configuration
    Reload {
        /// Path to new config file (uses current if omitted)
        #[arg(short, long)]
        config: Option<PathBuf>,
    },
}
