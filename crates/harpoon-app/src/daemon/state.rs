#![allow(dead_code)]

use std::path::PathBuf;

pub const DEFAULT_SOCKET_PATH: &str = "/tmp/harpoon.sock";
pub const DEFAULT_PID_FILE: &str = "/tmp/harpoon.pid";

pub fn default_socket_path() -> PathBuf {
    PathBuf::from(DEFAULT_SOCKET_PATH)
}

pub fn default_pid_file() -> PathBuf {
    PathBuf::from(DEFAULT_PID_FILE)
}

pub fn write_pid_file(path: &std::path::Path) -> std::io::Result<()> {
    std::fs::write(path, std::process::id().to_string())
}

pub fn remove_pid_file(path: &std::path::Path) {
    let _ = std::fs::remove_file(path);
}

pub fn read_pid_file(path: &std::path::Path) -> Option<u32> {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
}

pub fn is_daemon_running(pid_path: &std::path::Path) -> bool {
    if let Some(pid) = read_pid_file(pid_path) {
        // Check if process exists
        unsafe { libc::kill(pid as i32, 0) == 0 }
    } else {
        false
    }
}
