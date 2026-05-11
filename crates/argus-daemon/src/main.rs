fn main() -> anyhow::Result<()> {
    // Keep this binding alive for the lifetime of the process: dropping the
    // WorkerGuard flushes the non-blocking tracing appender thread.
    let _flush_guard = argus_core::logging::init(argus_core::logging::default_config()?)?;
    tracing::info!("argus-daemon starting");
    Ok(())
}
