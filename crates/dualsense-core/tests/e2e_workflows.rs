use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use byteorder::{ByteOrder, LittleEndian};
use crc32fast::Hasher;
use dualsense_core::audio::{
    get_set_mic_mute_command, get_switch_playback_device_command, get_toggle_mic_command,
};
use dualsense_core::device::{
    detect_connection_from_input_report, detect_connection_type, identify_controller,
    is_supported_sony_controller, DUALSENSE_PID, SONY_VENDOR_ID,
};
use dualsense_core::emulation::{EmulationConfig, EmulationMode, GyroSettings};
use dualsense_core::lightbar::{LightbarMode, LightingConfig, MicLed, PlayerLed, RgbColor};
use dualsense_core::manager::{DualSenseManager, DualSenseStatus};
use dualsense_core::profiles::TriggerProfile;
use dualsense_core::protocol::{
    parse_input_report, ConnectionType, OutputReportBuilder, BT_OUTPUT_REPORT_SIZE,
    USB_OUTPUT_REPORT_SIZE,
};
use dualsense_core::triggers::{DualTriggerConfig, TriggerEffect};
use dualsense_core::vigem::{map_input_to_ds4, map_input_to_xusb};
use tokio::net::UdpSocket;

fn allocate_free_udp_port() -> u16 {
    let socket =
        std::net::UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0))).expect("bind ephemeral UDP");
    socket.local_addr().expect("read local addr").port()
}

async fn wait_for_condition<F, Fut>(timeout: Duration, mut check: F) -> bool
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = bool>,
{
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if check().await {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

fn verify_bt_crc32(report: &[u8; BT_OUTPUT_REPORT_SIZE]) {
    let expected_crc = LittleEndian::read_u32(&report[74..78]);
    let mut hasher = Hasher::new();
    hasher.update(&[0xA2]);
    hasher.update(&report[0..74]);
    assert_eq!(
        hasher.finalize(),
        expected_crc,
        "Bluetooth 0x31 report CRC32 mismatch"
    );
}

#[allow(clippy::too_many_arguments)]
fn build_usb_input_report(
    lx: u8,
    ly: u8,
    rx: u8,
    ry: u8,
    l2: u8,
    r2: u8,
    buttons1: u8,
    buttons2: u8,
    buttons3: u8,
    gyro: (i16, i16, i16),
    accel: (i16, i16, i16),
    touch_active: bool,
    touch_id: u8,
    touch_x: u16,
    touch_y: u16,
    battery_byte: u8,
) -> [u8; 64] {
    let mut raw = [0u8; 64];
    raw[0] = 0x01; // USB Report ID
    raw[1] = lx;
    raw[2] = ly;
    raw[3] = rx;
    raw[4] = ry;
    raw[5] = l2;
    raw[6] = r2;
    raw[8] = buttons1;
    raw[9] = buttons2;
    raw[10] = buttons3;

    // Telemetry offsets for USB (offset = 1)
    LittleEndian::write_i16(&mut raw[16..18], gyro.0);
    LittleEndian::write_i16(&mut raw[18..20], gyro.1);
    LittleEndian::write_i16(&mut raw[20..22], gyro.2);
    LittleEndian::write_i16(&mut raw[22..24], accel.0);
    LittleEndian::write_i16(&mut raw[24..26], accel.1);
    LittleEndian::write_i16(&mut raw[26..28], accel.2);

    // Touchpoint 1 at offset + 32 = 33..37
    raw[33] = (touch_id & 0x7F) | if touch_active { 0x00 } else { 0x80 };
    raw[34] = (touch_x & 0xFF) as u8;
    raw[35] = (((touch_x >> 8) & 0x0F) as u8) | (((touch_y & 0x0F) as u8) << 4);
    raw[36] = ((touch_y >> 4) & 0xFF) as u8;

    // Battery at offset + 52 = 53
    raw[53] = battery_byte;
    raw
}

#[allow(clippy::too_many_arguments)]
fn build_bt_extended_input_report(
    lx: u8,
    ly: u8,
    rx: u8,
    ry: u8,
    l2: u8,
    r2: u8,
    buttons1: u8,
    buttons2: u8,
    buttons3: u8,
    battery_byte: u8,
) -> [u8; 78] {
    let mut raw = [0u8; 78];
    raw[0] = 0x31; // Bluetooth Extended Report ID
    raw[1] = 0x00;
    // offset = 2
    raw[2] = lx;
    raw[3] = ly;
    raw[4] = rx;
    raw[5] = ry;
    raw[6] = l2;
    raw[7] = r2;
    raw[9] = buttons1;
    raw[10] = buttons2;
    raw[11] = buttons3;
    // Battery at offset + 52 = 54
    raw[54] = battery_byte;
    raw
}

/// E2E: Real loopback UDP datagrams -> DualSenseUdpServer -> DualSenseManager -> USB & BT HID output reports
#[tokio::test]
async fn e2e_udp_datagram_to_hid_output_report_pipeline() {
    let port = allocate_free_udp_port();
    let manager = DualSenseManager::with_udp_port(port);
    manager.set_extension_enabled(true);
    manager.set_connected(true, "Sony DualSense Wireless Controller");

    manager
        .start_udp()
        .await
        .expect("UDP server should bind free loopback port");

    let status_started = manager.get_status().await;
    assert!(status_started.udp_stats.running);
    assert_eq!(status_started.udp_stats.port, port);
    assert_eq!(status_started.udp_stats.packets_received, 0);

    let client = UdpSocket::bind("127.0.0.1:0")
        .await
        .expect("bind client UDP socket");
    let target = SocketAddr::from(([127, 0, 0, 1], port));

    // 1. Send standard DualSenseX instruction packet (L2 Bow, R2 Weapon, RGB Orange, Rumble)
    let packet1 = r#"{
        "instructions": [
            { "type": 1, "parameters": [0, 1, 3, 1, 7, 4, 8] },
            { "type": 1, "parameters": [0, 2, 5, 2, 6, 7] },
            { "type": 2, "parameters": [0, 255, 100, 25] },
            { "type": 4, "parameters": [0, 210, 95] }
        ]
    }"#;
    client
        .send_to(packet1.as_bytes(), target)
        .await
        .expect("send packet1");

    let received1 = wait_for_condition(Duration::from_secs(2), || async {
        manager.get_status().await.udp_stats.packets_received >= 1
    })
    .await;
    assert!(received1, "UDP server did not receive packet1 in time");

    let status1 = manager.get_status().await;
    assert_eq!(status1.udp_stats.packets_received, 1);
    assert!(status1.udp_stats.last_packet_time.is_some());
    assert!(status1
        .udp_stats
        .last_sender
        .as_deref()
        .is_some_and(|s| s.starts_with("127.0.0.1:")));
    assert_eq!(
        status1.triggers.left,
        TriggerEffect::Bow {
            start_position: 1,
            end_position: 7,
            force: 4,
            snap_force: 8,
        }
    );
    assert_eq!(
        status1.triggers.right,
        TriggerEffect::Weapon {
            start_position: 2,
            end_position: 6,
            force: 7,
        }
    );

    // Verify compiled USB (0x02) and Bluetooth (0x31) HID output reports match the UDP payload
    let builder = manager.build_current_output_report().with_seq(5);
    let usb = builder.build_usb();
    assert_eq!(usb.len(), USB_OUTPUT_REPORT_SIZE);
    assert_eq!(usb[0], 0x02);
    assert_eq!(usb[2] & 0x08, 0, "RELEASE_LEDS bit must stay clear");
    assert_eq!(usb[3], 95); // light rumble
    assert_eq!(usb[4], 210); // heavy rumble
    assert_eq!(usb[11], 0x25); // R2 Weapon(start=2, end=6, force=7 -> force_val=6)
    assert_eq!(LittleEndian::read_u16(&usb[12..14]), (1 << 2) | (1 << 6));
    assert_eq!(usb[14], 6);
    assert_eq!(usb[22], 0x22); // L2 Bow(start=1, end=7, force=4->3, snap=8->7)
    assert_eq!(LittleEndian::read_u16(&usb[23..25]), (1 << 1) | (1 << 7));
    assert_eq!(usb[25], (7 << 3) | 3);
    assert_eq!(usb[45..48], [255, 100, 25]); // RGB

    let bt = builder.build_bluetooth();
    assert_eq!(bt.len(), BT_OUTPUT_REPORT_SIZE);
    assert_eq!(bt[0], 0x31);
    assert_eq!(bt[1], (5 << 4) | 0x02);
    assert_eq!(bt[4] & 0x08, 0, "RELEASE_LEDS bit must stay clear on BT");
    assert_eq!(bt[5], 95);
    assert_eq!(bt[6], 210);
    assert_eq!(bt[13], 0x25);
    assert_eq!(bt[24], 0x22);
    assert_eq!(bt[47..50], [255, 100, 25]);
    verify_bt_crc32(&bt);

    // 2. Send PascalCase DualSenseX mod packet updating presets (GameCube & Hardest)
    let packet2 = r#"{
        "Instructions": [
            { "Type": 1, "Parameters": [0, 1, 19] },
            { "Type": 1, "Parameters": [0, 2, 16] }
        ]
    }"#;
    client
        .send_to(packet2.as_bytes(), target)
        .await
        .expect("send packet2");

    let received2 = wait_for_condition(Duration::from_secs(2), || async {
        manager.get_status().await.udp_stats.packets_received >= 2
    })
    .await;
    assert!(received2, "UDP server did not receive packet2 in time");

    let triggers2 = manager.get_triggers();
    assert_eq!(triggers2.left, TriggerEffect::GameCube);
    assert_eq!(triggers2.right, TriggerEffect::Hardest);

    // 3. Send Reset instruction over UDP and verify complete state reset
    let reset_packet = r#"{ "instructions": [{ "type": 5, "parameters": [] }] }"#;
    client
        .send_to(reset_packet.as_bytes(), target)
        .await
        .expect("send reset packet");

    let received3 = wait_for_condition(Duration::from_secs(2), || async {
        manager.get_status().await.udp_stats.packets_received >= 3
    })
    .await;
    assert!(received3, "UDP server did not receive reset packet in time");

    let reset_usb = manager.build_current_output_report().build_usb();
    assert_eq!(reset_usb[3], 0);
    assert_eq!(reset_usb[4], 0);
    assert_eq!(reset_usb[11], 0x05); // Off
    assert_eq!(reset_usb[22], 0x05); // Off
    assert_eq!(
        reset_usb[45..48],
        [
            RgbColor::SONY_BLUE.r,
            RgbColor::SONY_BLUE.g,
            RgbColor::SONY_BLUE.b
        ]
    );

    manager.stop_udp().await;
    assert!(!manager.get_status().await.udp_stats.running);
}

/// E2E: Raw USB & Bluetooth HID input reports -> transport detection -> parsing -> manager state ->
/// mic mute callback & LED sync -> battery-driven lightbar -> XInput & DS4 virtual reports
#[tokio::test]
async fn e2e_raw_hid_input_to_manager_emulation_and_output_pipeline() {
    // 1. Hardware discovery and transport detection
    assert!(is_supported_sony_controller(SONY_VENDOR_ID, DUALSENSE_PID));
    let (name, is_dualsense) = identify_controller(DUALSENSE_PID);
    assert!(is_dualsense);
    assert_eq!(name, "Sony DualSense Wireless Controller");

    let initial_transport = detect_connection_type(r"\\?\hid#vid_054c&pid_0ce6&mi_03#7", 3);
    assert_eq!(initial_transport, ConnectionType::Usb);

    let manager = DualSenseManager::with_udp_port(allocate_free_udp_port());
    manager.set_extension_enabled(true);
    manager.set_connected(true, name);

    // Enable BatteryLevel lightbar mode + Player 3 LED
    manager.set_lighting(LightingConfig {
        lightbar: LightbarMode::BatteryLevel,
        player_led: PlayerLed::Player3,
        mic_led: MicLed::Off,
    });

    // Enable Native emulation (safe on CI without ViGEmBus kernel driver) with gyro aiming
    let gyro_cfg = GyroSettings {
        enabled: true,
        sensitivity: 1.5,
        deadzone: 40,
        invert_x: false,
        invert_y: true,
    };
    assert!(manager.set_emulation(EmulationConfig {
        mode: EmulationMode::Native,
        gyro: gyro_cfg.clone(),
    }));

    // Register physical microphone mute edge callback
    let callback_count = Arc::new(AtomicUsize::new(0));
    let last_muted_state = Arc::new(AtomicBool::new(false));
    let cb_count = callback_count.clone();
    let cb_muted = last_muted_state.clone();
    manager.set_mic_toggle_handler(Arc::new(move |muted| {
        cb_count.fetch_add(1, Ordering::SeqCst);
        cb_muted.store(muted, Ordering::SeqCst);
    }));

    // 2. Feed raw 64-byte USB input report: Cross + L1 + Dpad Up-Right (1) + Mic Mute pressed + 80% battery
    let usb_raw = build_usb_input_report(
        255,                // left stick full right
        0,                  // left stick full up
        128,                // right stick center X
        128,                // right stick center Y
        180,                // L2 analog
        240,                // R2 analog
        0x20 | 0x01,        // Cross (0x20) + Dpad Up-Right (1)
        0x01 | 0x20,        // L1 (0x01) + Options (0x20)
        0x01 | 0x02 | 0x04, // PS + Touchpad click + Mic Mute pressed
        (100, -200, 300),   // gyro (x, y, z)
        (1000, -500, 8192), // accel
        true,               // touch active
        42,                 // touch id
        960,                // touch X
        540,                // touch Y
        0x08,               // battery: status=0 (discharging), raw_level=8 -> 80%
    );

    assert_eq!(
        detect_connection_from_input_report(&usb_raw),
        Some(ConnectionType::Usb)
    );
    let parsed_usb =
        parse_input_report(&usb_raw, ConnectionType::Usb).expect("parse 64-byte USB report");

    assert!(parsed_usb.cross);
    assert!(parsed_usb.dpad_up && parsed_usb.dpad_right);
    assert!(parsed_usb.l1 && parsed_usb.options);
    assert!(parsed_usb.ps_button && parsed_usb.touchpad_click && parsed_usb.mic_mute);
    assert_eq!(parsed_usb.battery.percentage, 80);
    assert!(!parsed_usb.battery.is_charging);
    assert!(parsed_usb.touch1.active);
    assert_eq!(parsed_usb.touch1.id, 42);
    assert_eq!(parsed_usb.touch1.x, 960);
    assert_eq!(parsed_usb.touch1.y, 540);
    assert_eq!(parsed_usb.motion.gyro_z, 300);

    let mic_event = manager.update_input_state(parsed_usb.clone());
    assert_eq!(mic_event, Some(true));
    assert_eq!(callback_count.load(Ordering::SeqCst), 1);
    assert!(last_muted_state.load(Ordering::SeqCst));

    // Verify BatteryLevel lightbar resolved to GREEN (80% >= 60%), Player3 bitmask, and MicLed::Solid
    let out_usb = manager.build_current_output_report().build_usb();
    assert_eq!(out_usb[9], MicLed::Solid.to_byte());
    assert_eq!(out_usb[44], PlayerLed::Player3.to_bitmask());
    assert_eq!(
        out_usb[45..48],
        [RgbColor::GREEN.r, RgbColor::GREEN.g, RgbColor::GREEN.b]
    );

    // Verify end-to-end virtual controller report translations from parsed input
    let xusb = map_input_to_xusb(&parsed_usb, &gyro_cfg);
    assert_eq!(xusb.w_buttons & 0x1000, 0x1000); // A (Cross)
    assert_eq!(xusb.w_buttons & 0x0100, 0x0100); // LB (L1)
    assert_eq!(xusb.w_buttons & 0x0010, 0x0010); // Start (Options)
    assert_eq!(xusb.w_buttons & 0x0001, 0x0001); // Dpad Up
    assert_eq!(xusb.w_buttons & 0x0008, 0x0008); // Dpad Right
    assert_eq!(xusb.b_left_trigger, 180);
    assert_eq!(xusb.b_right_trigger, 240);
    assert_eq!(xusb.s_thumb_lx, 32512); // (255 - 128) * 256
    assert_eq!(xusb.s_thumb_ly, 32767); // (-(0 - 128) * 256).clamp(-32768, 32767)
    assert_eq!(xusb.s_thumb_rx, 450); // center 0 + gyro_z(300) * 1.5
    assert_eq!(xusb.s_thumb_ry, 300); // center 0 + inverted(gyro_y(-200) * 1.5)

    let ds4 = map_input_to_ds4(&parsed_usb);
    let ds4_buttons = ds4.w_buttons;
    assert_eq!(ds4_buttons & 0x000F, 1); // Up-Right HAT
    assert_eq!(ds4_buttons & 0x0020, 0x0020); // Cross
    assert_eq!(ds4_buttons & 0x0100, 0x0100); // L1
    assert_eq!(ds4_buttons & 0x2000, 0x2000); // Options
    assert_eq!(ds4.b_special, 0x03); // PS + Touchpad click
    assert_eq!(ds4.b_trigger_l, 180);
    assert_eq!(ds4.b_trigger_r, 240);

    // 3. Switch to Bluetooth 0x31 extended report with low battery (10%) and released mic button
    let bt_raw = build_bt_extended_input_report(
        128, 128, 128, 128, 0, 0, 0x08, // neutral dpad, no face buttons
        0x00, 0x00, // mic released
        0x01, // battery: 10% discharging -> RED
    );
    assert_eq!(
        detect_connection_from_input_report(&bt_raw),
        Some(ConnectionType::Bluetooth)
    );
    let parsed_bt =
        parse_input_report(&bt_raw, ConnectionType::Bluetooth).expect("parse BT 0x31 report");
    assert_eq!(parsed_bt.connection, ConnectionType::Bluetooth);
    assert_eq!(parsed_bt.battery.percentage, 10);

    // Release mic button (no callback fired)
    assert_eq!(manager.update_input_state(parsed_bt), None);
    assert_eq!(callback_count.load(Ordering::SeqCst), 1);

    // Lightbar should now be RED due to 10% battery
    let out_bt = manager
        .build_current_output_report()
        .with_seq(1)
        .build_bluetooth();
    assert_eq!(
        out_bt[47..50],
        [RgbColor::RED.r, RgbColor::RED.g, RgbColor::RED.b]
    );
    verify_bt_crc32(&out_bt);

    // 4. Press mic mute again over BT -> unmutes and invokes callback a second time
    let bt_unmute = build_bt_extended_input_report(
        128, 128, 128, 128, 0, 0, 0x08, 0x00, 0x04, // mic pressed again
        0x04, // battery 40% -> AMBER
    );
    let parsed_bt_unmute = parse_input_report(&bt_unmute, ConnectionType::Bluetooth).unwrap();
    assert_eq!(manager.update_input_state(parsed_bt_unmute), Some(false));
    assert_eq!(callback_count.load(Ordering::SeqCst), 2);
    assert!(!last_muted_state.load(Ordering::SeqCst));

    let out_bt2 = manager
        .build_current_output_report()
        .with_seq(2)
        .build_bluetooth();
    assert_eq!(out_bt2[11], MicLed::Off.to_byte());
    assert_eq!(
        out_bt2[47..50],
        [RgbColor::AMBER.r, RgbColor::AMBER.g, RgbColor::AMBER.b]
    );
    verify_bt_crc32(&out_bt2);

    // 5. Disable extension -> status must report disconnected and cleared telemetry
    manager.set_extension_enabled(false);
    let final_status = manager.get_status().await;
    assert!(!final_status.extension_enabled);
    assert!(!final_status.connected);
    assert_eq!(final_status.name, "Disconnected");
    assert_eq!(final_status.battery.percentage, 0);
    assert!(!final_status.audio_device_detected);
}

/// E2E: Applying every built-in profile updates status, JSON API contract, and HID output reports
#[tokio::test]
async fn e2e_profile_switching_and_status_serialization_contract() {
    let manager = DualSenseManager::with_udp_port(allocate_free_udp_port());
    manager.set_extension_enabled(true);
    manager.set_connected(true, "Sony DualSense Edge Wireless Controller");

    let profiles = TriggerProfile::default_profiles();
    assert_eq!(profiles.len(), 4);

    for profile in &profiles {
        assert!(
            manager.apply_profile(&profile.id),
            "Failed to apply built-in profile {}",
            profile.id
        );

        let status = manager.get_status().await;
        assert_eq!(status.triggers, profile.triggers);
        assert_eq!(
            status.lighting.lightbar,
            LightbarMode::Static {
                color: profile.lightbar
            }
        );

        // Verify JSON serialization round-trip for UI/API contract
        let serialized = serde_json::to_string(&status).expect("serialize DualSenseStatus");
        let deserialized: DualSenseStatus =
            serde_json::from_str(&serialized).expect("deserialize DualSenseStatus");
        assert_eq!(deserialized.triggers, profile.triggers);
        assert_eq!(deserialized.name, "Sony DualSense Edge Wireless Controller");
        assert!(deserialized.connected);

        // Verify HID output report carries exact compiled trigger payloads & lightbar RGB
        let report_builder = manager.build_current_output_report();
        let usb = report_builder.build_usb();
        assert_eq!(&usb[11..22], &profile.triggers.right.compile_payload());
        assert_eq!(&usb[22..33], &profile.triggers.left.compile_payload());
        assert_eq!(
            usb[45..48],
            [profile.lightbar.r, profile.lightbar.g, profile.lightbar.b]
        );

        let bt = report_builder.build_bluetooth();
        assert_eq!(&bt[13..24], &profile.triggers.right.compile_payload());
        assert_eq!(&bt[24..35], &profile.triggers.left.compile_payload());
        assert_eq!(
            bt[47..50],
            [profile.lightbar.r, profile.lightbar.g, profile.lightbar.b]
        );
        verify_bt_crc32(&bt);
    }

    // Applying a nonexistent profile must fail and preserve the last valid profile
    let before = manager.get_triggers();
    assert!(!manager.apply_profile("non_existent_profile"));
    assert_eq!(manager.get_triggers(), before);
}

/// E2E: Audio & microphone controls, speaker volume clamping, and platform command construction
#[tokio::test]
async fn e2e_audio_and_microphone_workflow() {
    let manager = DualSenseManager::with_udp_port(allocate_free_udp_port());
    manager.set_extension_enabled(true);
    manager.set_connected(true, "Sony DualSense Wireless Controller");

    // Default volume is 80; setting > 100 clamps to 100
    assert_eq!(manager.get_status().await.speaker_volume, 80);
    manager.set_speaker_volume(65);
    assert_eq!(manager.get_status().await.speaker_volume, 65);
    manager.set_speaker_volume(200);
    assert_eq!(manager.get_status().await.speaker_volume, 100);

    // Explicit mute/unmute synchronizes MicLed in HID output reports
    manager.set_microphone_mute(true);
    assert!(manager.get_status().await.mic_muted);
    assert_eq!(
        manager.build_current_output_report().build_usb()[9],
        MicLed::Solid.to_byte()
    );

    manager.set_microphone_mute(false);
    assert!(!manager.get_status().await.mic_muted);
    assert_eq!(
        manager.build_current_output_report().build_usb()[9],
        MicLed::Off.to_byte()
    );

    // Platform audio command generation integrity
    let (toggle_prog, toggle_args) = get_toggle_mic_command();
    let (mute_prog, mute_args) = get_set_mic_mute_command(true);
    let (unmute_prog, unmute_args) = get_set_mic_mute_command(false);
    let (route_prog, route_args) = get_switch_playback_device_command("DualSense Wireless");

    if cfg!(target_os = "windows") {
        assert_eq!(toggle_prog, "powershell");
        assert!(!toggle_args.is_empty());
        assert_eq!(mute_prog, "powershell");
        assert_eq!(unmute_prog, "powershell");
        assert_eq!(route_prog, "powershell");
        assert!(mute_args.iter().any(|a| a.contains("RecordingMute 1")));
        assert!(unmute_args.iter().any(|a| a.contains("RecordingMute 0")));
    } else {
        assert_eq!(toggle_prog, "pactl");
        assert_eq!(mute_prog, "pactl");
        assert_eq!(unmute_prog, "pactl");
        assert_eq!(route_prog, "pactl");
        assert_eq!(
            toggle_args,
            vec!["set-source-mute", "@DEFAULT_SOURCE@", "toggle"]
        );
        assert_eq!(mute_args, vec!["set-source-mute", "@DEFAULT_SOURCE@", "1"]);
        assert_eq!(
            unmute_args,
            vec!["set-source-mute", "@DEFAULT_SOURCE@", "0"]
        );
        assert_eq!(route_args, vec!["set-default-sink", "DualSense Wireless"]);
    }
    assert!(route_args.iter().any(|a| a.contains("DualSense Wireless")));

    // Direct OutputReportBuilder custom configuration smoke check
    let custom = OutputReportBuilder::new()
        .with_triggers(DualTriggerConfig::new(
            TriggerEffect::Hard,
            TriggerEffect::GameCube,
        ))
        .with_lightbar(RgbColor::PURPLE)
        .with_player_led(PlayerLed::All)
        .with_mic_led(MicLed::Pulse)
        .with_rumble(120, 45)
        .with_seq(15);

    let bt = custom.build_bluetooth();
    assert_eq!(bt[1], (15 << 4) | 0x02);
    assert_eq!(bt[11], MicLed::Pulse.to_byte());
    assert_eq!(bt[46], PlayerLed::All.to_bitmask());
    verify_bt_crc32(&bt);
}

/// E2E: Virtual DualSense bidirectional bridge (Physical Input -> Mapping -> Virtual DualSense ->
/// Game Output Report 0x02 -> DualSenseManager -> Physical USB 0x02 & Bluetooth 0x31 with CRC32)
#[tokio::test]
async fn e2e_virtual_dualsense_bidirectional_bridge_with_mapping() {
    use dualsense_core::mapping::{ButtonAction, ButtonId, InputMappingConfig};
    use dualsense_core::virtual_device::{
        decode_dualsense_usb_output_report, MockVirtualDualSenseBackend,
    };

    let manager = DualSenseManager::with_udp_port(allocate_free_udp_port());
    manager.set_extension_enabled(true);
    manager.set_connected(true, "Sony DualSense Edge Wireless Controller");
    manager.set_rumble_gain(1.5);

    // Remap Edge PaddleLeft -> Cross so virtual controller sees Cross
    let mut map_cfg = InputMappingConfig::default();
    map_cfg.button_remaps.insert(
        ButtonId::PaddleLeft,
        ButtonAction::Button {
            target: ButtonId::Cross,
        },
    );
    manager.set_mapping(map_cfg);

    let mut mock = MockVirtualDualSenseBackend::new();
    let game_output = OutputReportBuilder::new()
        .with_triggers(DualTriggerConfig {
            left: TriggerEffect::Bow {
                start_position: 1,
                end_position: 8,
                force: 4,
                snap_force: 7,
            },
            right: TriggerEffect::Weapon {
                start_position: 2,
                end_position: 6,
                force: 8,
            },
        })
        .with_lightbar(RgbColor::CYAN)
        .with_player_led(PlayerLed::Player2)
        .with_mic_led(MicLed::Solid)
        .with_rumble(100, 60)
        .build_usb();

    assert!(decode_dualsense_usb_output_report(&game_output).is_some());
    assert!(mock.inject_game_output_report(&game_output));

    manager.set_virtual_dualsense_backend(Box::new(mock));
    assert!(manager.set_emulation(EmulationConfig {
        mode: EmulationMode::DualSense,
        gyro: GyroSettings::default(),
    }));

    // Submit physical input with Edge PaddleLeft pressed
    let raw_input = dualsense_core::DualSenseInputState {
        paddle_left: true,
        left_stick_x: 128,
        left_stick_y: 128,
        ..Default::default()
    };
    manager.update_input_state(raw_input);

    // Poll game feedback from virtual DualSense back into manager
    let applied = manager.poll_virtual_feedback();
    assert_eq!(applied, 1);

    let out_usb = manager.build_current_output_report().build_usb();
    assert_eq!(out_usb[4], 150); // heavy rumble 100 * 1.5 gain
    assert_eq!(out_usb[3], 90); // light rumble 60 * 1.5 gain
    assert_eq!(out_usb[11], 0x25); // R2 Weapon from game
    assert_eq!(out_usb[22], 0x22); // L2 Bow from game
    assert_eq!(out_usb[44], PlayerLed::Player2.to_bitmask());
    assert_eq!(out_usb[9], MicLed::Solid.to_byte());
    assert_eq!(
        out_usb[45..48],
        [RgbColor::CYAN.r, RgbColor::CYAN.g, RgbColor::CYAN.b]
    );

    let out_bt = manager
        .build_current_output_report()
        .with_seq(7)
        .build_bluetooth();
    verify_bt_crc32(&out_bt);
}

/// E2E: Bluetooth HD Haptics (Report 0x32) + Audio-to-Haptics 48 kHz -> 3 kHz DSP pipeline
#[tokio::test]
async fn e2e_bluetooth_hd_haptics_0x32_and_audio_dsp_pipeline() {
    use dualsense_core::haptics_audio::{AudioToHapticsDsp, HapticsConfig, HapticsMode};
    use dualsense_core::protocol::{build_bluetooth_haptics_0x32, BT_HAPTICS_REPORT_SIZE};

    let manager = DualSenseManager::with_udp_port(allocate_free_udp_port());
    let haptics_cfg = HapticsConfig {
        mode: HapticsMode::AudioToHaptics,
        enabled: true,
        gain: 1.4,
        lowpass_cutoff_hz: 180.0,
        left_balance: 1.0,
        right_balance: 0.8,
        audio_source: "default_loopback".to_string(),
    };
    manager.set_haptics(haptics_cfg.clone());
    assert!(manager.is_bt_hd_haptics_active());

    // Process 1024 stereo 48 kHz frames -> 2 blocks of 32 stereo 3 kHz S8 frames
    let mut dsp = AudioToHapticsDsp::new();
    let mut pcm_48k = Vec::with_capacity(1024 * 2);
    for i in 0..1024 {
        let t = i as f32 / 48000.0;
        let wave = (2.0 * std::f32::consts::PI * 90.0 * t).sin();
        pcm_48k.push(wave);
        pcm_48k.push(wave * 0.7);
    }

    let blocks = dsp.process_stereo_48k(&pcm_48k, &haptics_cfg);
    assert_eq!(blocks.len(), 2);
    for b in blocks {
        manager.push_haptic_block(b, dsp.levels());
    }

    let popped = manager
        .pop_next_haptic_block()
        .expect("haptic block queued");
    let pkt = build_bluetooth_haptics_0x32(4, 11, &popped);
    assert_eq!(pkt.len(), BT_HAPTICS_REPORT_SIZE);
    assert_eq!(pkt[0], 0x32);
    assert_eq!(pkt[1], 4 << 4);
    assert_eq!(&pkt[2..5], &[0x91, 0x07, 0xFE]);
    assert_eq!(pkt[5], 16);
    assert_eq!(pkt[10], 11);
    assert_eq!(&pkt[11..13], &[0x92, 0x40]);

    // Verify CRC32 over [0xA2] || pkt[0..138]
    let crc = LittleEndian::read_u32(&pkt[138..142]);
    let mut hasher = Hasher::new();
    hasher.update(&[0xA2]);
    hasher.update(&pkt[0..138]);
    assert_eq!(hasher.finalize(), crc);

    // Control report 0x31 in HD Haptics mode must clear legacy rumble flags & preserve RELEASE_LEDS=0
    let bt_ctrl = manager
        .build_current_output_report()
        .with_seq(4)
        .build_bluetooth();
    assert_eq!(bt_ctrl[3] & 0x03, 0);
    assert_eq!(bt_ctrl[4] & 0x08, 0);
    assert_eq!(bt_ctrl[4] & 0x62, 0x62);
    verify_bt_crc32(&bt_ctrl);
}

/// E2E: Live Cemuhook Motion UDP Server (DSU protocol) streaming IMU motion from manager
#[tokio::test]
async fn e2e_cemuhook_motion_udp_live_streaming() {
    use dualsense_core::cemuhook::{DSU_PROTOCOL_VERSION, MSG_TYPE_DATA, MSG_TYPE_PORTS};

    let udp_port = allocate_free_udp_port();
    let cemu_port = allocate_free_udp_port();
    let manager = DualSenseManager::with_ports(udp_port, cemu_port);
    manager.set_extension_enabled(true);
    manager.set_connected(true, "Sony DualSense Wireless Controller");

    let mut state = dualsense_core::DualSenseInputState::default();
    state.battery.percentage = 90;
    state.motion.accel_z = 8192;
    state.motion.gyro_z = 4096;
    manager.update_input_state(state);

    manager
        .start_cemuhook()
        .await
        .expect("start cemuhook server");

    let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
    let target = SocketAddr::from(([127, 0, 0, 1], cemu_port));

    // Build DSUC MSG_TYPE_PORTS request for slot 0
    let mut req_ports = vec![0u8; 25];
    req_ports[0..4].copy_from_slice(b"DSUC");
    LittleEndian::write_u16(&mut req_ports[4..6], DSU_PROTOCOL_VERSION);
    LittleEndian::write_u16(&mut req_ports[6..8], 9);
    LittleEndian::write_u32(&mut req_ports[12..16], 99);
    LittleEndian::write_u32(&mut req_ports[16..20], MSG_TYPE_PORTS);
    LittleEndian::write_i32(&mut req_ports[20..24], 1);
    req_ports[24] = 0;
    let mut h = Hasher::new();
    h.update(&req_ports);
    let crc = h.finalize();
    LittleEndian::write_u32(&mut req_ports[8..12], crc);

    client.send_to(&req_ports, target).await.unwrap();
    let mut resp_ports = [0u8; 128];
    let (n, _) = tokio::time::timeout(Duration::from_secs(2), client.recv_from(&mut resp_ports))
        .await
        .expect("timeout on DSU ports")
        .unwrap();
    assert_eq!(n, 32);
    assert_eq!(&resp_ports[0..4], b"DSUS");
    assert_eq!(resp_ports[21], 2); // Connected

    // Build DSUC MSG_TYPE_DATA subscription
    let mut req_data = vec![0u8; 28];
    req_data[0..4].copy_from_slice(b"DSUC");
    LittleEndian::write_u16(&mut req_data[4..6], DSU_PROTOCOL_VERSION);
    LittleEndian::write_u16(&mut req_data[6..8], 12);
    LittleEndian::write_u32(&mut req_data[12..16], 99);
    LittleEndian::write_u32(&mut req_data[16..20], MSG_TYPE_DATA);
    let mut h2 = Hasher::new();
    h2.update(&req_data);
    let crc2 = h2.finalize();
    LittleEndian::write_u32(&mut req_data[8..12], crc2);

    client.send_to(&req_data, target).await.unwrap();
    let mut resp_data = [0u8; 128];
    let (n2, _) = tokio::time::timeout(Duration::from_secs(2), client.recv_from(&mut resp_data))
        .await
        .expect("timeout on DSU data")
        .unwrap();
    assert_eq!(n2, 100);
    assert_eq!(LittleEndian::read_u32(&resp_data[16..20]), MSG_TYPE_DATA);
    assert!((LittleEndian::read_f32(&resp_data[84..88]) - 1.0).abs() < 1e-3); // 1.0 G
    assert!((LittleEndian::read_f32(&resp_data[96..100]) - 250.0).abs() < 1e-1); // 250 deg/s

    manager.stop_cemuhook().await;
    assert!(!manager.get_status().await.cemuhook_stats.running);
}

/// E2E: Standalone disk persistence roundtrip (AppConfig + custom profiles + game rules)
#[tokio::test]
async fn e2e_standalone_config_persistence_and_restore() {
    use dualsense_core::storage::{AppConfig, GameProfileRule};

    let tmp = tempfile::tempdir().expect("tempdir");
    let cfg_path = tmp.path().join("config.json");

    let mgr_a = DualSenseManager::with_udp_port(allocate_free_udp_port());
    mgr_a.set_extension_enabled(true);
    mgr_a.set_rumble_gain(1.35);
    mgr_a.add_custom_profile(TriggerProfile {
        id: "custom_returnal".to_string(),
        name: "Returnal Alt-Fire".to_string(),
        description: "Half-pull aim + full-pull alt-fire".to_string(),
        triggers: DualTriggerConfig::new(
            TriggerEffect::Weapon {
                start_position: 2,
                end_position: 5,
                force: 7,
            },
            TriggerEffect::MachineGun {
                start_position: 1,
                end_position: 8,
                strength: 6,
                frequency: 20,
            },
        ),
        lightbar: RgbColor::PURPLE,
    });
    mgr_a.set_game_profile_rules(vec![GameProfileRule {
        process_name: "Returnal-Win64-Shipping.exe".to_string(),
        profile_id: "custom_returnal".to_string(),
        enabled: true,
    }]);

    let exported = mgr_a.export_config().await;
    exported.save_to_path(&cfg_path).expect("save config.json");

    let loaded = AppConfig::load_from_path(&cfg_path);
    let mgr_b = DualSenseManager::with_udp_port(allocate_free_udp_port());
    mgr_b.apply_config(&loaded);

    assert!((mgr_b.get_rumble_gain() - 1.35).abs() < 1e-5);
    assert!(mgr_b.apply_profile("custom_returnal"));
    assert_eq!(mgr_b.get_game_profile_rules().len(), 1);
    assert_eq!(
        mgr_b.get_game_profile_rules()[0].process_name,
        "Returnal-Win64-Shipping.exe"
    );
}
