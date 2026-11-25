use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use dualsense_core::{
    DualSenseManager, DualTriggerConfig, EmulationConfig, EmulationMode, GameProfileRule,
    HapticsConfig, InputMappingConfig, LightingConfig, TriggerProfile,
};
use serde::Deserialize;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tracing::{debug, info, warn};

pub const DEFAULT_API_ADDR: &str = "127.0.0.1:48080";

#[derive(Debug, Deserialize)]
struct ToggleRequest {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
struct ProfileRequest {
    id: String,
}

#[derive(Debug, Deserialize)]
struct ExtensionToggleRequest {
    enabled: Option<bool>,
    autostart: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct RumbleRequest {
    heavy: Option<u8>,
    light: Option<u8>,
    gain: Option<f32>,
    duration_ms: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct HapticPulseRequest {
    freq_hz: Option<f32>,
    amplitude: Option<f32>,
    duration_ms: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct GameRulesRequest {
    rules: Vec<GameProfileRule>,
}

/// Starts the HTTP API server on `bind_addr` without disk persistence (convenient for tests)
pub async fn start_http_server(
    bind_addr: &str,
    manager: Arc<DualSenseManager>,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    start_http_server_with_config(bind_addr, manager, None).await
}

/// Starts the HTTP API server on `bind_addr`, automatically persisting `AppConfig` to `config_path`
pub async fn start_http_server_with_config(
    bind_addr: &str,
    manager: Arc<DualSenseManager>,
    config_path: Option<PathBuf>,
) -> Result<(SocketAddr, tokio::task::JoinHandle<()>)> {
    let listener = TcpListener::bind(bind_addr).await?;
    let local_addr = listener.local_addr()?;
    info!(
        "DualSense Studio HTTP API listening on http://{}",
        local_addr
    );

    let cfg_path = Arc::new(config_path);

    let handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _peer)) => {
                    let mgr = manager.clone();
                    let path = cfg_path.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_connection(stream, mgr, path).await {
                            debug!("HTTP connection error: {}", e);
                        }
                    });
                }
                Err(e) => {
                    warn!("HTTP accept error: {}", e);
                }
            }
        }
    });

    Ok((local_addr, handle))
}

async fn persist_if_configured(manager: &Arc<DualSenseManager>, config_path: &Option<PathBuf>) {
    if let Some(path) = config_path.as_ref() {
        let cfg = manager.export_config().await;
        if let Err(e) = cfg.save_to_path(path) {
            warn!("Failed to persist config to {}: {}", path.display(), e);
        }
    }
}

async fn handle_connection(
    mut stream: TcpStream,
    manager: Arc<DualSenseManager>,
    config_path: Arc<Option<PathBuf>>,
) -> Result<()> {
    let mut buf = Vec::with_capacity(4096);
    let mut chunk = [0u8; 2048];

    let header_end = loop {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            return Ok(());
        }
        buf.extend_from_slice(&chunk[..n]);
        if let Some(pos) = find_header_end(&buf) {
            break pos;
        }
        if buf.len() > 64 * 1024 {
            write_response(
                &mut stream,
                413,
                "Payload Too Large",
                "application/json",
                b"{\"error\":\"headers too large\"}",
            )
            .await?;
            return Ok(());
        }
    };

    let (method, path, content_length) = {
        let header_bytes = &buf[..header_end];
        let header_text = String::from_utf8_lossy(header_bytes);
        let mut lines = header_text.lines();
        let request_line = match lines.next() {
            Some(line) => line,
            None => return Ok(()),
        };

        let mut parts = request_line.split_whitespace();
        let method = parts.next().unwrap_or("").to_string();
        let raw_path = parts.next().unwrap_or("/");
        let path = raw_path.split('?').next().unwrap_or("/").to_string();

        let mut content_length: usize = 0;
        for line in lines {
            if let Some((name, val)) = line.split_once(':') {
                if name.trim().eq_ignore_ascii_case("content-length") {
                    content_length = val.trim().parse::<usize>().unwrap_or(0);
                }
            }
        }
        (method, path, content_length)
    };

    let body_start = header_end + 4;
    if content_length > 1024 * 1024 {
        write_response(
            &mut stream,
            413,
            "Payload Too Large",
            "application/json",
            b"{\"error\":\"body too large\"}",
        )
        .await?;
        return Ok(());
    }

    while buf.len() < body_start + content_length {
        let n = stream.read(&mut chunk).await?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
    }

    let body_end = (body_start + content_length).min(buf.len());
    let body = &buf[body_start..body_end];

    let (status, reason, content_type, resp_body) =
        route_request(&method, &path, body, &manager, &config_path).await;
    write_response(&mut stream, status, reason, content_type, &resp_body).await?;
    Ok(())
}

fn find_header_end(buf: &[u8]) -> Option<usize> {
    buf.windows(4).position(|w| w == b"\r\n\r\n")
}

async fn route_request(
    method: &str,
    path: &str,
    body: &[u8],
    manager: &Arc<DualSenseManager>,
    config_path: &Option<PathBuf>,
) -> (u16, &'static str, &'static str, Vec<u8>) {
    if method == "OPTIONS" {
        return (204, "No Content", "text/plain", Vec::new());
    }

    match (method, path) {
        ("GET", "/api/dualsense/status") => {
            let status = manager.get_status().await;
            json_response(200, "OK", &status)
        }
        ("GET", "/api/dualsense/input") => {
            let payload = json!({
                "connected": manager.is_connected(),
                "name": manager.controller_name(),
                "input": manager.get_input_state(),
            });
            json_response(200, "OK", &payload)
        }
        ("POST", "/api/dualsense/trigger") => {
            match serde_json::from_slice::<DualTriggerConfig>(body) {
                Ok(triggers) => {
                    manager.set_triggers(triggers);
                    persist_if_configured(manager, config_path).await;
                    json_response(200, "OK", &json!({ "ok": true }))
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("POST", "/api/dualsense/lightbar") => match serde_json::from_slice::<LightingConfig>(body)
        {
            Ok(lighting) => {
                manager.set_lighting(lighting);
                persist_if_configured(manager, config_path).await;
                json_response(200, "OK", &json!({ "ok": true }))
            }
            Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
        },
        ("POST", "/api/dualsense/emulation") => {
            match serde_json::from_slice::<EmulationConfig>(body) {
                Ok(emulation) => {
                    let is_vds = emulation.mode == EmulationMode::DualSense;
                    let applied = manager.set_emulation(emulation);
                    if is_vds {
                        let _ = manager.start_usbip_virtual_dualsense().await;
                    } else {
                        manager.stop_usbip_virtual_dualsense().await;
                    }
                    persist_if_configured(manager, config_path).await;
                    json_response(
                        200,
                        "OK",
                        &json!({
                            "ok": true,
                            "driver_attached": applied,
                            "virtual_driver": manager.get_virtual_driver_status()
                        }),
                    )
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("POST", "/api/dualsense/mapping") => {
            match serde_json::from_slice::<InputMappingConfig>(body) {
                Ok(mapping) => {
                    manager.set_mapping(mapping);
                    persist_if_configured(manager, config_path).await;
                    json_response(200, "OK", &json!({ "ok": true }))
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("POST", "/api/dualsense/haptics") => match serde_json::from_slice::<HapticsConfig>(body) {
            Ok(haptics) => {
                manager.set_haptics(haptics);
                persist_if_configured(manager, config_path).await;
                json_response(200, "OK", &json!({ "ok": true }))
            }
            Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
        },
        ("POST", "/api/dualsense/haptics/test") => {
            let req =
                serde_json::from_slice::<HapticPulseRequest>(body).unwrap_or(HapticPulseRequest {
                    freq_hz: Some(120.0),
                    amplitude: Some(0.8),
                    duration_ms: Some(250),
                });
            let blocks = manager.trigger_hd_haptic_pulse(
                req.freq_hz.unwrap_or(120.0),
                req.amplitude.unwrap_or(0.8),
                req.duration_ms.unwrap_or(250),
            );
            json_response(200, "OK", &json!({ "ok": true, "queued_blocks": blocks }))
        }
        ("POST", "/api/dualsense/rumble") => match serde_json::from_slice::<RumbleRequest>(body) {
            Ok(req) => {
                if let Some(gain) = req.gain {
                    manager.set_rumble_gain(gain);
                    persist_if_configured(manager, config_path).await;
                }
                if req.heavy.is_some() || req.light.is_some() {
                    manager.set_rumble_direct(req.heavy.unwrap_or(0), req.light.unwrap_or(0));
                    if let Some(ms) = req.duration_ms {
                        let mgr = manager.clone();
                        tokio::spawn(async move {
                            tokio::time::sleep(std::time::Duration::from_millis(ms.min(5000)))
                                .await;
                            mgr.set_rumble_direct(0, 0);
                        });
                    }
                }
                json_response(
                    200,
                    "OK",
                    &json!({ "ok": true, "rumble_gain": manager.get_rumble_gain() }),
                )
            }
            Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
        },
        ("POST", "/api/dualsense/udp/toggle") => {
            match serde_json::from_slice::<ToggleRequest>(body) {
                Ok(req) => {
                    if req.enabled {
                        match manager.start_udp().await {
                            Ok(()) => {
                                persist_if_configured(manager, config_path).await;
                                json_response(200, "OK", &json!({ "ok": true, "running": true }))
                            }
                            Err(e) => json_response(
                                500,
                                "Internal Server Error",
                                &json!({ "error": e.to_string() }),
                            ),
                        }
                    } else {
                        manager.stop_udp().await;
                        persist_if_configured(manager, config_path).await;
                        json_response(200, "OK", &json!({ "ok": true, "running": false }))
                    }
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("POST", "/api/dualsense/cemuhook/toggle") => {
            match serde_json::from_slice::<ToggleRequest>(body) {
                Ok(req) => {
                    if req.enabled {
                        match manager.start_cemuhook().await {
                            Ok(()) => {
                                persist_if_configured(manager, config_path).await;
                                json_response(200, "OK", &json!({ "ok": true, "running": true }))
                            }
                            Err(e) => json_response(
                                500,
                                "Internal Server Error",
                                &json!({ "error": e.to_string() }),
                            ),
                        }
                    } else {
                        manager.stop_cemuhook().await;
                        persist_if_configured(manager, config_path).await;
                        json_response(200, "OK", &json!({ "ok": true, "running": false }))
                    }
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("POST", "/api/dualsense/mic/toggle") => {
            let muted = manager.toggle_microphone();
            json_response(200, "OK", &json!({ "ok": true, "mic_muted": muted }))
        }
        ("POST", "/api/dualsense/profile") => {
            match serde_json::from_slice::<ProfileRequest>(body) {
                Ok(req) => {
                    if manager.apply_profile(&req.id) {
                        persist_if_configured(manager, config_path).await;
                        json_response(200, "OK", &json!({ "ok": true, "id": req.id }))
                    } else {
                        json_response(
                            404,
                            "Not Found",
                            &json!({ "ok": false, "error": "profile not found" }),
                        )
                    }
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("POST", "/api/dualsense/profiles/custom") => {
            match serde_json::from_slice::<TriggerProfile>(body) {
                Ok(profile) => {
                    manager.add_custom_profile(profile);
                    persist_if_configured(manager, config_path).await;
                    json_response(200, "OK", &json!({ "ok": true }))
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("DELETE", "/api/dualsense/profiles/custom") => {
            match serde_json::from_slice::<ProfileRequest>(body) {
                Ok(req) => {
                    let deleted = manager.delete_custom_profile(&req.id);
                    if deleted {
                        persist_if_configured(manager, config_path).await;
                        json_response(200, "OK", &json!({ "ok": true }))
                    } else {
                        json_response(404, "Not Found", &json!({ "error": "profile not found" }))
                    }
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("POST", "/api/dualsense/game-rules") => {
            match serde_json::from_slice::<GameRulesRequest>(body) {
                Ok(req) => {
                    manager.set_game_profile_rules(req.rules);
                    persist_if_configured(manager, config_path).await;
                    json_response(200, "OK", &json!({ "ok": true }))
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("POST", "/api/dualsense/extension/toggle") => {
            match serde_json::from_slice::<ExtensionToggleRequest>(body) {
                Ok(req) => {
                    if let Some(enabled) = req.enabled {
                        manager.set_extension_enabled(enabled);
                    }
                    if let Some(autostart) = req.autostart {
                        manager.set_autostart_enabled(autostart);
                    }
                    persist_if_configured(manager, config_path).await;
                    json_response(
                        200,
                        "OK",
                        &json!({
                            "ok": true,
                            "extension_enabled": manager.is_extension_enabled(),
                            "autostart_on_launch": manager.is_autostart_enabled()
                        }),
                    )
                }
                Err(e) => json_response(400, "Bad Request", &json!({ "error": e.to_string() })),
            }
        }
        ("POST", "/api/settings") => json_response(200, "OK", &json!({ "ok": true })),
        ("GET", _) => serve_static_asset(path),
        _ => json_response(404, "Not Found", &json!({ "error": "not found" })),
    }
}

fn resolve_ui_asset(rel: &str) -> Option<PathBuf> {
    // 1. Standalone release layout: `<executable_dir>/ui/<asset>`
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let portable = exe_dir.join("ui").join(rel);
            if portable.is_file() {
                return Some(portable);
            }
        }
    }
    // 2. Current working directory `./ui/<asset>`
    let cwd_candidate = PathBuf::from("ui").join(rel);
    if cwd_candidate.is_file() {
        return Some(cwd_candidate);
    }
    // 3. Development cargo workspace fallback
    let dev_candidate = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("ui")
        .join(rel);
    if dev_candidate.is_file() {
        return Some(dev_candidate);
    }
    None
}

fn serve_static_asset(path: &str) -> (u16, &'static str, &'static str, Vec<u8>) {
    let rel = match path {
        "/" | "" => "index.html",
        other => other.trim_start_matches('/'),
    };
    if rel.contains("..") {
        return json_response(400, "Bad Request", &json!({ "error": "invalid path" }));
    }

    if let Some(candidate) = resolve_ui_asset(rel) {
        if let Ok(bytes) = std::fs::read(&candidate) {
            let mime = if rel.ends_with(".html") {
                "text/html; charset=utf-8"
            } else if rel.ends_with(".js") {
                "application/javascript; charset=utf-8"
            } else if rel.ends_with(".css") {
                "text/css; charset=utf-8"
            } else if rel.ends_with(".svg") {
                "image/svg+xml"
            } else {
                "application/octet-stream"
            };
            return (200, "OK", mime, bytes);
        }
    }

    json_response(404, "Not Found", &json!({ "error": "not found" }))
}

fn json_response<T: serde::Serialize>(
    status: u16,
    reason: &'static str,
    payload: &T,
) -> (u16, &'static str, &'static str, Vec<u8>) {
    let bytes = serde_json::to_vec(payload).unwrap_or_else(|_| b"{}".to_vec());
    (status, reason, "application/json", bytes)
}

async fn write_response(
    stream: &mut TcpStream,
    status: u16,
    reason: &str,
    content_type: &str,
    body: &[u8],
) -> Result<()> {
    let header = format!(
        "HTTP/1.1 {} {}\r\nContent-Type: {}\r\nContent-Length: {}\r\nAccess-Control-Allow-Origin: *\r\nAccess-Control-Allow-Methods: GET, POST, DELETE, OPTIONS\r\nAccess-Control-Allow-Headers: Content-Type\r\nConnection: close\r\n\r\n",
        status,
        reason,
        content_type,
        body.len()
    );
    stream.write_all(header.as_bytes()).await?;
    stream.write_all(body).await?;
    stream.flush().await?;
    Ok(())
}
