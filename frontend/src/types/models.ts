export type Language = 'en' | 'pt';

// ==========================================
// DUALSENSE & ADAPTIVE TRIGGERS TYPES
// ==========================================

export type TriggerMode =
  | 'off'
  | 'feedback'
  | 'weapon'
  | 'bow'
  | 'galloping'
  | 'machine_gun'
  | 'machine'
  | 'vibration'
  | 'game_cube'
  | 'choppy'
  | 'very_soft'
  | 'soft'
  | 'medium'
  | 'hard'
  | 'very_hard'
  | 'hardest'
  | 'rigid'
  | 'vibrate_trigger_pulse'
  | 'custom_zone';

export type TriggerEffect =
  | { mode: 'off' }
  | { mode: 'feedback'; start_position: number; force: number }
  | { mode: 'weapon'; start_position: number; end_position: number; force: number }
  | { mode: 'bow'; start_position: number; end_position: number; force: number; snap_force: number }
  | { mode: 'galloping'; start_position: number; end_position: number; first_foot: number; second_foot: number; frequency: number }
  | { mode: 'machine_gun'; start_position: number; end_position: number; strength: number; frequency: number }
  | { mode: 'machine'; start_position: number; end_position: number; strength_a: number; strength_b: number; frequency: number; period: number }
  | { mode: 'vibration'; position: number; amplitude: number; frequency: number }
  | { mode: 'game_cube' }
  | { mode: 'choppy' }
  | { mode: 'very_soft' }
  | { mode: 'soft' }
  | { mode: 'medium' }
  | { mode: 'hard' }
  | { mode: 'very_hard' }
  | { mode: 'hardest' }
  | { mode: 'rigid' }
  | { mode: 'vibrate_trigger_pulse' }
  | { mode: 'custom_zone'; zones: number[] };

export interface DualTriggerConfig {
  left: TriggerEffect;
  right: TriggerEffect;
}

export interface RgbColor {
  r: number;
  g: number;
  b: number;
}

export type PlayerLed = 'off' | 'player1' | 'player2' | 'player3' | 'player4' | 'all';
export type MicLed = 'off' | 'solid' | 'pulse';

export type LightbarMode =
  | { mode: 'off' }
  | { mode: 'static'; color: RgbColor }
  | { mode: 'breathing'; color: RgbColor; speed: number }
  | { mode: 'rainbow'; speed: number }
  | { mode: 'battery_level' };

export interface LightingConfig {
  lightbar: LightbarMode;
  player_led: PlayerLed;
  mic_led: MicLed;
}

export type EmulationMode = 'native' | 'xbox360' | 'dualshock4' | 'dualsense';

export interface GyroSettings {
  enabled: boolean;
  sensitivity: number;
  deadzone: number;
  invert_x: boolean;
  invert_y: boolean;
}

export interface EmulationConfig {
  mode: EmulationMode;
  gyro: GyroSettings;
}

export type ResponseCurve =
  | { type: 'linear' }
  | { type: 'exponential'; exponent: number }
  | { type: 's_curve' };

export interface StickTuning {
  inner_deadzone: number;
  outer_deadzone: number;
  anti_deadzone: number;
  curve: ResponseCurve;
  invert_x: boolean;
  invert_y: boolean;
}

export interface TriggerTuning {
  deadzone: number;
  max_threshold: number;
  sensitivity: number;
}

export type ButtonId =
  | 'cross'
  | 'circle'
  | 'square'
  | 'triangle'
  | 'l1'
  | 'r1'
  | 'l2_click'
  | 'r2_click'
  | 'l3'
  | 'r3'
  | 'create'
  | 'options'
  | 'ps'
  | 'touchpad_click'
  | 'mic_mute'
  | 'dpad_up'
  | 'dpad_down'
  | 'dpad_left'
  | 'dpad_right'
  | 'fn_left'
  | 'fn_right'
  | 'paddle_left'
  | 'paddle_right';

export type ButtonAction =
  | { kind: 'button'; target: ButtonId }
  | { kind: 'disabled' }
  | { kind: 'toggle_mic_mute' }
  | { kind: 'next_profile' };

export type TouchpadMode = 'passthrough' | 'mouse' | 'split_buttons' | 'disabled';

export interface TouchpadConfig {
  mode: TouchpadMode;
  sensitivity: number;
  tap_to_click: boolean;
  two_finger_scroll: boolean;
  invert_scroll: boolean;
}

export type GyroActivation = 'always' | 'hold_l2' | 'hold_r2' | 'hold_touchpad' | 'toggle';

export interface InputMappingConfig {
  left_stick: StickTuning;
  right_stick: StickTuning;
  left_trigger: TriggerTuning;
  right_trigger: TriggerTuning;
  button_remaps: Partial<Record<ButtonId, ButtonAction>>;
  touchpad: TouchpadConfig;
  gyro_activation: GyroActivation;
  gyro_smoothing: number;
  gyro_calibration_offset: [number, number, number];
}

export type HapticsMode = 'legacy_rumble' | 'bluetooth_hd_haptics' | 'audio_to_haptics' | 'off';

export interface HapticsConfig {
  mode: HapticsMode;
  enabled: boolean;
  gain: number;
  lowpass_cutoff_hz: number;
  left_balance: number;
  right_balance: number;
  audio_source: string;
}

export interface HapticsMetrics {
  active: boolean;
  packets_sent: number;
  left_level: number;
  right_level: number;
  sample_rate_hz: number;
}

export interface CemuhookStats {
  running: boolean;
  port: number;
  active_clients: number;
  packets_sent: number;
  packets_received: number;
}

export interface VirtualDualSenseDriverStatus {
  available: boolean;
  active: boolean;
  backend_name: string;
  vigem_installed: boolean;
  hidhide_installed: boolean;
  detail: string;
}

export interface GameProfileRule {
  process_name: string;
  profile_id: string;
  enabled: boolean;
}

export interface TriggerProfile {
  id: string;
  name: string;
  description: string;
  triggers: DualTriggerConfig;
  lightbar: RgbColor;
}

export interface BatteryState {
  percentage: number;
  is_charging: boolean;
  is_full: boolean;
}

export interface UdpServerStats {
  running: boolean;
  port: number;
  packets_received: number;
  last_packet_time: number | null;
  last_sender: string | null;
}

export interface DualSenseStatus {
  extension_enabled: boolean;
  autostart_on_launch: boolean;
  connected: boolean;
  name: string;
  connection_type: 'usb' | 'bluetooth';
  battery: BatteryState;
  triggers: DualTriggerConfig;
  lighting: LightingConfig;
  emulation: EmulationConfig;
  udp_stats: UdpServerStats;
  profiles: TriggerProfile[];
  mic_muted: boolean;
  audio_device_detected: boolean;
  speaker_volume: number;
  mapping: InputMappingConfig;
  haptics: HapticsConfig;
  haptics_metrics: HapticsMetrics;
  rumble_gain: number;
  cemuhook_stats: CemuhookStats;
  virtual_driver: VirtualDualSenseDriverStatus;
  game_profile_rules: GameProfileRule[];
}
