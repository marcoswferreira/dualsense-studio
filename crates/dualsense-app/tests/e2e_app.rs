use std::io::{BufRead, BufReader};
use std::net::SocketAddr;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use dualsense_core::triggers::TriggerEffect;
use dualsense_core::DualSenseManager;
use tokio::net::UdpSocket;

fn allocate_free_udp_port() -> u16 {
    let socket =
        std::net::UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0))).expect("bind ephemeral UDP");
    socket.local_addr().expect("read local addr").port()
}

/// E2E: Launches the compiled `dualsense-studio` application binary, verifies startup lifecycle
/// logs, confirms it stays alive waiting for shutdown signal, and terminates it cleanly.
#[test]
fn e2e_application_binary_starts_and_runs_cleanly() {
    let bin_path = env!("CARGO_BIN_EXE_dualsense-studio");
    let mut child = Command::new(bin_path)
        .env("RUST_LOG", "info")
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
    let mut saw_running = false;
    let deadline = Instant::now() + Duration::from_secs(5);

    while Instant::now() < deadline && !(saw_starting && saw_running) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining) {
            Ok(line) => {
                if line.contains("Starting DualSense Studio") {
                    saw_starting = true;
                }
                if line.contains("DualSense Studio is running") {
                    saw_running = true;
                }
            }
            Err(_) => break,
        }
    }

    // Confirm the process did not crash immediately after startup
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
        saw_running,
        "Expected 'DualSense Studio is running' log message from binary"
    );
    assert!(
        still_running,
        "Expected dualsense-studio process to remain running until terminated"
    );
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
