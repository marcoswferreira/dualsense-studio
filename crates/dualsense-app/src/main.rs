use anyhow::Result;
use dualsense_core::DualSenseManager;
use std::sync::Arc;
use tracing::info;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();
    info!("Starting DualSense Studio v0.1.0...");

    let manager = Arc::new(DualSenseManager::new());
    manager.set_extension_enabled(true);

    info!("DualSense Studio is running. Press Ctrl+C to terminate.");
    tokio::signal::ctrl_c().await?;
    info!("Shutting down DualSense Studio.");
    Ok(())
}
