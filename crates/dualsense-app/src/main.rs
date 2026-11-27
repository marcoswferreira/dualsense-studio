#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::Result;
use dualsense_app::process_watcher::ProcessProfileWatcher;
use dualsense_app::server::{
    is_existing_dualsense_server, start_http_server_with_fallback_and_shutdown, DEFAULT_API_ADDR,
};
use dualsense_app::single_instance::{
    read_active_instance_url, write_active_instance_url, SingleInstanceGuard,
};
use dualsense_app::tray::{hide_console_window_if_windows, open_browser, TrayHandle};
use dualsense_app::updater::check_for_updates;
use dualsense_core::{default_config_path, AppConfig, DualSenseManager, HidDeviceWorker};
use tracing::{info, warn};
use tracing_subscriber::fmt::writer::MakeWriter;
use tracing_subscriber::EnvFilter;

#[derive(Clone)]
struct TeeLogWriter {
    file: Option<Arc<Mutex<std::fs::File>>>,
}

struct TeeGuard {
    file: Option<Arc<Mutex<std::fs::File>>>,
}

impl Write for TeeGuard {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let _ = std::io::stdout().write(buf);
        if let Some(f) = &self.file {
            if let Ok(mut guard) = f.lock() {
                let _ = guard.write_all(buf);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let _ = std::io::stdout().flush();
        if let Some(f) = &self.file {
            if let Ok(mut guard) = f.lock() {
                let _ = guard.flush();
            }
        }
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for TeeLogWriter {
    type Writer = TeeGuard;

    fn make_writer(&'a self) -> Self::Writer {
        TeeGuard {
            file: self.file.clone(),
        }
    }
}

fn open_log_file(config_path: &Path) -> Option<Arc<Mutex<std::fs::File>>> {
    let dir = config_path.parent()?;
    std::fs::create_dir_all(dir).ok()?;
    let log_path = dir.join("dualsense-studio.log");
    let truncate = std::fs::metadata(&log_path)
        .map(|m| m.len() > 1024 * 1024)
        .unwrap_or(false);
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(truncate)
        .append(!truncate)
        .open(log_path)
        .ok()?;
    Some(Arc::new(Mutex::new(file)))
}

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--tray" || a == "--minimized") {
        hide_console_window_if_windows();
    }

    let config_path: PathBuf = std::env::var("DUALSENSE_CONFIG_PATH")
        .map(PathBuf::from)
        .unwrap_or_else(|_| default_config_path());

    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let log_writer = TeeLogWriter {
        file: open_log_file(&config_path),
    };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(false)
        .with_writer(log_writer)
        .init();

    info!("Starting DualSense Studio v0.1.0 (Standalone Mode)...");

    let api_addr =
        std::env::var("DUALSENSE_API_ADDR").unwrap_or_else(|_| DEFAULT_API_ADDR.to_string());
    let bypass_single_instance = api_addr == "127.0.0.1:0"
        || std::env::var("DUALSENSE_DISABLE_SINGLE_INSTANCE").is_ok_and(|v| v == "1");
    let _instance_guard = SingleInstanceGuard::try_acquire(bypass_single_instance);
    if !bypass_single_instance && !_instance_guard.is_primary() {
        let target_url = read_active_instance_url().unwrap_or_else(|| format!("http://{api_addr}"));
        info!(
            "DualSense Studio is already running; opening existing UI at {}",
            target_url
        );
        open_browser(&target_url);
        return Ok(());
    }

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

    if is_existing_dualsense_server(&api_addr).await {
        info!("DualSense Studio is already running at http://{}", api_addr);
        if args.iter().any(|a| a == "--open-ui") {
            open_browser(&format!("http://{}", api_addr));
        }
        hid_worker.stop();
        process_watcher.stop();
        return Ok(());
    }

    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    let (bound_addr, http_handle) = start_http_server_with_fallback_and_shutdown(
        &api_addr,
        manager.clone(),
        Some(config_path.clone()),
        Some(shutdown_tx.clone()),
    )
    .await?;

    if !bypass_single_instance {
        write_active_instance_url(bound_addr);
    }

    tokio::spawn(async {
        if let Ok(info) = check_for_updates().await {
            if info.update_available {
                info!(
                    "Update available: {} (current: {}). Download at {}",
                    info.latest_version, info.current_version, info.release_url
                );
            }
        }
    });

    let no_tray = args.iter().any(|a| a == "--no-tray");
    let mut tray_handle = if no_tray {
        None
    } else {
        Some(TrayHandle::start(
            manager.clone(),
            bound_addr,
            config_path,
            shutdown_tx,
        ))
    };

    if args.iter().any(|a| a == "--open-ui") {
        open_browser(&format!("http://{}", bound_addr));
    }

    info!("DualSense Studio is running. Press Ctrl+C or use the system tray to exit.");
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {}
        _ = async {
            while !*shutdown_rx.borrow() {
                if shutdown_rx.changed().await.is_err() {
                    std::future::pending::<()>().await;
                }
            }
        } => {}
    }

    info!("Shutting down DualSense Studio.");
    if let Some(mut tray) = tray_handle.take() {
        tray.stop();
    }
    http_handle.abort();
    process_watcher.stop();
    manager.stop_udp().await;
    manager.stop_cemuhook().await;
    manager.stop_usbip_virtual_dualsense().await;
    hid_worker.stop();
    Ok(())
}
