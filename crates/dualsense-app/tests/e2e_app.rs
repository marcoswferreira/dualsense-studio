use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use dualsense_app::server::start_http_server_with_config;
use dualsense_core::lightbar::{LightbarMode, RgbColor};
use dualsense_core::storage::AppConfig;
use dualsense_core::triggers::TriggerEffect;
use dualsense_core::{DualSenseManager, DualSenseStatus};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpStream, UdpSocket};

fn allocate_free_udp_port() -> u16 {
    let socket =
        std::net::UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0))).expect("bind ephemeral UDP");
    socket.local_addr().expect("read local addr").port()
}

async fn send_http_request(
    addr: SocketAddr,
    method: &str,
    path: &str,
    body: Option<&str>,
) -> (u16, String) {
    let mut stream = TcpStream::connect(addr).await.expect("connect HTTP API");
    let body_bytes = body.unwrap_or("").as_bytes();
    let req = format!(
        "{} {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        method,
        path,
        addr,
        body_bytes.len(),
        body.unwrap_or("")
    );
    stream
        .write_all(req.as_bytes())
        .await
        .expect("write HTTP request");
    stream.flush().await.expect("flush HTTP request");

    let mut resp = Vec::new();
    stream
        .read_to_end(&mut resp)
        .await
        .expect("read HTTP response");
    let text = String::from_utf8_lossy(&resp).to_string();
    let (headers, resp_body) = text
        .split_once("\r\n\r\n")
        .expect("valid HTTP response with CRLFCRLF");
    let status_code = headers
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|s| s.parse::<u16>().ok())
        .expect("parse HTTP status code");
    (status_code, resp_body.to_string())
}

/// E2E: Launches the compiled `dualsense-studio` application binary, verifies startup lifecycle
/// logs, confirms it stays alive waiting for shutdown signal, and terminates it cleanly.
#[test]
fn e2e_application_binary_starts_and_runs_cleanly() {
    let bin_path = env!("CARGO_BIN_EXE_dualsense-studio");
    let tmp_cfg = tempfile::tempdir().expect("tempdir for binary config");
    let cfg_file = tmp_cfg.path().join("config.json");
    let mut child = Command::new(bin_path)
        .env("RUST_LOG", "info")
        .env("DUALSENSE_API_ADDR", "127.0.0.1:0")
        .env("DUALSENSE_CONFIG_PATH", &cfg_file)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("failed to spawn dualsense-studio binary");

    let stdout = child.stdout.take().expect("capture stdout");
    let (tx, rx) = mpsc::channel::<String>();

    let reader_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if tx.send(line).is_err() {
                break;
            }
        }
    });

    let mut saw_starting = false;
    let mut saw_http = false;
    let mut saw_running = false;
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline && !(saw_starting && saw_http && saw_running) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(line) => {
                if line.contains("Starting DualSense Studio") {
                    saw_starting = true;
                }
                if line.contains("HTTP API listening on http://") {
                    saw_http = true;
                }
                if line.contains("DualSense Studio is running") {
                    saw_running = true;
                }
            }
            Err(_) => break,
        }
    }

    let still_running = child
        .try_wait()
        .expect("check child process status")
        .is_none();

    let _ = child.kill();
    let _ = child.wait();
    drop(rx);
    let _ = reader_handle.join();

    assert!(
        saw_starting,
        "Expected 'Starting DualSense Studio' log message from binary"
    );
    assert!(
        saw_http,
        "Expected HTTP API listening log message from binary"
    );
    assert!(
        saw_running,
        "Expected 'DualSense Studio is running' log message from binary"
    );
    assert!(
        still_running,
        "Expected dualsense-studio process to remain running until terminated"
    );
}

/// E2E: Verifies all HTTP API endpoints used by the React frontend against a live TCP server
/// including mapping, BT HD haptics, rumble gain, Cemuhook UDP, custom profiles, and disk persistence.
#[tokio::test]
async fn e2e_http_api_full_lifecycle() {
    let udp_port = allocate_free_udp_port();
    let cemu_port = allocate_free_udp_port();
    let manager = Arc::new(DualSenseManager::with_ports(udp_port, cemu_port));
    manager.set_extension_enabled(true);
    manager.set_connected(true, "Sony DualSense Wireless Controller");

    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg_path = tmp.path().join("config.json");

    let (addr, server_handle) =
        start_http_server_with_config("127.0.0.1:0", manager.clone(), Some(cfg_path.clone()))
            .await
            .expect("start HTTP API server");

    // 1. GET /api/dualsense/status & GET /api/dualsense/input
    let (code, body) = send_http_request(addr, "GET", "/api/dualsense/status", None).await;
    assert_eq!(code, 200);
    let status: DualSenseStatus = serde_json::from_str(&body).expect("parse DualSenseStatus");
    assert!(status.extension_enabled);
    assert!(status.connected);

    let live_in = dualsense_core::DualSenseInputState {
        left_stick_x: 220,
        l2_trigger: 180,
        cross: true,
        ..Default::default()
    };
    manager.update_input_state(live_in);
    let (code, input_body) = send_http_request(addr, "GET", "/api/dualsense/input", None).await;
    assert_eq!(code, 200);
    let input_val: serde_json::Value =
        serde_json::from_str(&input_body).expect("parse live input JSON");
    assert_eq!(input_val["connected"], true);
    // Default StickTuning applies a 0.05 radial inner deadzone: raw (220, 128) maps to (218, 128)
    assert_eq!(input_val["input"]["left_stick_x"], 218);
    assert_eq!(input_val["input"]["left_stick_y"], 128);
    assert_eq!(input_val["input"]["l2_trigger"], 180);
    assert_eq!(input_val["input"]["cross"], true);

    // 2. POST /api/dualsense/trigger
    let trigger_json = r#"{
        "left": { "mode": "feedback", "start_position": 2, "force": 6 },
        "right": { "mode": "weapon", "start_position": 1, "end_position": 6, "force": 8 }
    }"#;
    let (code, _) =
        send_http_request(addr, "POST", "/api/dualsense/trigger", Some(trigger_json)).await;
    assert_eq!(code, 200);
    assert_eq!(
        manager.get_triggers().right,
        TriggerEffect::Weapon {
            start_position: 1,
            end_position: 6,
            force: 8,
        }
    );

    // 3. POST /api/dualsense/lightbar
    let lightbar_json = r#"{
        "lightbar": { "mode": "static", "color": { "r": 255, "g": 20, "b": 40 } },
        "player_led": "player2",
        "mic_led": "off"
    }"#;
    let (code, _) =
        send_http_request(addr, "POST", "/api/dualsense/lightbar", Some(lightbar_json)).await;
    assert_eq!(code, 200);
    assert_eq!(
        manager.get_lighting().lightbar,
        LightbarMode::Static {
            color: RgbColor::new(255, 20, 40)
        }
    );

    // 4. POST /api/dualsense/mapping
    let mapping_json = r#"{
        "left_stick": { "inner_deadzone": 0.12, "outer_deadzone": 0.95, "anti_deadzone": 0.05, "curve": { "type": "s_curve" }, "invert_x": false, "invert_y": false },
        "right_stick": { "inner_deadzone": 0.08, "outer_deadzone": 1.0, "anti_deadzone": 0.0, "curve": { "type": "linear" }, "invert_x": false, "invert_y": false },
        "left_trigger": { "deadzone": 10, "max_threshold": 180, "sensitivity": 1.0 },
        "right_trigger": { "deadzone": 5, "max_threshold": 150, "sensitivity": 1.2 },
        "button_remaps": { "paddle_left": { "kind": "button", "target": "cross" } },
        "touchpad": { "mode": "mouse", "sensitivity": 1.5, "tap_to_click": true, "two_finger_scroll": true, "invert_scroll": false },
        "gyro_activation": "hold_l2",
        "gyro_smoothing": 0.25,
        "gyro_calibration_offset": [0, 0, 0]
    }"#;
    let (code, _) =
        send_http_request(addr, "POST", "/api/dualsense/mapping", Some(mapping_json)).await;
    assert_eq!(code, 200);
    assert!((manager.get_mapping().left_stick.inner_deadzone - 0.12).abs() < 1e-4);

    // 5. POST /api/dualsense/haptics & /api/dualsense/haptics/test & /api/dualsense/rumble
    let haptics_json = r#"{
        "mode": "bluetooth_hd_haptics",
        "enabled": true,
        "gain": 1.3,
        "lowpass_cutoff_hz": 200.0,
        "left_balance": 1.0,
        "right_balance": 0.9,
        "audio_source": "default_loopback"
    }"#;
    let (code, _) =
        send_http_request(addr, "POST", "/api/dualsense/haptics", Some(haptics_json)).await;
    assert_eq!(code, 200);
    assert!(manager.is_bt_hd_haptics_active());

    let (code, body) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/haptics/test",
        Some(r#"{"freq_hz":140,"amplitude":0.9,"duration_ms":100}"#),
    )
    .await;
    assert_eq!(code, 200);
    assert!(body.contains("\"queued_blocks\":"));

    let (code, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/rumble",
        Some(r#"{"gain":1.4,"heavy":100,"light":50}"#),
    )
    .await;
    assert_eq!(code, 200);
    assert!((manager.get_rumble_gain() - 1.4).abs() < 1e-4);

    // 6. POST /api/dualsense/cemuhook/toggle
    let (code, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/cemuhook/toggle",
        Some(r#"{"enabled":true}"#),
    )
    .await;
    assert_eq!(code, 200);
    assert!(manager.get_status().await.cemuhook_stats.running);
    let (code, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/cemuhook/toggle",
        Some(r#"{"enabled":false}"#),
    )
    .await;
    assert_eq!(code, 200);

    // 7. POST & DELETE /api/dualsense/profiles/custom + /api/dualsense/game-rules
    let custom_prof = r#"{
        "id": "custom_test",
        "name": "Custom Test",
        "description": "Test profile",
        "triggers": { "left": { "mode": "hard" }, "right": { "mode": "game_cube" } },
        "lightbar": { "r": 120, "g": 200, "b": 255 }
    }"#;
    let (code, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/profiles/custom",
        Some(custom_prof),
    )
    .await;
    assert_eq!(code, 200);

    let (code, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/profile",
        Some(r#"{"id":"custom_test"}"#),
    )
    .await;
    assert_eq!(code, 200);

    let (code_404, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/profile",
        Some(r#"{"id":"unknown"}"#),
    )
    .await;
    assert_eq!(code_404, 404);

    let (code, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/game-rules",
        Some(r#"{"rules":[{"process_name":"SpiderMan.exe","profile_id":"custom_test","enabled":true}]}"#),
    )
    .await;
    assert_eq!(code, 200);

    // Verify automatic persistence to disk
    let saved = AppConfig::load_from_path(&cfg_path);
    assert!((saved.rumble_gain - 1.4).abs() < 1e-4);
    assert_eq!(saved.custom_profiles.len(), 1);
    assert_eq!(saved.game_profile_rules.len(), 1);

    let (code, _) = send_http_request(
        addr,
        "DELETE",
        "/api/dualsense/profiles/custom",
        Some(r#"{"id":"custom_test"}"#),
    )
    .await;
    assert_eq!(code, 200);

    // 8. POST /api/dualsense/mic/toggle
    let (code, body) = send_http_request(addr, "POST", "/api/dualsense/mic/toggle", None).await;
    assert_eq!(code, 200);
    assert!(body.contains("\"mic_muted\":true"));

    // 9. POST /api/dualsense/udp/toggle (start & stop)
    let (code, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/udp/toggle",
        Some(r#"{"enabled":true}"#),
    )
    .await;
    assert_eq!(code, 200);
    assert!(manager.get_status().await.udp_stats.running);

    let (code, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/udp/toggle",
        Some(r#"{"enabled":false}"#),
    )
    .await;
    assert_eq!(code, 200);
    assert!(!manager.get_status().await.udp_stats.running);

    // 10. POST /api/dualsense/extension/toggle
    let (code, _) = send_http_request(
        addr,
        "POST",
        "/api/dualsense/extension/toggle",
        Some(r#"{"enabled":false,"autostart":true}"#),
    )
    .await;
    assert_eq!(code, 200);
    assert!(!manager.is_extension_enabled());
    assert!(manager.is_autostart_enabled());

    server_handle.abort();
}

/// E2E: Verifies full app-level runtime integration with DualSenseManager and live UDP link
#[tokio::test]
async fn e2e_app_runtime_manager_and_udp_integration() {
    let port = allocate_free_udp_port();
    let manager = DualSenseManager::with_udp_port(port);
    manager.set_extension_enabled(true);
    manager.set_connected(true, "Sony DualSense Wireless Controller");
    manager.start_udp().await.expect("start app UDP server");

    let client = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind test client");
    let target = SocketAddr::from(([127, 0, 0, 1], port));

    let payload = r#"{
        "instructions": [
            { "type": 1, "parameters": [0, 2, 14] },
            { "type": 2, "parameters": [0, 10, 200, 150] }
        ]
    }"#;
    client
        .send_to(payload.as_bytes(), target)
        .await
        .expect("send UDP packet");

    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if manager.get_status().await.udp_stats.packets_received >= 1 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    let status = manager.get_status().await;
    assert_eq!(status.udp_stats.packets_received, 1);
    assert_eq!(status.triggers.right, TriggerEffect::Hard);

    let usb_report = manager.build_current_output_report().build_usb();
    assert_eq!(usb_report[0], 0x02);
    assert_eq!(usb_report[11], 0x01); // Hard preset opcode
    assert_eq!(usb_report[45..48], [10, 200, 150]);

    manager.stop_udp().await;
    assert!(!manager.get_status().await.udp_stats.running);
}
