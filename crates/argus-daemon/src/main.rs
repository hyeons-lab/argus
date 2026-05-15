#[cfg(unix)]
use std::sync::Arc;

#[cfg(unix)]
use argus_daemon::ipc::{UnixSocketConfig, UnixSocketServer, default_socket_path};
#[cfg(unix)]
use argus_daemon::session::SessionManager;

fn main() -> anyhow::Result<()> {
    // Keep this binding alive for the lifetime of the process: dropping the
    // WorkerGuard flushes the non-blocking tracing appender thread.
    let _flush_guard = argus_core::logging::init(argus_core::logging::default_config()?)?;
    run()
}

#[cfg(unix)]
fn run() -> anyhow::Result<()> {
    let socket_path = default_socket_path();
    tracing::info!(socket_path = %socket_path.display(), "argus-daemon starting");
    let manager = Arc::new(SessionManager::default());
    UnixSocketServer::new(manager, UnixSocketConfig::new(socket_path)).serve()
}

#[cfg(not(unix))]
fn run() -> anyhow::Result<()> {
    anyhow::bail!("argus-daemon Unix socket API is only available on Unix platforms")
}
