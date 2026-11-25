use std::mem::size_of;
use std::net::SocketAddr;
use std::sync::Arc;

use byteorder::{ByteOrder, LittleEndian};
use crc32fast::Hasher;
use dualsense_core::device::{
    detect_connection_from_input_report, detect_connection_type, identify_controller,
    is_supported_sony_controller, DUALSENSE_EDGE_PID, DUALSENSE_PID, DUALSHOCK4_V1_PID,
    DUALSHOCK4_V2_PID, SONY_VENDOR_ID,
};
use dualsense_core::emulation::{EmulationConfig, EmulationMode, GyroSettings};
use dualsense_core::lightbar::{LightbarMode, LightingConfig, MicLed, PlayerLed, RgbColor};
use dualsense_core::manager::DualSenseManager;
use dualsense_core::protocol::{
    parse_input_report, ConnectionType, DualSenseInputState, OutputReportBuilder,
};
use dualsense_core::triggers::TriggerEffect;
use dualsense_core::udp_server::{DualSenseUdpServer, SharedControllerState};
use dualsense_core::vigem::{
    map_input_to_ds4, map_input_to_xusb, Ds4Report, Ds4SubmitReport, ViGEmPluginTarget,
    ViGEmUnplugTarget, XusbReport, XusbSubmitReport, IOCTL_DS4_SUBMIT_REPORT,
    IOCTL_VIGEM_PLUGIN_TARGET, IOCTL_VIGEM_UNPLUG_TARGET, IOCTL_XUSB_SUBMIT_REPORT,
};

fn allocate_free_udp_port() -> u16 {
    let socket =
        std::net::UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], 0))).expect("bind ephemeral UDP");
    socket.local_addr().expect("read local addr").port()
}

/// REG-01 & REG-02: Firmware LED lock-out guard (`RELEASE_LEDS` 0x08 must never be set),
/// continuous `LIGHT_OUT` bug guard (`valid_flag2` and `lightbar_setup` must be 0x00 on normal
/// reports so LEDs stay lit), and 48-byte Windows-compatible USB output report length.
#[test]
fn regression_output_reports_never_set_release_leds_and_preserve_setup_flags() {
    let player_leds = [
        PlayerLed::Off,
        PlayerLed::Player1,
        PlayerLed::Player2,
        PlayerLed::Player3,
        PlayerLed::Player4,
        PlayerLed::All,
    ];
    let mic_leds = [MicLed::Off, MicLed::Solid, MicLed::Pulse];

    for player_led in player_leds {
        for mic_led in mic_leds {
            let builder = OutputReportBuilder::new()
                .with_lightbar(RgbColor::new(12, 34, 56))
                .with_player_led(player_led)
                .with_mic_led(mic_led)
                .with_rumble(255, 128);

            let usb = builder.build_usb();
            assert_eq!(usb.len(), 48, "USB output report must be exactly 48 bytes");
            assert_eq!(usb[0], 0x02);
            assert_eq!(
                usb[1], 0x0F,
                "USB valid_flag0 must only enable rumble + triggers"
            );
            assert_eq!(usb[2], 0x15);
            assert_eq!(
                usb[2] & 0x08,
                0,
                "USB valid_flag1 must never set RELEASE_LEDS (0x08)"
            );
            assert_eq!(
                usb[39], 0x00,
                "USB valid_flag2 must stay 0x00 on normal reports so LIGHT_OUT never turns off LEDs"
            );
            assert_eq!(
                usb[42], 0x00,
                "USB lightbar_setup must stay 0x00 on normal reports"
            );
            assert_eq!(usb[43], 0x00, "USB led_brightness high");
            assert_eq!(usb[44], player_led.to_bitmask());
            assert_eq!(usb[9], mic_led.to_byte());

            let bt = builder.build_bluetooth();
            assert_eq!(bt[0], 0x31);
            assert_eq!(bt[2], 0x10);
            assert_eq!(
                bt[3], 0x0F,
                "BT valid_flag0 must only enable rumble + triggers"
            );
            assert_eq!(bt[4], 0x15);
            assert_eq!(
                bt[4] & 0x08,
                0,
                "BT valid_flag1 must never set RELEASE_LEDS (0x08)"
            );
            assert_eq!(
                bt[41], 0x00,
                "BT valid_flag2 must stay 0x00 on normal reports so LIGHT_OUT never turns off LEDs"
            );
            assert_eq!(
                bt[44], 0x00,
                "BT lightbar_setup must stay 0x00 on normal reports"
            );
            assert_eq!(bt[45], 0x00, "BT led_brightness high");
            assert_eq!(bt[46], player_led.to_bitmask());
            assert_eq!(bt[11], mic_led.to_byte());
        }
    }

    // One-shot LED reset handshake reports must set LIGHT_OUT (0x02) cleanly
    let usb_reset = OutputReportBuilder::build_usb_reset_leds();
    assert_eq!(usb_reset.len(), 48);
    assert_eq!(usb_reset[39], 0x02);
    assert_eq!(usb_reset[42], 0x02);

    let bt_reset = OutputReportBuilder::build_bluetooth_reset_leds(4);
    assert_eq!(bt_reset[0], 0x31);
    assert_eq!(bt_reset[41], 0x02);
    assert_eq!(bt_reset[44], 0x02);
}

/// REG-03: Bluetooth 0x31 sequence masking (0..15) and Sony HID CRC32 (0xA2 seed) across all u8 seq values.
#[test]
fn regression_bluetooth_sequence_masking_and_crc32_all_values() {
    for seq in 0u8..=255 {
        let report = OutputReportBuilder::new()
            .with_seq(seq)
            .with_lightbar(RgbColor::new(seq, 255 - seq, seq.wrapping_mul(3)))
            .build_bluetooth();

        assert_eq!(report[0], 0x31);
        assert_eq!(report[1], ((seq & 0x0F) << 4) | 0x02);
        assert_eq!(report[2], 0x10);

        let embedded_crc = LittleEndian::read_u32(&report[74..78]);
        let mut hasher = Hasher::new();
        hasher.update(&[0xA2]);
        hasher.update(&report[0..74]);
        assert_eq!(
            hasher.finalize(),
            embedded_crc,
            "CRC32 failed for seq={}",
            seq
        );
    }
}

/// REG-04: Adaptive trigger clamping bounds, Sony/Nielk1 bit-packed opcodes, and serde aliases.
#[test]
fn regression_trigger_effect_clamping_and_serde_aliases() {
    // Out-of-range inputs must clamp to firmware-safe maxima and bit-pack properly
    let fb = TriggerEffect::Feedback {
        start_position: 255,
        force: 255,
    }
    .compile_payload();
    assert_eq!(fb[0], 0x21);
    assert_eq!(LittleEndian::read_u16(&fb[1..3]), 1 << 9); // zone 9 active
    assert_eq!((LittleEndian::read_u32(&fb[3..7]) >> 27) & 0x07, 7); // max force 8 -> 7

    let wp = TriggerEffect::Weapon {
        start_position: 99,
        end_position: 88,
        force: 77,
    }
    .compile_payload();
    assert_eq!(wp[0], 0x25);
    assert_eq!(LittleEndian::read_u16(&wp[1..3]), (1 << 8) | (1 << 9));
    assert_eq!(wp[3], 7);

    let bow = TriggerEffect::Bow {
        start_position: 20,
        end_position: 30,
        force: 40,
        snap_force: 50,
    }
    .compile_payload();
    assert_eq!(bow[0], 0x22);
    assert_eq!(LittleEndian::read_u16(&bow[1..3]), (1 << 8) | (1 << 9));
    assert_eq!(bow[3], (7 << 3) | 7);

    let gal = TriggerEffect::Galloping {
        start_position: 15,
        end_position: 16,
        first_foot: 17,
        second_foot: 18,
        frequency: 250,
    }
    .compile_payload();
    assert_eq!(gal[0], 0x23);
    assert_eq!(LittleEndian::read_u16(&gal[1..3]), (1 << 8) | (1 << 9));
    assert_eq!(gal[3], (7 << 3) | 6);
    assert_eq!(gal[4], 250);

    let mg = TriggerEffect::MachineGun {
        start_position: 12,
        end_position: 13,
        strength: 14,
        frequency: 120,
    }
    .compile_payload();
    assert_eq!(mg[0], 0x26);
    assert_eq!(LittleEndian::read_u16(&mg[1..3]), 1 << 9);
    assert_eq!((LittleEndian::read_u32(&mg[3..7]) >> 27) & 0x07, 7);
    assert_eq!(mg[9], 120);

    let mach = TriggerEffect::Machine {
        start_position: 12,
        end_position: 13,
        strength_a: 25,
        strength_b: 26,
        frequency: 40,
        period: 5,
    }
    .compile_payload();
    assert_eq!(mach[0], 0x27);
    assert_eq!(LittleEndian::read_u16(&mach[1..3]), (1 << 8) | (1 << 9));
    assert_eq!(mach[3], (7 << 3) | 7);
    assert_eq!(mach[4], 40);
    assert_eq!(mach[5], 5);

    let vib = TriggerEffect::Vibration {
        position: 50,
        amplitude: 255,
        frequency: 180,
    }
    .compile_payload();
    assert_eq!(vib[0], 0x26);
    assert_eq!(LittleEndian::read_u16(&vib[1..3]), 1 << 9);
    assert_eq!((LittleEndian::read_u32(&vib[3..7]) >> 27) & 0x07, 7);
    assert_eq!(vib[9], 180);

    // Serde legacy / DualSenseX aliases must continue to deserialize cleanly
    let alias_cases: [(&str, TriggerEffect); 6] = [
        (r#"{"mode":"normal"}"#, TriggerEffect::Off),
        (
            r#"{"mode":"resistance","start_position":2,"force":6}"#,
            TriggerEffect::Feedback {
                start_position: 2,
                force: 6,
            },
        ),
        (
            r#"{"mode":"semi_automatic_gun","start_position":1,"end_position":5,"force":7}"#,
            TriggerEffect::Weapon {
                start_position: 1,
                end_position: 5,
                force: 7,
            },
        ),
        (
            r#"{"mode":"automatic_gun","start_position":1,"end_position":8,"strength":6,"frequency":15}"#,
            TriggerEffect::MachineGun {
                start_position: 1,
                end_position: 8,
                strength: 6,
                frequency: 15,
            },
        ),
        (
            r#"{"mode":"vibrate_trigger","position":3,"amplitude":10,"frequency":20}"#,
            TriggerEffect::Vibration {
                position: 3,
                amplitude: 10,
                frequency: 20,
            },
        ),
        (
            r#"{"mode":"custom_trigger_value","zones":[1,2,3,4,5,6,7,8,9,10]}"#,
            TriggerEffect::CustomZone {
                zones: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            },
        ),
    ];

    for (json, expected) in alias_cases {
        let decoded: TriggerEffect = serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("Failed to decode {}: {}", json, e));
        assert_eq!(decoded, expected);
        assert!(!decoded.name().is_empty());
    }
}

/// REG-05: Input report parser edge cases (truncated buffers, all 8 D-pad angles, BT compact fallbacks,
/// negative motion values, 12-bit touchpad coordinates, and battery nibble saturation).
#[test]
fn regression_input_report_parser_edge_cases() {
    // Empty and truncated buffers must return None without panicking
    assert!(parse_input_report(&[], ConnectionType::Usb).is_none());
    assert!(parse_input_report(&[], ConnectionType::Bluetooth).is_none());
    for len in 1..10 {
        let buf = vec![0x01u8; len];
        assert!(
            parse_input_report(&buf, ConnectionType::Usb).is_none(),
            "USB buffer of len {} should return None",
            len
        );
    }
    for len in 1..7 {
        let buf = vec![0x01u8; len];
        assert!(
            parse_input_report(&buf, ConnectionType::Bluetooth).is_none(),
            "BT compact buffer of len {} should return None",
            len
        );
    }

    // BT compact 7-byte report (no analog trigger bytes): falls back to 255/0 from click flags
    let bt_short_clicked = [0x01, 128, 128, 128, 128, 0x08, 0x04]; // L2 click set, R2 click clear
    let state_short = parse_input_report(&bt_short_clicked, ConnectionType::Bluetooth).unwrap();
    assert!(state_short.l2_click);
    assert!(!state_short.r2_click);
    assert_eq!(state_short.l2_trigger, 255);
    assert_eq!(state_short.r2_trigger, 0);

    // All 8 D-pad HAT values + neutral (8) across USB, BT compact, and BT extended 0x31
    let expected_dpad = [
        (0u8, (true, false, false, false)),  // Up
        (1u8, (true, true, false, false)),   // Up-Right
        (2u8, (false, true, false, false)),  // Right
        (3u8, (false, true, true, false)),   // Down-Right
        (4u8, (false, false, true, false)),  // Down
        (5u8, (false, false, true, true)),   // Down-Left
        (6u8, (false, false, false, true)),  // Left
        (7u8, (true, false, false, true)),   // Up-Left
        (8u8, (false, false, false, false)), // Neutral
    ];

    for (hat, (up, right, down, left)) in expected_dpad {
        let mut usb = [0u8; 64];
        usb[0] = 0x01;
        usb[8] = hat;
        let s_usb = parse_input_report(&usb, ConnectionType::Usb).unwrap();
        assert_eq!(
            (
                s_usb.dpad_up,
                s_usb.dpad_right,
                s_usb.dpad_down,
                s_usb.dpad_left
            ),
            (up, right, down, left),
            "USB dpad mismatch for hat={}",
            hat
        );

        let mut bt_compact = [0u8; 10];
        bt_compact[0] = 0x01;
        bt_compact[5] = hat;
        let s_btc = parse_input_report(&bt_compact, ConnectionType::Bluetooth).unwrap();
        assert_eq!(
            (
                s_btc.dpad_up,
                s_btc.dpad_right,
                s_btc.dpad_down,
                s_btc.dpad_left
            ),
            (up, right, down, left),
            "BT compact dpad mismatch for hat={}",
            hat
        );

        let mut bt_ext = [0u8; 78];
        bt_ext[0] = 0x31;
        bt_ext[9] = hat;
        let s_bte = parse_input_report(&bt_ext, ConnectionType::Bluetooth).unwrap();
        assert_eq!(
            (
                s_bte.dpad_up,
                s_bte.dpad_right,
                s_bte.dpad_down,
                s_bte.dpad_left
            ),
            (up, right, down, left),
            "BT 0x31 dpad mismatch for hat={}",
            hat
        );
    }

    // Telemetry: signed i16 motion bounds, 12-bit max touchpad coords, battery clamping & charging
    let mut usb_telem = [0u8; 64];
    usb_telem[0] = 0x01;
    LittleEndian::write_i16(&mut usb_telem[16..18], i16::MIN);
    LittleEndian::write_i16(&mut usb_telem[18..20], i16::MAX);
    LittleEndian::write_i16(&mut usb_telem[20..22], -12345);
    LittleEndian::write_i16(&mut usb_telem[22..24], 23456);
    LittleEndian::write_i16(&mut usb_telem[24..26], -1);
    LittleEndian::write_i16(&mut usb_telem[26..28], 0);

    // Touchpad inactive (bit 7 = 1), id = 0x7F, x = 0xFFF (4095), y = 0xABC (2748)
    usb_telem[33] = 0x80 | 0x7F;
    usb_telem[34] = 0xFF;
    usb_telem[35] = 0x0F | (0x0C << 4);
    usb_telem[36] = 0xAB;

    // Battery raw_level = 0x0F (15 -> clamped to 10 -> 100%), status = 1 (charging -> is_full = false)
    usb_telem[53] = 0x1F;
    let parsed = parse_input_report(&usb_telem, ConnectionType::Usb).unwrap();
    assert_eq!(parsed.motion.gyro_x, i16::MIN);
    assert_eq!(parsed.motion.gyro_y, i16::MAX);
    assert_eq!(parsed.motion.gyro_z, -12345);
    assert_eq!(parsed.motion.accel_x, 23456);
    assert_eq!(parsed.motion.accel_y, -1);
    assert_eq!(parsed.motion.accel_z, 0);
    assert!(!parsed.touch1.active);
    assert_eq!(parsed.touch1.id, 0x7F);
    assert_eq!(parsed.touch1.x, 0xFFF);
    assert_eq!(parsed.touch1.y, 0xABC);
    assert_eq!(parsed.battery.percentage, 100);
    assert!(parsed.battery.is_charging);
    assert!(!parsed.battery.is_full);

    // Battery raw_level = 10, status = 0 (not charging -> is_full = true)
    usb_telem[53] = 0x0A;
    let parsed_full = parse_input_report(&usb_telem, ConnectionType::Usb).unwrap();
    assert_eq!(parsed_full.battery.percentage, 100);
    assert!(!parsed_full.battery.is_charging);
    assert!(parsed_full.battery.is_full);
}

/// REG-06: Supported Sony PIDs and transport auto-detection across Windows/Linux HID paths and reports.
#[test]
fn regression_device_identification_and_transport_detection() {
    let supported = [
        (DUALSENSE_PID, true),
        (DUALSENSE_EDGE_PID, true),
        (DUALSHOCK4_V1_PID, false),
        (DUALSHOCK4_V2_PID, false),
    ];
    for (pid, expected_is_ds) in supported {
        assert!(is_supported_sony_controller(SONY_VENDOR_ID, pid));
        assert!(!is_supported_sony_controller(0x045E, pid)); // Wrong vendor
        let (name, is_ds) = identify_controller(pid);
        assert_eq!(is_ds, expected_is_ds);
        assert!(!name.is_empty());
    }

    assert!(!is_supported_sony_controller(SONY_VENDOR_ID, 0xFFFF));
    assert_eq!(identify_controller(0xFFFF), ("Sony Gamepad", false));

    // Path-based heuristics
    assert_eq!(
        detect_connection_type(r"\\?\hid#bthledevice#1", 0),
        ConnectionType::Bluetooth
    );
    assert_eq!(
        detect_connection_type("/sys/bus/bluetooth/hidraw2", 0),
        ConnectionType::Bluetooth
    );
    assert_eq!(
        detect_connection_type("/dev/usb/hidraw0", -1),
        ConnectionType::Usb
    );
    assert_eq!(detect_connection_type("", -1), ConnectionType::Usb);

    // Packet-based dynamic transport refinement
    assert_eq!(
        detect_connection_from_input_report(&[0x01; 12]),
        Some(ConnectionType::Bluetooth)
    );
    assert_eq!(
        detect_connection_from_input_report(&[0x01; 60]),
        Some(ConnectionType::Usb)
    );
    assert_eq!(detect_connection_from_input_report(&[0x01; 30]), None);
    assert_eq!(detect_connection_from_input_report(&[0x99; 64]), None);
}

/// REG-07: DualSenseX UDP payload parser covers all 20 trigger modes (0..=19), unknown mode fallback,
/// direct JSON fields, and safely ignores malformed or incomplete packets.
#[test]
fn regression_udp_payload_all_trigger_modes_and_malformed_inputs() {
    let shared = Arc::new(SharedControllerState::default());

    let mode_cases: [(u64, &str, TriggerEffect); 21] = [
        (0, "[0, 1, 0]", TriggerEffect::Off),
        (
            1,
            "[0, 1, 1, 2, 6]",
            TriggerEffect::Feedback {
                start_position: 2,
                force: 6,
            },
        ),
        (
            2,
            "[0, 1, 2, 3, 7]",
            TriggerEffect::Feedback {
                start_position: 3,
                force: 7,
            },
        ),
        (
            3,
            "[0, 1, 3, 1, 8, 3, 7]",
            TriggerEffect::Bow {
                start_position: 1,
                end_position: 8,
                force: 3,
                snap_force: 7,
            },
        ),
        (
            4,
            "[0, 1, 4, 1, 7, 2, 5, 12]",
            TriggerEffect::Galloping {
                start_position: 1,
                end_position: 7,
                first_foot: 2,
                second_foot: 5,
                frequency: 12,
            },
        ),
        (
            5,
            "[0, 1, 5, 2, 6, 8]",
            TriggerEffect::Weapon {
                start_position: 2,
                end_position: 6,
                force: 8,
            },
        ),
        (
            6,
            "[0, 1, 6, 1, 8, 6, 25]",
            TriggerEffect::MachineGun {
                start_position: 1,
                end_position: 8,
                strength: 6,
                frequency: 25,
            },
        ),
        (
            7,
            "[0, 1, 7, 2, 5, 18]",
            TriggerEffect::Vibration {
                position: 2,
                amplitude: 5,
                frequency: 18,
            },
        ),
        (
            8,
            "[0, 1, 8, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10]",
            TriggerEffect::CustomZone {
                zones: [1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            },
        ),
        (
            9,
            "[0, 1, 9, 1, 9, 3, 6, 14, 2]",
            TriggerEffect::Machine {
                start_position: 1,
                end_position: 9,
                strength_a: 3,
                strength_b: 6,
                frequency: 14,
                period: 2,
            },
        ),
        (10, "[0, 1, 10]", TriggerEffect::Choppy),
        (11, "[0, 1, 11]", TriggerEffect::VerySoft),
        (12, "[0, 1, 12]", TriggerEffect::Soft),
        (13, "[0, 1, 13]", TriggerEffect::Medium),
        (14, "[0, 1, 14]", TriggerEffect::Hard),
        (15, "[0, 1, 15]", TriggerEffect::VeryHard),
        (16, "[0, 1, 16]", TriggerEffect::Hardest),
        (17, "[0, 1, 17]", TriggerEffect::Rigid),
        (18, "[0, 1, 18]", TriggerEffect::VibrateTriggerPulse),
        (19, "[0, 1, 19]", TriggerEffect::GameCube),
        (99, "[0, 1, 99]", TriggerEffect::Off),
    ];

    for (mode_id, params_json, expected) in mode_cases {
        let payload = format!(
            r#"{{"instructions":[{{"type":1,"parameters":{}}}]}}"#,
            params_json
        );
        DualSenseUdpServer::process_payload(&payload, &shared);
        assert_eq!(
            shared.triggers.read().left,
            expected,
            "UDP trigger mode_id={} did not map as expected",
            mode_id
        );
    }

    // Direct JSON fields (non-instruction format)
    let direct_payload = r#"{
        "left_trigger": { "mode": "hard" },
        "right_trigger": { "mode": "game_cube" },
        "rgb": { "r": 11, "g": 22, "b": 33 }
    }"#;
    DualSenseUdpServer::process_payload(direct_payload, &shared);
    assert_eq!(shared.triggers.read().left, TriggerEffect::Hard);
    assert_eq!(shared.triggers.read().right, TriggerEffect::GameCube);
    assert_eq!(*shared.lightbar.read(), RgbColor::new(11, 22, 33));

    // Malformed / partial / unknown payloads must never panic or corrupt existing state
    let bad_payloads = [
        "",
        "not json at all",
        "{}",
        r#"{"instructions":[{"type":1,"parameters":[0]}]}"#,
        r#"{"instructions":[{"type":2,"parameters":[0,255]}]}"#,
        r#"{"instructions":[{"type":4,"parameters":[0]}]}"#,
        r#"{"instructions":[{"type":999,"parameters":[1,2,3]}]}"#,
    ];
    for bad in bad_payloads {
        DualSenseUdpServer::process_payload(bad, &shared);
    }
    assert_eq!(shared.triggers.read().left, TriggerEffect::Hard);
    assert_eq!(shared.triggers.read().right, TriggerEffect::GameCube);
    assert_eq!(*shared.lightbar.read(), RgbColor::new(11, 22, 33));
}

/// REG-08: XInput & DS4 button/stick/HAT completeness, gyro i16::MIN overflow safety, and ViGEmBus FFI ABI sizes.
#[test]
fn regression_vigem_mappings_gyro_extreme_values_and_abi_layout() {
    // Kernel IOCTL ABI sizes and control codes
    assert_eq!(size_of::<ViGEmPluginTarget>(), 16);
    assert_eq!(size_of::<ViGEmUnplugTarget>(), 8);
    assert_eq!(size_of::<XusbReport>(), 12);
    assert_eq!(size_of::<XusbSubmitReport>(), 20);
    assert_eq!(size_of::<Ds4Report>(), 9);
    assert_eq!(size_of::<Ds4SubmitReport>(), 20);
    assert_eq!(IOCTL_VIGEM_PLUGIN_TARGET, 0x2AA004);
    assert_eq!(IOCTL_VIGEM_UNPLUG_TARGET, 0x2AA008);
    assert_eq!(IOCTL_XUSB_SUBMIT_REPORT, 0x2AA808);
    assert_eq!(IOCTL_DS4_SUBMIT_REPORT, 0x2AA80C);

    // Gyro extreme i16::MIN / i16::MAX must never panic (even when inverted or scaled)
    let gyro = GyroSettings {
        enabled: true,
        sensitivity: 5.0,
        deadzone: 50,
        invert_x: true,
        invert_y: true,
    };
    let (dx_min, dy_min) = gyro.map_gyro_to_stick_delta(i16::MIN, i16::MIN);
    assert_eq!(dx_min, i16::MAX);
    assert_eq!(dy_min, i16::MAX);
    let (dx_max, dy_max) = gyro.map_gyro_to_stick_delta(i16::MAX, i16::MAX);
    assert_eq!(dx_max, -i16::MAX);
    assert_eq!(dy_max, -i16::MAX);

    // Exact deadzone boundary: |val| == deadzone is active; |val| == deadzone - 1 is filtered
    let gyro_dz = GyroSettings {
        enabled: true,
        sensitivity: 1.0,
        deadzone: 50,
        invert_x: false,
        invert_y: false,
    };
    assert_eq!(gyro_dz.map_gyro_to_stick_delta(49, -49), (0, 0));
    assert_eq!(gyro_dz.map_gyro_to_stick_delta(50, -50), (50, -50));

    // Centered DualSense sticks (128, 128) must produce exact (0, 0) in XInput
    let centered = DualSenseInputState {
        left_stick_x: 128,
        left_stick_y: 128,
        right_stick_x: 128,
        right_stick_y: 128,
        ..Default::default()
    };
    let xusb_center = map_input_to_xusb(&centered, &GyroSettings::default());
    assert_eq!(xusb_center.w_buttons, 0);
    assert_eq!(xusb_center.s_thumb_lx, 0);
    assert_eq!(xusb_center.s_thumb_ly, 0);
    assert_eq!(xusb_center.s_thumb_rx, 0);
    assert_eq!(xusb_center.s_thumb_ry, 0);

    // All buttons active simultaneously -> verify complete XInput & DS4 bitmasks
    let all_buttons = DualSenseInputState {
        left_stick_x: 0,
        left_stick_y: 255,
        right_stick_x: 255,
        right_stick_y: 0,
        l2_trigger: 123,
        r2_trigger: 234,
        cross: true,
        circle: true,
        square: true,
        triangle: true,
        dpad_up: true,
        dpad_down: true,
        dpad_left: true,
        dpad_right: true,
        l1: true,
        r1: true,
        l2_click: true,
        r2_click: true,
        l3: true,
        r3: true,
        create: true,
        options: true,
        ps_button: true,
        touchpad_click: true,
        ..Default::default()
    };

    let xusb_all = map_input_to_xusb(&all_buttons, &GyroSettings::default());
    assert_eq!(xusb_all.w_buttons, 0xF3FF);
    assert_eq!(xusb_all.s_thumb_lx, -32768);
    assert_eq!(xusb_all.s_thumb_ly, -32512);
    assert_eq!(xusb_all.s_thumb_rx, 32512);
    assert_eq!(xusb_all.s_thumb_ry, 32767);

    let ds4_all = map_input_to_ds4(&all_buttons);
    let ds4_w_buttons = ds4_all.w_buttons;
    // Conflicting all-4 D-pad falls back to neutral HAT = 8, plus all 12 upper button bits (0xFFF0)
    assert_eq!(ds4_w_buttons, 0xFFF8);
    assert_eq!(ds4_all.b_special, 0x03);
    assert_eq!(ds4_all.b_trigger_l, 123);
    assert_eq!(ds4_all.b_trigger_r, 234);

    // Every single DS4 D-pad HAT direction (0..=7) + neutral (8)
    let hat_cases = [
        ((true, false, false, false), 0u16),
        ((true, true, false, false), 1u16),
        ((false, true, false, false), 2u16),
        ((false, true, true, false), 3u16),
        ((false, false, true, false), 4u16),
        ((false, false, true, true), 5u16),
        ((false, false, false, true), 6u16),
        ((true, false, false, true), 7u16),
        ((false, false, false, false), 8u16),
    ];
    for ((up, right, down, left), expected_hat) in hat_cases {
        let inp = DualSenseInputState {
            dpad_up: up,
            dpad_right: right,
            dpad_down: down,
            dpad_left: left,
            ..Default::default()
        };
        let ds4 = map_input_to_ds4(&inp);
        let hat = ds4.w_buttons & 0x000F;
        assert_eq!(hat, expected_hat);
    }
}

/// REG-09: Lightbar hex parsing edge cases, exact battery thresholds, rainbow sweep, and breathing bounds.
#[test]
fn regression_lightbar_color_math_and_hex_parsing() {
    assert_eq!(
        RgbColor::from_hex("  #AbCdEf  "),
        Some(RgbColor::new(0xAB, 0xCD, 0xEF))
    );
    assert_eq!(RgbColor::from_hex("000000"), Some(RgbColor::BLACK));
    assert_eq!(RgbColor::from_hex("#ffffff"), Some(RgbColor::WHITE));
    assert_eq!(RgbColor::from_hex("#fff"), None);
    assert_eq!(RgbColor::from_hex("#1234567"), None);
    assert_eq!(RgbColor::from_hex("#zzzzzz"), None);

    assert_eq!(
        LightbarMode::Off.compute_color(10.0, 100, false),
        RgbColor::BLACK
    );

    let bat = LightbarMode::BatteryLevel;
    assert_eq!(bat.compute_color(0.0, 100, false), RgbColor::GREEN);
    assert_eq!(bat.compute_color(0.0, 60, false), RgbColor::GREEN);
    assert_eq!(bat.compute_color(0.0, 59, false), RgbColor::AMBER);
    assert_eq!(bat.compute_color(0.0, 20, false), RgbColor::AMBER);
    assert_eq!(bat.compute_color(0.0, 19, false), RgbColor::RED);
    assert_eq!(bat.compute_color(0.0, 0, false), RgbColor::RED);

    // Charging always outputs pulsing amber (r=255, b=0, g in 0..=140)
    let charging = bat.compute_color(0.5, 95, true);
    assert_eq!(charging.r, 255);
    assert_eq!(charging.b, 0);
    assert!(charging.g <= 140);

    // Breathing trough at t=0.0 is black; peak at t=0.5 (speed=1.0) is full color
    let breathing = LightbarMode::Breathing {
        color: RgbColor::new(100, 150, 200),
        speed: 1.0,
    };
    assert_eq!(breathing.compute_color(0.0, 50, false), RgbColor::BLACK);
    assert_eq!(
        breathing.compute_color(0.5, 50, false),
        RgbColor::new(100, 150, 200)
    );

    // Rainbow full 360-degree primary/secondary vertices + negative elapsed safety
    let rainbow = LightbarMode::Rainbow { speed: 1.0 };
    assert_eq!(rainbow.compute_color(0.0, 50, false), RgbColor::RED);
    assert_eq!(
        rainbow.compute_color(1.0, 50, false),
        RgbColor::new(255, 255, 0)
    );
    assert_eq!(rainbow.compute_color(2.0, 50, false), RgbColor::GREEN);
    assert_eq!(rainbow.compute_color(3.0, 50, false), RgbColor::CYAN);
    assert_eq!(
        rainbow.compute_color(4.0, 50, false),
        RgbColor::new(0, 0, 255)
    );
    assert_eq!(
        rainbow.compute_color(5.0, 50, false),
        RgbColor::new(255, 0, 255)
    );
    assert_eq!(rainbow.compute_color(-6.0, 50, false), RgbColor::RED);
}

/// REG-10: Manager state gating, disconnect telemetry cleanup, and UDP start/stop idempotency.
#[tokio::test]
async fn regression_manager_state_isolation_and_udp_idempotency() {
    let port = allocate_free_udp_port();
    let manager = DualSenseManager::with_udp_port(port);

    // Even if connected flag is set, disabled extension must mask connection & audio detection
    manager.set_connected(true, "Sony DualSense Wireless Controller");
    let masked = manager.get_status().await;
    assert!(!masked.extension_enabled);
    assert!(!masked.connected);
    assert_eq!(masked.name, "Disconnected");
    assert!(!masked.audio_device_detected);
    assert_eq!(masked.battery.percentage, 0);

    // Enabling extension exposes connection and audio detection
    manager.set_extension_enabled(true);
    let exposed = manager.get_status().await;
    assert!(exposed.connected);
    assert_eq!(exposed.name, "Sony DualSense Wireless Controller");
    assert!(exposed.audio_device_detected);

    // Disconnecting clears stale battery telemetry
    let mut with_battery = DualSenseInputState::default();
    with_battery.battery.percentage = 70;
    manager.update_input_state(with_battery);
    assert_eq!(manager.get_status().await.battery.percentage, 70);

    manager.set_connected(false, "Disconnected");
    assert_eq!(manager.get_status().await.battery.percentage, 0);

    // Native emulation always succeeds regardless of ViGEmBus availability
    assert!(manager.set_emulation(EmulationConfig {
        mode: EmulationMode::Native,
        gyro: GyroSettings::default(),
    }));

    // Lighting getter/setter roundtrip
    let custom_lighting = LightingConfig {
        lightbar: LightbarMode::Static {
            color: RgbColor::CYAN,
        },
        player_led: PlayerLed::Player4,
        mic_led: MicLed::Pulse,
    };
    manager.set_lighting(custom_lighting.clone());
    assert_eq!(manager.get_lighting(), custom_lighting);

    // UDP start and stop must be safely idempotent
    manager.start_udp().await.expect("first start");
    manager.start_udp().await.expect("second idempotent start");
    assert!(manager.get_status().await.udp_stats.running);

    manager.stop_udp().await;
    manager.stop_udp().await;
    assert!(!manager.get_status().await.udp_stats.running);
}

/// REG-11 & REG-12: Bluetooth HD Haptics 0x32 binary layout/CRC32 and 0x31 HD mode flag safety.
#[test]
fn regression_bluetooth_0x32_haptics_packet_and_0x31_hd_mode_flags() {
    use dualsense_core::protocol::{build_bluetooth_haptics_0x32, BT_HAPTICS_REPORT_SIZE};

    let extremes: [[i8; 64]; 3] = [[0; 64], [i8::MAX; 64], [i8::MIN; 64]];
    for (idx, pcm) in extremes.iter().enumerate() {
        let pkt = build_bluetooth_haptics_0x32(idx as u8, (idx as u8) * 10, pcm);
        assert_eq!(pkt.len(), BT_HAPTICS_REPORT_SIZE);
        assert_eq!(pkt[0], 0x32);
        assert_eq!(pkt[1], ((idx as u8) & 0x0F) << 4);
        assert_eq!(&pkt[2..5], &[0x91, 0x07, 0xFE]);
        assert_eq!(pkt[5], 16);
        assert_eq!(pkt[10], (idx as u8) * 10);
        assert_eq!(&pkt[11..13], &[0x92, 0x40]);

        let crc = LittleEndian::read_u32(&pkt[138..142]);
        let mut hasher = Hasher::new();
        hasher.update(&[0xA2]);
        hasher.update(&pkt[0..138]);
        assert_eq!(hasher.finalize(), crc);
    }

    // HD Haptics mode on BT 0x31 must clear legacy rumble bits (0x01|0x02) and never set RELEASE_LEDS (0x08)
    let bt_hd = OutputReportBuilder::new()
        .with_rumble(255, 255)
        .with_bt_hd_haptics(true)
        .build_bluetooth();
    assert_eq!(bt_hd[3], 0x0C);
    assert_eq!(bt_hd[4] & 0x08, 0);
    assert_eq!(bt_hd[4], 0x15 | 0x02 | 0x20 | 0x40);
    assert_eq!(bt_hd[5], 0);
    assert_eq!(bt_hd[6], 0);
}

/// REG-13: ViGEmBus notification IOCTL ABI sizes, parsers, rumble gain clamping, and disconnect zeroing.
#[test]
fn regression_vigem_notification_abi_and_rumble_gain_clamping() {
    use dualsense_core::vigem::{
        apply_rumble_gain, parse_ds4_notification, parse_xusb_notification, Ds4RequestNotification,
        XusbRequestNotification, IOCTL_DS4_REQUEST_NOTIFICATION, IOCTL_XUSB_REQUEST_NOTIFICATION,
    };

    assert_eq!(IOCTL_XUSB_REQUEST_NOTIFICATION, 0x2AA804);
    assert_eq!(IOCTL_DS4_REQUEST_NOTIFICATION, 0x2AA810);
    assert_eq!(size_of::<XusbRequestNotification>(), 12);
    assert_eq!(size_of::<Ds4RequestNotification>(), 16);

    let x_ev = parse_xusb_notification(&XusbRequestNotification {
        size: 12,
        serial_no: 1,
        large_motor: 180,
        small_motor: 90,
        led_number: 0,
    });
    assert_eq!(x_ev.heavy_rumble, Some(180));
    assert_eq!(x_ev.light_rumble, Some(90));

    let ds4_ev = parse_ds4_notification(&Ds4RequestNotification {
        size: 16,
        serial_no: 1,
        large_motor: 200,
        small_motor: 100,
        lightbar_red: 10,
        lightbar_green: 20,
        lightbar_blue: 30,
        flash_on: 0,
        flash_off: 0,
    });
    assert_eq!(ds4_ev.heavy_rumble, Some(200));
    assert_eq!(ds4_ev.lightbar, Some(RgbColor::new(10, 20, 30)));

    assert_eq!(apply_rumble_gain(100, -1.0), 0);
    assert_eq!(apply_rumble_gain(100, 0.5), 50);
    assert_eq!(apply_rumble_gain(200, 2.0), 255);
    assert_eq!(apply_rumble_gain(200, 10.0), 255);
}

/// REG-14: Input mapping pipeline edge cases (deadzones, curves, Edge paddles, gyro activation, touchpad modes).
#[test]
fn regression_input_mapping_edge_cases_and_edge_paddles() {
    use dualsense_core::mapping::{
        apply_input_mapping, GyroActivation, InputMappingConfig, ResponseCurve, StickTuning,
        TouchpadMode,
    };

    // Parse DualSense Edge Fn & Paddle bits from raw USB report byte 10 (buttons3)
    let mut raw_usb = [0u8; 64];
    raw_usb[0] = 0x01;
    raw_usb[1] = 128;
    raw_usb[2] = 128;
    raw_usb[3] = 128;
    raw_usb[4] = 128;
    raw_usb[10] = 0x10 | 0x20 | 0x40 | 0x80; // FnL, FnR, PaddleL, PaddleR
    let parsed = parse_input_report(&raw_usb, ConnectionType::Usb).unwrap();
    assert!(parsed.fn_left && parsed.fn_right && parsed.paddle_left && parsed.paddle_right);

    // Gyro activation HoldL2 zeros gyro when L2 is released, passes gyro when L2 >= 64
    let mut cfg = InputMappingConfig {
        left_stick: StickTuning {
            inner_deadzone: 0.15,
            outer_deadzone: 0.95,
            anti_deadzone: 0.1,
            curve: ResponseCurve::SCurve,
            invert_x: false,
            invert_y: false,
        },
        gyro_activation: GyroActivation::HoldL2,
        gyro_calibration_offset: [10, -20, 30],
        ..Default::default()
    };
    cfg.touchpad.mode = TouchpadMode::Disabled;

    let mut state = parsed;
    state.motion.gyro_x = 110;
    state.motion.gyro_y = 80;
    state.motion.gyro_z = 130;
    state.touch1.active = true;
    state.touchpad_click = true;
    state.l2_trigger = 10;

    let mapped_inactive = apply_input_mapping(&state, &cfg);
    assert_eq!(mapped_inactive.motion.gyro_x, 0);
    assert!(!mapped_inactive.touch1.active);
    assert!(!mapped_inactive.touchpad_click);

    state.l2_trigger = 120;
    let mapped_active = apply_input_mapping(&state, &cfg);
    assert_eq!(mapped_active.motion.gyro_x, 100);
    assert_eq!(mapped_active.motion.gyro_y, 100);
    assert_eq!(mapped_active.motion.gyro_z, 100);
}

/// REG-15 & REG-16: Animated 40 Hz lightbar evaluation & Virtual DualSense USB 0x02 reverse decoding safety.
#[test]
fn regression_animated_lightbar_40hz_and_virtual_dualsense_reverse_decoder() {
    use dualsense_core::virtual_device::{
        decode_dualsense_usb_output_report, decode_trigger_payload,
    };

    let manager = DualSenseManager::with_udp_port(allocate_free_udp_port());
    manager.set_lighting(LightingConfig {
        lightbar: LightbarMode::Rainbow { speed: 1.0 },
        player_led: PlayerLed::Player1,
        mic_led: MicLed::Off,
    });

    let at_t0 = manager.build_output_report_at(0.0).build_usb();
    let at_t2 = manager.build_output_report_at(2.0).build_usb();
    assert_eq!(at_t0[45..48], [255, 0, 0]); // Red at 0 deg
    assert_eq!(at_t2[45..48], [0, 255, 0]); // Green at 120 deg

    // Reject invalid/short Virtual DualSense output reports safely
    assert!(decode_dualsense_usb_output_report(&[]).is_none());
    assert!(decode_dualsense_usb_output_report(&[0x02; 20]).is_none());
    assert!(decode_dualsense_usb_output_report(&[0x01; 64]).is_none());

    // Roundtrip all major trigger opcodes through compile_payload -> decode_trigger_payload
    let effects = [
        TriggerEffect::Off,
        TriggerEffect::Feedback {
            start_position: 2,
            force: 6,
        },
        TriggerEffect::Weapon {
            start_position: 2,
            end_position: 7,
            force: 8,
        },
        TriggerEffect::Bow {
            start_position: 1,
            end_position: 8,
            force: 4,
            snap_force: 7,
        },
        TriggerEffect::MachineGun {
            start_position: 1,
            end_position: 8,
            strength: 7,
            frequency: 20,
        },
        TriggerEffect::Vibration {
            position: 3,
            amplitude: 6,
            frequency: 25,
        },
        TriggerEffect::GameCube,
        TriggerEffect::Choppy,
        TriggerEffect::Hard,
        TriggerEffect::Rigid,
    ];
    for eff in effects {
        let compiled = eff.compile_payload();
        assert_eq!(decode_trigger_payload(&compiled), eff);
    }
}
