use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::protocol::{DualSenseInputState, TouchPoint};

/// Analog stick response curve shaping
#[derive(Debug, Clone, Copy, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ResponseCurve {
    #[default]
    Linear,
    Exponential {
        exponent: f32,
    },
    SCurve,
}

/// Per-stick deadzone, anti-deadzone, curve, and axis inversion settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StickTuning {
    /// Inner deadzone radius (0.0..=0.5, default 0.05)
    pub inner_deadzone: f32,
    /// Outer deadzone saturation radius (0.5..=1.0, default 1.0)
    pub outer_deadzone: f32,
    /// Anti-deadzone floor boost (0.0..=0.3, default 0.0)
    pub anti_deadzone: f32,
    #[serde(default)]
    pub curve: ResponseCurve,
    #[serde(default)]
    pub invert_x: bool,
    #[serde(default)]
    pub invert_y: bool,
}

impl Default for StickTuning {
    fn default() -> Self {
        Self {
            inner_deadzone: 0.05,
            outer_deadzone: 1.0,
            anti_deadzone: 0.0,
            curve: ResponseCurve::Linear,
            invert_x: false,
            invert_y: false,
        }
    }
}

/// Per-trigger deadzone, hair-trigger max threshold, and sensitivity
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TriggerTuning {
    /// Minimum raw value (0..=200) below which output is 0
    pub deadzone: u8,
    /// Value (1..=255) at which output reaches 255 (hair-trigger when < 255)
    pub max_threshold: u8,
    /// Sensitivity multiplier (0.5..=2.0)
    pub sensitivity: f32,
}

impl Default for TriggerTuning {
    fn default() -> Self {
        Self {
            deadzone: 0,
            max_threshold: 255,
            sensitivity: 1.0,
        }
    }
}

/// Remappable physical buttons (DualSense + DualSense Edge)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtonId {
    Cross,
    Circle,
    Square,
    Triangle,
    L1,
    R1,
    L2Click,
    R2Click,
    L3,
    R3,
    Create,
    Options,
    Ps,
    TouchpadClick,
    MicMute,
    DpadUp,
    DpadDown,
    DpadLeft,
    DpadRight,
    FnLeft,
    FnRight,
    PaddleLeft,
    PaddleRight,
}

/// Target action assigned to a physical button
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ButtonAction {
    Button { target: ButtonId },
    Disabled,
    ToggleMicMute,
    NextProfile,
}

/// Touchpad operating mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TouchpadMode {
    #[default]
    Passthrough,
    Mouse,
    SplitButtons,
    Disabled,
}

/// Touchpad fine-tuning settings
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TouchpadConfig {
    pub mode: TouchpadMode,
    pub sensitivity: f32,
    pub tap_to_click: bool,
    pub two_finger_scroll: bool,
    pub invert_scroll: bool,
}

impl Default for TouchpadConfig {
    fn default() -> Self {
        Self {
            mode: TouchpadMode::Passthrough,
            sensitivity: 1.0,
            tap_to_click: true,
            two_finger_scroll: true,
            invert_scroll: false,
        }
    }
}

/// When gyro aiming is active
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GyroActivation {
    #[default]
    Always,
    HoldL2,
    HoldR2,
    HoldTouchpad,
    Toggle,
}

/// Complete deadzone, curve, remapping, touchpad, and motion fine-tuning configuration
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InputMappingConfig {
    #[serde(default)]
    pub left_stick: StickTuning,
    #[serde(default)]
    pub right_stick: StickTuning,
    #[serde(default)]
    pub left_trigger: TriggerTuning,
    #[serde(default)]
    pub right_trigger: TriggerTuning,
    #[serde(default)]
    pub button_remaps: BTreeMap<ButtonId, ButtonAction>,
    #[serde(default)]
    pub touchpad: TouchpadConfig,
    #[serde(default)]
    pub gyro_activation: GyroActivation,
    /// Exponential smoothing factor for gyro noise reduction (0.0 = off, up to 0.95)
    #[serde(default)]
    pub gyro_smoothing: f32,
    /// Calibrated stationary gyro offset (x, y, z) subtracted from raw readings
    #[serde(default)]
    pub gyro_calibration_offset: [i16; 3],
}

impl Default for InputMappingConfig {
    fn default() -> Self {
        Self {
            left_stick: StickTuning::default(),
            right_stick: StickTuning::default(),
            left_trigger: TriggerTuning::default(),
            right_trigger: TriggerTuning::default(),
            button_remaps: BTreeMap::new(),
            touchpad: TouchpadConfig::default(),
            gyro_activation: GyroActivation::Always,
            gyro_smoothing: 0.0,
            gyro_calibration_offset: [0, 0, 0],
        }
    }
}

fn shape_magnitude(mag: f32, curve: ResponseCurve) -> f32 {
    let m = mag.clamp(0.0, 1.0);
    match curve {
        ResponseCurve::Linear => m,
        ResponseCurve::Exponential { exponent } => m.powf(exponent.clamp(0.25, 4.0)),
        ResponseCurve::SCurve => m * m * (3.0 - 2.0 * m),
    }
}

/// Applies radial inner/outer deadzone, anti-deadzone, curve shaping, and axis inversion to an analog stick pair
pub fn transform_stick_pair(raw_x: u8, raw_y: u8, tuning: &StickTuning) -> (u8, u8) {
    let mut nx = ((raw_x as f32) - 128.0) / 127.0;
    let mut ny = ((raw_y as f32) - 128.0) / 127.0;
    nx = nx.clamp(-1.0, 1.0);
    ny = ny.clamp(-1.0, 1.0);

    if tuning.invert_x {
        nx = -nx;
    }
    if tuning.invert_y {
        ny = -ny;
    }

    let inner = tuning.inner_deadzone.clamp(0.0, 0.5);
    let outer = tuning.outer_deadzone.clamp((inner + 0.05).min(1.0), 1.0);
    let anti = tuning.anti_deadzone.clamp(0.0, 0.3);

    let mag = (nx * nx + ny * ny).sqrt();
    if mag <= inner || mag < 1e-6 {
        return (128, 128);
    }

    let normalized = ((mag - inner) / (outer - inner)).clamp(0.0, 1.0);
    let curved = shape_magnitude(normalized, tuning.curve);
    let boosted = (anti + curved * (1.0 - anti)).clamp(0.0, 1.0);

    let scale = boosted / mag;
    let out_x = (nx * scale * 127.0 + 128.0).round().clamp(0.0, 255.0) as u8;
    let out_y = (ny * scale * 127.0 + 128.0).round().clamp(0.0, 255.0) as u8;
    (out_x, out_y)
}

/// Applies deadzone, hair-trigger threshold, and sensitivity scaling to an analog trigger (0..=255)
pub fn transform_trigger(raw: u8, tuning: &TriggerTuning) -> u8 {
    let dz = tuning.deadzone.min(240);
    let max_t = tuning.max_threshold.max(dz.saturating_add(1));
    if raw <= dz {
        return 0;
    }
    if raw >= max_t {
        return 255;
    }
    let norm = ((raw - dz) as f32) / ((max_t - dz) as f32);
    let scaled = (norm * tuning.sensitivity.clamp(0.5, 2.0)).clamp(0.0, 1.0);
    (scaled * 255.0).round().clamp(0.0, 255.0) as u8
}

fn read_button(state: &DualSenseInputState, id: ButtonId) -> bool {
    match id {
        ButtonId::Cross => state.cross,
        ButtonId::Circle => state.circle,
        ButtonId::Square => state.square,
        ButtonId::Triangle => state.triangle,
        ButtonId::L1 => state.l1,
        ButtonId::R1 => state.r1,
        ButtonId::L2Click => state.l2_click,
        ButtonId::R2Click => state.r2_click,
        ButtonId::L3 => state.l3,
        ButtonId::R3 => state.r3,
        ButtonId::Create => state.create,
        ButtonId::Options => state.options,
        ButtonId::Ps => state.ps_button,
        ButtonId::TouchpadClick => state.touchpad_click,
        ButtonId::MicMute => state.mic_mute,
        ButtonId::DpadUp => state.dpad_up,
        ButtonId::DpadDown => state.dpad_down,
        ButtonId::DpadLeft => state.dpad_left,
        ButtonId::DpadRight => state.dpad_right,
        ButtonId::FnLeft => state.fn_left,
        ButtonId::FnRight => state.fn_right,
        ButtonId::PaddleLeft => state.paddle_left,
        ButtonId::PaddleRight => state.paddle_right,
    }
}

fn set_button(state: &mut DualSenseInputState, id: ButtonId, pressed: bool) {
    if !pressed {
        return;
    }
    match id {
        ButtonId::Cross => state.cross = true,
        ButtonId::Circle => state.circle = true,
        ButtonId::Square => state.square = true,
        ButtonId::Triangle => state.triangle = true,
        ButtonId::L1 => state.l1 = true,
        ButtonId::R1 => state.r1 = true,
        ButtonId::L2Click => state.l2_click = true,
        ButtonId::R2Click => state.r2_click = true,
        ButtonId::L3 => state.l3 = true,
        ButtonId::R3 => state.r3 = true,
        ButtonId::Create => state.create = true,
        ButtonId::Options => state.options = true,
        ButtonId::Ps => state.ps_button = true,
        ButtonId::TouchpadClick => state.touchpad_click = true,
        ButtonId::MicMute => state.mic_mute = true,
        ButtonId::DpadUp => state.dpad_up = true,
        ButtonId::DpadDown => state.dpad_down = true,
        ButtonId::DpadLeft => state.dpad_left = true,
        ButtonId::DpadRight => state.dpad_right = true,
        ButtonId::FnLeft => state.fn_left = true,
        ButtonId::FnRight => state.fn_right = true,
        ButtonId::PaddleLeft => state.paddle_left = true,
        ButtonId::PaddleRight => state.paddle_right = true,
    }
}

const ALL_BUTTONS: [ButtonId; 23] = [
    ButtonId::Cross,
    ButtonId::Circle,
    ButtonId::Square,
    ButtonId::Triangle,
    ButtonId::L1,
    ButtonId::R1,
    ButtonId::L2Click,
    ButtonId::R2Click,
    ButtonId::L3,
    ButtonId::R3,
    ButtonId::Create,
    ButtonId::Options,
    ButtonId::Ps,
    ButtonId::TouchpadClick,
    ButtonId::MicMute,
    ButtonId::DpadUp,
    ButtonId::DpadDown,
    ButtonId::DpadLeft,
    ButtonId::DpadRight,
    ButtonId::FnLeft,
    ButtonId::FnRight,
    ButtonId::PaddleLeft,
    ButtonId::PaddleRight,
];

/// Determines whether gyro aiming should be active for the current input frame
pub fn is_gyro_active(state: &DualSenseInputState, activation: GyroActivation) -> bool {
    match activation {
        GyroActivation::Always => true,
        GyroActivation::HoldL2 => state.l2_trigger >= 64 || state.l2_click,
        GyroActivation::HoldR2 => state.r2_trigger >= 64 || state.r2_click,
        GyroActivation::HoldTouchpad => state.touch1.active || state.touchpad_click,
        GyroActivation::Toggle => true,
    }
}

/// Applies the full input mapping pipeline (deadzones, curves, trigger thresholds, button remaps,
/// touchpad modes, and gyro calibration) to a raw DualSense input state.
pub fn apply_input_mapping(
    raw: &DualSenseInputState,
    cfg: &InputMappingConfig,
) -> DualSenseInputState {
    let (lx, ly) = transform_stick_pair(raw.left_stick_x, raw.left_stick_y, &cfg.left_stick);
    let (rx, ry) = transform_stick_pair(raw.right_stick_x, raw.right_stick_y, &cfg.right_stick);
    let l2 = transform_trigger(raw.l2_trigger, &cfg.left_trigger);
    let r2 = transform_trigger(raw.r2_trigger, &cfg.right_trigger);

    let mut out = DualSenseInputState {
        connection: raw.connection,
        left_stick_x: lx,
        left_stick_y: ly,
        right_stick_x: rx,
        right_stick_y: ry,
        l2_trigger: l2,
        r2_trigger: r2,
        battery: raw.battery.clone(),
        motion: raw.motion.clone(),
        touch1: raw.touch1.clone(),
        touch2: raw.touch2.clone(),
        ..Default::default()
    };

    // Subtract calibrated stationary gyro bias
    out.motion.gyro_x = out
        .motion
        .gyro_x
        .saturating_sub(cfg.gyro_calibration_offset[0]);
    out.motion.gyro_y = out
        .motion
        .gyro_y
        .saturating_sub(cfg.gyro_calibration_offset[1]);
    out.motion.gyro_z = out
        .motion
        .gyro_z
        .saturating_sub(cfg.gyro_calibration_offset[2]);

    if !is_gyro_active(raw, cfg.gyro_activation) {
        out.motion.gyro_x = 0;
        out.motion.gyro_y = 0;
        out.motion.gyro_z = 0;
    }

    // Apply button remapping table
    for btn in ALL_BUTTONS {
        let pressed = read_button(raw, btn);
        if !pressed {
            continue;
        }
        match cfg.button_remaps.get(&btn).copied() {
            None => set_button(&mut out, btn, true),
            Some(ButtonAction::Button { target }) => set_button(&mut out, target, true),
            Some(ButtonAction::Disabled) => {}
            Some(ButtonAction::ToggleMicMute) => out.mic_mute = true,
            Some(ButtonAction::NextProfile) => {}
        }
    }

    // Apply touchpad mode overrides
    match cfg.touchpad.mode {
        TouchpadMode::Passthrough => {}
        TouchpadMode::Disabled => {
            out.touch1 = TouchPoint::default();
            out.touch2 = TouchPoint::default();
            out.touchpad_click = false;
        }
        TouchpadMode::SplitButtons => {
            // Left half of touchpad (0..960) -> Create, Right half (960..1920) -> Options when clicked
            if raw.touchpad_click && raw.touch1.active {
                out.touchpad_click = false;
                if raw.touch1.x < 960 {
                    out.create = true;
                } else {
                    out.options = true;
                }
            }
        }
        TouchpadMode::Mouse => {
            // Keep touch coordinates available for mouse delta calculation
        }
    }

    out
}

/// Computes mouse cursor delta (dx, dy) from consecutive touchpad contact samples in Mouse mode
pub fn compute_touchpad_mouse_delta(
    prev: &TouchPoint,
    curr: &TouchPoint,
    cfg: &TouchpadConfig,
) -> Option<(f32, f32)> {
    if cfg.mode != TouchpadMode::Mouse || !prev.active || !curr.active || prev.id != curr.id {
        return None;
    }
    let dx = (curr.x as f32 - prev.x as f32) * cfg.sensitivity.clamp(0.1, 5.0);
    let dy = (curr.y as f32 - prev.y as f32) * cfg.sensitivity.clamp(0.1, 5.0);
    Some((dx, dy))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_stick_deadzone_and_inversion() {
        let tuning = StickTuning {
            inner_deadzone: 0.1,
            outer_deadzone: 0.9,
            anti_deadzone: 0.0,
            curve: ResponseCurve::Linear,
            invert_x: false,
            invert_y: false,
        };

        // Inside deadzone -> exact center
        assert_eq!(transform_stick_pair(135, 122, &tuning), (128, 128));

        // Saturated at outer deadzone -> full right
        let (rx, ry) = transform_stick_pair(250, 128, &tuning);
        assert_eq!(rx, 255);
        assert_eq!(ry, 128);

        // Axis inversion (cardinal axes avoid circular magnitude clamping of diagonals)
        let inv = StickTuning {
            inner_deadzone: 0.0,
            outer_deadzone: 1.0,
            invert_x: true,
            invert_y: true,
            ..Default::default()
        };
        let (ix, _) = transform_stick_pair(255, 128, &inv);
        let (_, iy) = transform_stick_pair(128, 0, &inv);
        assert!(ix <= 2);
        assert!(iy >= 253);
    }

    #[test]
    fn test_stick_anti_deadzone_and_curves() {
        let linear = StickTuning {
            inner_deadzone: 0.0,
            outer_deadzone: 1.0,
            anti_deadzone: 0.2,
            curve: ResponseCurve::Linear,
            ..Default::default()
        };
        let (x_boost, _) = transform_stick_pair(132, 128, &linear);
        // Anti-deadzone lifts small deflection well above 128 + 4
        assert!(x_boost >= 152);

        let exp = StickTuning {
            inner_deadzone: 0.0,
            outer_deadzone: 1.0,
            anti_deadzone: 0.0,
            curve: ResponseCurve::Exponential { exponent: 2.0 },
            ..Default::default()
        };
        // Mid deflection (0.5)^2 = 0.25 -> ~160 instead of ~192
        let (x_exp, _) = transform_stick_pair(192, 128, &exp);
        assert!(x_exp > 150 && x_exp < 170);
    }

    #[test]
    fn test_trigger_hair_trigger_and_deadzone() {
        let tuning = TriggerTuning {
            deadzone: 20,
            max_threshold: 120,
            sensitivity: 1.0,
        };
        assert_eq!(transform_trigger(15, &tuning), 0);
        assert_eq!(transform_trigger(20, &tuning), 0);
        assert_eq!(transform_trigger(70, &tuning), 128);
        assert_eq!(transform_trigger(120, &tuning), 255);
        assert_eq!(transform_trigger(200, &tuning), 255);
    }

    #[test]
    fn test_button_remapping_edge_paddles_and_split_touchpad() {
        let mut cfg = InputMappingConfig::default();
        // Remap Cross -> Circle, PaddleLeft -> Cross, Disable Square
        cfg.button_remaps.insert(
            ButtonId::Cross,
            ButtonAction::Button {
                target: ButtonId::Circle,
            },
        );
        cfg.button_remaps.insert(
            ButtonId::PaddleLeft,
            ButtonAction::Button {
                target: ButtonId::Cross,
            },
        );
        cfg.button_remaps
            .insert(ButtonId::Square, ButtonAction::Disabled);
        cfg.touchpad.mode = TouchpadMode::SplitButtons;

        let raw = DualSenseInputState {
            left_stick_x: 128,
            left_stick_y: 128,
            right_stick_x: 128,
            right_stick_y: 128,
            cross: true,
            square: true,
            paddle_left: true,
            touchpad_click: true,
            touch1: TouchPoint {
                active: true,
                id: 1,
                x: 1500, // right side -> Options
                y: 400,
            },
            ..Default::default()
        };

        let mapped = apply_input_mapping(&raw, &cfg);
        assert!(mapped.circle);
        assert!(mapped.cross); // From PaddleLeft
        assert!(!mapped.square); // Disabled
        assert!(mapped.options); // From right-half touchpad click
        assert!(!mapped.touchpad_click);
    }

    #[test]
    fn test_touchpad_mouse_delta() {
        let cfg = TouchpadConfig {
            mode: TouchpadMode::Mouse,
            sensitivity: 1.5,
            ..Default::default()
        };
        let p1 = TouchPoint {
            active: true,
            id: 5,
            x: 100,
            y: 200,
        };
        let p2 = TouchPoint {
            active: true,
            id: 5,
            x: 120,
            y: 190,
        };
        let (dx, dy) = compute_touchpad_mouse_delta(&p1, &p2, &cfg).unwrap();
        assert!((dx - 30.0).abs() < 1e-4);
        assert!((dy + 15.0).abs() < 1e-4);

        // Different touch ID must not produce a jump
        let p3 = TouchPoint {
            active: true,
            id: 6,
            x: 800,
            y: 400,
        };
        assert!(compute_touchpad_mouse_delta(&p2, &p3, &cfg).is_none());
    }
}
