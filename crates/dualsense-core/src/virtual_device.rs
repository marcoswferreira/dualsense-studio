use byteorder::{ByteOrder, LittleEndian};
use serde::{Deserialize, Serialize};

use crate::emulation::{EmulationMode, GyroSettings};
use crate::lightbar::{MicLed, PlayerLed, RgbColor};
use crate::protocol::DualSenseInputState;
use crate::triggers::{DualTriggerConfig, TriggerEffect};

/// Bidirectional feedback event originating from a game writing to a virtual controller
/// (ViGEmBus X360/DS4 or Virtual DualSense via UHID / USB-IP)
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct VirtualFeedbackEvent {
    pub heavy_rumble: Option<u8>,
    pub light_rumble: Option<u8>,
    pub triggers: Option<DualTriggerConfig>,
    pub lightbar: Option<RgbColor>,
    pub player_led: Option<PlayerLed>,
    pub mic_led: Option<MicLed>,
}

/// Diagnostic status for Virtual DualSense & companion drivers
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VirtualDualSenseDriverStatus {
    pub available: bool,
    pub active: bool,
    pub backend_name: String,
    pub vigem_installed: bool,
    pub hidhide_installed: bool,
    pub detail: String,
}

impl Default for VirtualDualSenseDriverStatus {
    fn default() -> Self {
        Self {
            available: false,
            active: false,
            backend_name: "none".to_string(),
            vigem_installed: false,
            hidhide_installed: false,
            detail: "Not initialized".to_string(),
        }
    }
}

/// Pluggable backend abstraction for Virtual Controller devices (ViGEmBus, Linux UHID,
/// Windows USB/IP `usbip-win2`, or custom/licensed driver adapters).
pub trait VirtualControllerBackend: Send {
    fn backend_name(&self) -> &'static str;
    fn is_available(&self) -> bool;
    fn plug(&mut self, mode: EmulationMode) -> Result<(), String>;
    fn unplug(&mut self);
    fn submit_input(&mut self, input: &DualSenseInputState, gyro: &GyroSettings);
    fn poll_feedback(&mut self) -> Option<VirtualFeedbackEvent>;
}

/// Official Sony DualSense USB HID Report Descriptor (condensed standard descriptor for Virtual HID)
pub const DUALSENSE_USB_HID_DESCRIPTOR: &[u8] = &[
    0x05, 0x01, // Usage Page (Generic Desktop Ctrls)
    0x09, 0x05, // Usage (Game Pad)
    0xA1, 0x01, // Collection (Application)
    0x85, 0x01, //   Report ID (1)
    0x09, 0x30, //   Usage (X)
    0x09, 0x31, //   Usage (Y)
    0x09, 0x32, //   Usage (Z)
    0x09, 0x35, //   Usage (Rz)
    0x09, 0x33, //   Usage (Rx - L2)
    0x09, 0x34, //   Usage (Ry - R2)
    0x15, 0x00, //   Logical Minimum (0)
    0x26, 0xFF, 0x00, // Logical Maximum (255)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x06, //   Report Count (6)
    0x81, 0x02, //   Input (Data,Var,Abs)
    0x06, 0x00, 0xFF, // Usage Page (Vendor Defined 0xFF00)
    0x09, 0x20, //   Usage (0x20)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x02, //   Input (Data,Var,Abs)
    0x05, 0x01, //   Usage Page (Generic Desktop Ctrls)
    0x09, 0x39, //   Usage (Hat switch)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x07, //   Logical Maximum (7)
    0x35, 0x00, //   Physical Minimum (0)
    0x46, 0x3B, 0x01, // Physical Maximum (315)
    0x65, 0x14, //   Unit (Degrees)
    0x75, 0x04, //   Report Size (4)
    0x95, 0x01, //   Report Count (1)
    0x81, 0x42, //   Input (Data,Var,Abs,Null)
    0x65, 0x00, //   Unit (None)
    0x05, 0x09, //   Usage Page (Button)
    0x19, 0x01, //   Usage Minimum (0x01)
    0x29, 0x0F, //   Usage Maximum (0x0F)
    0x15, 0x00, //   Logical Minimum (0)
    0x25, 0x01, //   Logical Maximum (1)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x0F, //   Report Count (15)
    0x81, 0x02, //   Input (Data,Var,Abs)
    0x75, 0x01, //   Report Size (1)
    0x95, 0x05, //   Report Count (5)
    0x81, 0x01, //   Input (Const)
    0x06, 0x00, 0xFF, // Usage Page (Vendor Defined)
    0x09, 0x21, //   Usage (0x21 - Telemetry/Touch/Battery)
    0x75, 0x08, //   Report Size (8)
    0x95, 0x35, //   Report Count (53)
    0x81, 0x02, //   Input (Data,Var,Abs)
    0x85, 0x02, //   Report ID (2 - Output Report)
    0x09, 0x22, //   Usage (0x22)
    0x95, 0x3F, //   Report Count (63)
    0x91, 0x02, //   Output (Data,Var,Abs)
    0x85, 0x05, //   Report ID (5 - Calibration Feature)
    0x09, 0x33, //   Usage (0x33)
    0x95, 0x28, //   Report Count (40)
    0xB1, 0x02, //   Feature (Data,Var,Abs)
    0x85, 0x09, //   Report ID (9 - MAC Address Feature)
    0x09, 0x34, //   Usage (0x34)
    0x95, 0x13, //   Report Count (19)
    0xB1, 0x02, //   Feature (Data,Var,Abs)
    0x85, 0x20, //   Report ID (32 - Firmware Info Feature)
    0x09, 0x35, //   Usage (0x35)
    0x95, 0x3F, //   Report Count (63)
    0xB1, 0x02, //   Feature (Data,Var,Abs)
    0xC0, // End Collection
];

/// Encodes a high-level `DualSenseInputState` into a genuine 64-byte DualSense USB `0x01` Input Report
pub fn encode_dualsense_usb_input_report(state: &DualSenseInputState, seq: u8) -> [u8; 64] {
    let mut raw = [0u8; 64];
    raw[0] = 0x01; // USB Input Report ID
    raw[1] = state.left_stick_x;
    raw[2] = state.left_stick_y;
    raw[3] = state.right_stick_x;
    raw[4] = state.right_stick_y;
    raw[5] = state.l2_trigger;
    raw[6] = state.r2_trigger;
    raw[7] = seq;

    let dpad: u8 = match (
        state.dpad_up,
        state.dpad_right,
        state.dpad_down,
        state.dpad_left,
    ) {
        (true, false, false, false) => 0,
        (true, true, false, false) => 1,
        (false, true, false, false) => 2,
        (false, true, true, false) => 3,
        (false, false, true, false) => 4,
        (false, false, true, true) => 5,
        (false, false, false, true) => 6,
        (true, false, false, true) => 7,
        _ => 8,
    };

    let mut b1 = dpad & 0x0F;
    if state.square {
        b1 |= 0x10;
    }
    if state.cross {
        b1 |= 0x20;
    }
    if state.circle {
        b1 |= 0x40;
    }
    if state.triangle {
        b1 |= 0x80;
    }
    raw[8] = b1;

    let mut b2 = 0u8;
    if state.l1 {
        b2 |= 0x01;
    }
    if state.r1 {
        b2 |= 0x02;
    }
    if state.l2_click {
        b2 |= 0x04;
    }
    if state.r2_click {
        b2 |= 0x08;
    }
    if state.create {
        b2 |= 0x10;
    }
    if state.options {
        b2 |= 0x20;
    }
    if state.l3 {
        b2 |= 0x40;
    }
    if state.r3 {
        b2 |= 0x80;
    }
    raw[9] = b2;

    let mut b3 = 0u8;
    if state.ps_button {
        b3 |= 0x01;
    }
    if state.touchpad_click {
        b3 |= 0x02;
    }
    if state.mic_mute {
        b3 |= 0x04;
    }
    if state.fn_left {
        b3 |= 0x10;
    }
    if state.fn_right {
        b3 |= 0x20;
    }
    if state.paddle_left {
        b3 |= 0x40;
    }
    if state.paddle_right {
        b3 |= 0x80;
    }
    raw[10] = b3;

    // Motion telemetry (offset 1 + 15 = 16..28)
    LittleEndian::write_i16(&mut raw[16..18], state.motion.gyro_x);
    LittleEndian::write_i16(&mut raw[18..20], state.motion.gyro_y);
    LittleEndian::write_i16(&mut raw[20..22], state.motion.gyro_z);
    LittleEndian::write_i16(&mut raw[22..24], state.motion.accel_x);
    LittleEndian::write_i16(&mut raw[24..26], state.motion.accel_y);
    LittleEndian::write_i16(&mut raw[26..28], state.motion.accel_z);

    // Touchpoint 1 (bytes 33..37)
    raw[33] = (state.touch1.id & 0x7F) | if state.touch1.active { 0x00 } else { 0x80 };
    raw[34] = (state.touch1.x & 0xFF) as u8;
    raw[35] = (((state.touch1.x >> 8) & 0x0F) as u8) | (((state.touch1.y & 0x0F) as u8) << 4);
    raw[36] = ((state.touch1.y >> 4) & 0xFF) as u8;

    // Battery (byte 53)
    let level_nibble = (state.battery.percentage / 10).min(10);
    let status_nibble = if state.battery.is_charging { 1u8 } else { 0u8 };
    raw[53] = (status_nibble << 4) | level_nibble;

    raw
}

/// Decodes an 11-byte Sony DualSense adaptive trigger payload back into `TriggerEffect`
pub fn decode_trigger_payload(bytes: &[u8]) -> TriggerEffect {
    if bytes.len() < 11 {
        return TriggerEffect::Off;
    }
    match bytes[0] {
        0x00 | 0x05 => TriggerEffect::Off,
        0x21 => TriggerEffect::Feedback {
            start_position: bytes[1].min(9),
            force: bytes[2].min(8),
        },
        0x22 => TriggerEffect::Bow {
            start_position: bytes[1].min(9),
            end_position: bytes[2].min(9),
            force: bytes[3].min(8),
            snap_force: bytes[4].min(8),
        },
        0x23 => TriggerEffect::Galloping {
            start_position: bytes[1].min(9),
            end_position: bytes[2].min(9),
            first_foot: bytes[3].min(9),
            second_foot: bytes[4].min(9),
            frequency: bytes[5],
        },
        0x25 => TriggerEffect::Weapon {
            start_position: bytes[1].min(9),
            end_position: bytes[2].min(9),
            force: bytes[3].min(8),
        },
        0x26 => TriggerEffect::MachineGun {
            start_position: bytes[1].min(9),
            end_position: bytes[2].min(9),
            strength: bytes[3].min(8),
            frequency: bytes[4],
        },
        0x06 => TriggerEffect::Vibration {
            frequency: bytes[1],
            amplitude: bytes[2].min(63),
            position: bytes[3].min(9),
        },
        0x01 => match (bytes[1], bytes[2], bytes[3]) {
            (0x80, 0x20, 0x00) => TriggerEffect::VerySoft,
            (0x45, 0xA0, 0x00) => TriggerEffect::Soft,
            (0x20, 0xFF, 0x00) => TriggerEffect::Hard,
            (0x10, 0xFF, 0x00) => TriggerEffect::VeryHard,
            (0x00, 0xFF, 0xFF) => TriggerEffect::Hardest,
            (0x00, 0xFF, 0x00) => TriggerEffect::Rigid,
            _ => {
                // Standard continuous resistance (opcode 0x01: start 0..255, force 0..255)
                TriggerEffect::Feedback {
                    start_position: ((bytes[1] as u16 * 9) / 255) as u8,
                    force: ((bytes[2] as u16 * 8) / 255) as u8,
                }
            }
        },
        0x02 => match (bytes[1], bytes[2], bytes[3]) {
            (0x90, 0xA0, 0xFF) => TriggerEffect::GameCube,
            (0x02, 0x27, 0x03) => TriggerEffect::Choppy,
            (0x02, 0x01, 0x02) => TriggerEffect::Medium,
            (0x25, 0x06, 0x02) => TriggerEffect::VibrateTriggerPulse,
            _ => {
                // Section resistance (opcode 0x02: start, end, force)
                TriggerEffect::Weapon {
                    start_position: ((bytes[1] as u16 * 9) / 255) as u8,
                    end_position: ((bytes[2] as u16 * 9) / 255) as u8,
                    force: ((bytes[3] as u16 * 8) / 255) as u8,
                }
            }
        },
        0x27 => {
            let mut zones = [0u8; 10];
            zones.copy_from_slice(&bytes[1..11]);
            TriggerEffect::CustomZone { zones }
        }
        _ => TriggerEffect::Off,
    }
}

fn decode_player_led(mask: u8) -> PlayerLed {
    match mask & 0x1F {
        0x00 => PlayerLed::Off,
        0x04 => PlayerLed::Player1,
        0x0A => PlayerLed::Player2,
        0x15 => PlayerLed::Player3,
        0x1B => PlayerLed::Player4,
        _ => PlayerLed::All,
    }
}

/// Parses an incoming game USB `0x02` Output Report (48..=64 bytes) written to a Virtual DualSense
/// into a `VirtualFeedbackEvent` to bridge back to the physical controller (USB or Bluetooth).
pub fn decode_dualsense_usb_output_report(buf: &[u8]) -> Option<VirtualFeedbackEvent> {
    if buf.len() < 48 || buf[0] != 0x02 {
        return None;
    }

    let flag0 = buf[1];
    let flag1 = buf[2];
    let mut event = VirtualFeedbackEvent::default();

    // Rumble flags (0x01 compatible vibration, 0x02 haptics select)
    if (flag0 & 0x03) != 0 {
        event.light_rumble = Some(buf[3]);
        event.heavy_rumble = Some(buf[4]);
    }

    // Mic mute LED (flag1 bit 0x01)
    if (flag1 & 0x01) != 0 {
        event.mic_led = Some(match buf[9] {
            1 => MicLed::Solid,
            2 => MicLed::Pulse,
            _ => MicLed::Off,
        });
    }

    // Adaptive triggers: right trigger (flag0 0x04, bytes 11..22), left trigger (flag0 0x08, bytes 22..33)
    if (flag0 & 0x0C) != 0 {
        let right = if (flag0 & 0x04) != 0 {
            decode_trigger_payload(&buf[11..22])
        } else {
            TriggerEffect::Off
        };
        let left = if (flag0 & 0x08) != 0 {
            decode_trigger_payload(&buf[22..33])
        } else {
            TriggerEffect::Off
        };
        event.triggers = Some(DualTriggerConfig { left, right });
    }

    // Player LEDs (flag1 bit 0x10)
    if (flag1 & 0x10) != 0 {
        event.player_led = Some(decode_player_led(buf[44]));
    }

    // Lightbar RGB (flag1 bit 0x04)
    if (flag1 & 0x04) != 0 {
        event.lightbar = Some(RgbColor::new(buf[45], buf[46], buf[47]));
    }

    Some(event)
}

/// Deterministic in-memory mock Virtual DualSense backend for cross-platform E2E & regression testing
#[derive(Debug, Default)]
pub struct MockVirtualDualSenseBackend {
    plugged_mode: Option<EmulationMode>,
    pub submitted_reports: Vec<[u8; 64]>,
    pending_feedback: Vec<VirtualFeedbackEvent>,
    seq: u8,
}

impl MockVirtualDualSenseBackend {
    pub fn new() -> Self {
        Self::default()
    }

    /// Simulates a PC game writing a raw USB 0x02 output report to the virtual DualSense
    pub fn inject_game_output_report(&mut self, report: &[u8]) -> bool {
        if let Some(ev) = decode_dualsense_usb_output_report(report) {
            self.pending_feedback.push(ev);
            true
        } else {
            false
        }
    }
}

impl VirtualControllerBackend for MockVirtualDualSenseBackend {
    fn backend_name(&self) -> &'static str {
        "mock_virtual_dualsense"
    }

    fn is_available(&self) -> bool {
        true
    }

    fn plug(&mut self, mode: EmulationMode) -> Result<(), String> {
        self.plugged_mode = Some(mode);
        Ok(())
    }

    fn unplug(&mut self) {
        self.plugged_mode = None;
    }

    fn submit_input(&mut self, input: &DualSenseInputState, _gyro: &GyroSettings) {
        if self.plugged_mode == Some(EmulationMode::DualSense) {
            let report = encode_dualsense_usb_input_report(input, self.seq);
            self.seq = self.seq.wrapping_add(1);
            self.submitted_reports.push(report);
        }
    }

    fn poll_feedback(&mut self) -> Option<VirtualFeedbackEvent> {
        if self.pending_feedback.is_empty() {
            None
        } else {
            Some(self.pending_feedback.remove(0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{parse_input_report, ConnectionType, OutputReportBuilder};

    #[test]
    fn test_virtual_dualsense_input_report_roundtrip() {
        let mut input = DualSenseInputState {
            left_stick_x: 200,
            left_stick_y: 40,
            l2_trigger: 120,
            r2_trigger: 240,
            cross: true,
            dpad_up: true,
            ..Default::default()
        };
        input.motion.gyro_z = -450;
        input.battery.percentage = 70;

        let encoded = encode_dualsense_usb_input_report(&input, 9);
        let decoded = parse_input_report(&encoded, ConnectionType::Usb).unwrap();
        assert_eq!(decoded.left_stick_x, 200);
        assert_eq!(decoded.left_stick_y, 40);
        assert_eq!(decoded.l2_trigger, 120);
        assert_eq!(decoded.r2_trigger, 240);
        assert!(decoded.cross);
        assert!(decoded.dpad_up);
        assert_eq!(decoded.motion.gyro_z, -450);
        assert_eq!(decoded.battery.percentage, 70);
    }

    #[test]
    fn test_virtual_dualsense_output_report_bridge() {
        let game_report = OutputReportBuilder::new()
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
            .with_player_led(PlayerLed::Player3)
            .with_mic_led(MicLed::Solid)
            .with_rumble(210, 95)
            .build_usb();

        let feedback = decode_dualsense_usb_output_report(&game_report).unwrap();
        assert_eq!(feedback.heavy_rumble, Some(210));
        assert_eq!(feedback.light_rumble, Some(95));
        assert_eq!(feedback.lightbar, Some(RgbColor::CYAN));
        assert_eq!(feedback.player_led, Some(PlayerLed::Player3));
        assert_eq!(feedback.mic_led, Some(MicLed::Solid));
        let triggers = feedback.triggers.unwrap();
        assert_eq!(
            triggers.left,
            TriggerEffect::Bow {
                start_position: 1,
                end_position: 8,
                force: 4,
                snap_force: 7,
            }
        );
        assert_eq!(
            triggers.right,
            TriggerEffect::Weapon {
                start_position: 2,
                end_position: 6,
                force: 8,
            }
        );
    }
}
