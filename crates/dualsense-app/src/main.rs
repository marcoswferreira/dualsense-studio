use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use dualsense_app::process_watcher::ProcessProfileWatcher;
use dualsense_app::server::{start_http_server_with_config, DEFAULT_API_ADDR};
use dualsense_core::{default_config_path, AppConfig, DualSenseManager, HidDeviceWorker};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd")
        .args(["/C", "start", "", url])
        .spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
}

#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt().with_env_filter(filter).init();

    info!("Starting DualSense Studio v0.1.0 (Standalone Mode)...");

    let config_path: PathBuf = std::env::var("DUALSENSE_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_config_path());
    let persisted = AppConfig::load_from_path(&config_path);

    let manager = Arc::new(DualSenseManager::new());
    manager.apply_config(&persisted);
    manager.set_extension_enabled(true);

    if persisted.udp_enabled {
        if let Err(e) = manager.start_udp().await {
            warn!("Could not autostart DualSenseX UDP server: {}", e);
        }
    }
    if persisted.cemuhook_enabled {
        if let Err(e) = manager.start_cemuhook().await {
            warn!("Could not autostart Cemuhook Motion UDP server: {}", e);
        }
    }

    let hid_worker = HidDeviceWorker::start(manager.clone());
    let mut process_watcher = ProcessProfileWatcher::start(manager.clone());

    let api_addr =
        std::env::var("DUALSENSE_API_ADDR").unwrap_or_else(|_| DEFAULT_API_ADDR.to_string());
    let (bound_addr, http_handle) =
        start_http_server_with_config(&api_addr, manager.clone(), Some(config_path)).await?;

    if std::env::args().any(|a| a == "--open-ui") {
        open_browser(&format!("http://{}", bound_addr));
    }

    info!("DualSense Studio is running. Press Ctrl+C to terminate.");
    tokio::signal::ctrl_c().await?;

    info!("Shutting down DualSense Studio.");
    http_handle.abort();
    process_watcher.stop();
    manager.stop_udp().await;
    manager.stop_cemuhook().await;
    manager.stop_usbip_virtual_dualsense().await;
    hid_worker.stop();
    Ok(())
}
