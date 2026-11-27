use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};

use crate::audio_capture::AudioLoopbackWorker;
use crate::cemuhook::{CemuhookStats, CemuhookUdpServer, DEFAULT_CEMUHOOK_UDP_PORT};
use crate::emulation::{EmulationConfig, EmulationMode};
use crate::haptics_audio::{
    generate_test_haptic_blocks, HapticsConfig, HapticsFrameQueue, HapticsMetrics, HapticsMode,
    BT_HAPTICS_BYTES_PER_PACKET, BT_HAPTICS_SAMPLE_RATE_HZ,
};
use crate::lightbar::{LightbarMode, LightingConfig, MicLed};
use crate::mapping::{
    apply_input_mapping, compute_touchpad_mouse_delta, dispatch_os_mouse,
    has_next_profile_action_pressed, smooth_gyro, GyroActivation, InputMappingConfig, TouchpadMode,
};
use crate::profiles::TriggerProfile;
use crate::protocol::{
    BatteryState, ConnectionType, DualSenseInputState, OutputReportBuilder, TouchPoint,
};
use crate::storage::{AppConfig, GameProfileRule};
use crate::triggers::DualTriggerConfig;
use crate::udp_server::{
    DualSenseUdpServer, SharedControllerState, UdpServerStats, DEFAULT_DUALSENSEX_UDP_PORT,
};
use crate::uhid_dualsense::{is_uhid_available, UhidDualSenseBackend};
use crate::usbip_dualsense::{is_usbip_client_installed, UsbipVirtualDualSenseServer};
use crate::vigem::{
    apply_rumble_gain, is_hidhide_driver_installed, sync_hidhide_for_emulation, VirtualGamepadBus,
};
use crate::virtual_device::{
    VirtualControllerBackend, VirtualDualSenseDriverStatus, VirtualFeedbackEvent,
};

/// Comprehensive controller status report for UI and API clients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DualSenseStatus {
    pub extension_enabled: bool,
    pub autostart_on_launch: bool,
    pub connected: bool,
    pub name: String,
    pub connection_type: ConnectionType,
    pub battery: BatteryState,
    pub triggers: DualTriggerConfig,
    pub lighting: LightingConfig,
    pub emulation: EmulationConfig,
    pub udp_stats: UdpServerStats,
    pub profiles: Vec<TriggerProfile>,
    pub mic_muted: bool,
    pub audio_device_detected: bool,
    pub speaker_volume: u8,
    #[serde(default)]
    pub mapping: InputMappingConfig,
    #[serde(default)]
    pub haptics: HapticsConfig,
    #[serde(default)]
    pub haptics_metrics: HapticsMetrics,
    #[serde(default = "default_rumble_gain")]
    pub rumble_gain: f32,
    #[serde(default)]
    pub cemuhook_stats: CemuhookStats,
    #[serde(default)]
    pub virtual_driver: VirtualDualSenseDriverStatus,
    #[serde(default)]
    pub game_profile_rules: Vec<GameProfileRule>,
}

fn default_rumble_gain() -> f32 {
    1.0
}

/// Callback type for physical microphone mute toggle events
pub type MicToggleHandler = Arc<dyn Fn(bool) + Send + Sync>;

pub struct DualSenseManager {
    extension_enabled: RwLock<bool>,
    autostart_on_launch: RwLock<bool>,
    shared_state: Arc<SharedControllerState>,
    lighting: RwLock<LightingConfig>,
    emulation: RwLock<EmulationConfig>,
    mapping: RwLock<InputMappingConfig>,
    haptics: Arc<RwLock<HapticsConfig>>,
    haptics_queue: Arc<HapticsFrameQueue>,
    haptics_packets_sent: AtomicU64,
    haptics_levels: Arc<RwLock<(f32, f32)>>,
    audio_loopback: parking_lot::Mutex<Option<AudioLoopbackWorker>>,
    rumble_gain: Arc<RwLock<f32>>,
    input_state: RwLock<DualSenseInputState>,
    connected: RwLock<bool>,
    controller_name: RwLock<String>,
    udp_server: tokio::sync::Mutex<DualSenseUdpServer>,
    cemuhook_server: tokio::sync::Mutex<CemuhookUdpServer>,
    cemuhook_port: u16,
    profiles: RwLock<Vec<TriggerProfile>>,
    custom_profiles: RwLock<Vec<TriggerProfile>>,
    game_profile_rules: RwLock<Vec<GameProfileRule>>,
    mic_muted: RwLock<bool>,
    prev_mic_btn: RwLock<bool>,
    prev_next_profile_btn: RwLock<bool>,
    active_profile_idx: RwLock<usize>,
    prev_touch1: RwLock<TouchPoint>,
    prev_touchpad_click: RwLock<bool>,
    gyro_toggle_active: RwLock<bool>,
    prev_gyro_toggle_btn: RwLock<bool>,
    gyro_ema_state: RwLock<[f32; 3]>,
    audio_device_detected: RwLock<bool>,
    speaker_volume: RwLock<u8>,
    mic_toggle_callback: RwLock<Option<MicToggleHandler>>,
    virtual_bus: parking_lot::Mutex<VirtualGamepadBus>,
    virtual_dualsense: parking_lot::Mutex<Box<dyn VirtualControllerBackend>>,
    usbip_server: tokio::sync::Mutex<UsbipVirtualDualSenseServer>,
    usbip_running_flag: RwLock<bool>,
}

impl DualSenseManager {
    pub fn new() -> Self {
        Self::with_ports(DEFAULT_DUALSENSEX_UDP_PORT, DEFAULT_CEMUHOOK_UDP_PORT)
    }

    pub fn with_udp_port(port: u16) -> Self {
        Self::with_ports(port, DEFAULT_CEMUHOOK_UDP_PORT)
    }

    pub fn with_ports(udp_port: u16, cemuhook_port: u16) -> Self {
        let shared_state = Arc::new(SharedControllerState::default());
        let udp_server = DualSenseUdpServer::new(udp_port, shared_state.clone());
        let cemuhook_server = CemuhookUdpServer::new(cemuhook_port);
        let rumble_gain = Arc::new(RwLock::new(1.0f32));

        let mut virtual_bus = VirtualGamepadBus::new();
        {
            let shared_fb = shared_state.clone();
            let gain_fb = rumble_gain.clone();
            virtual_bus.set_feedback_callback(Arc::new(move |ev: VirtualFeedbackEvent| {
                let gain = *gain_fb.read();
                if let Some(heavy) = ev.heavy_rumble {
                    *shared_fb.heavy_rumble.write() = apply_rumble_gain(heavy, gain);
                }
                if let Some(light) = ev.light_rumble {
                    *shared_fb.light_rumble.write() = apply_rumble_gain(light, gain);
                }
                if let Some(rgb) = ev.lightbar {
                    *shared_fb.lightbar.write() = rgb;
                }
            }));
        }

        let initial_input = DualSenseInputState {
            connection: ConnectionType::Usb,
            battery: BatteryState {
                percentage: 0,
                is_charging: false,
                is_full: false,
            },
            ..Default::default()
        };

        Self {
            extension_enabled: RwLock::new(false),
            autostart_on_launch: RwLock::new(false),
            shared_state,
            lighting: RwLock::new(LightingConfig::default()),
            emulation: RwLock::new(EmulationConfig::default()),
            mapping: RwLock::new(InputMappingConfig::default()),
            haptics: Arc::new(RwLock::new(HapticsConfig::default())),
            haptics_queue: Arc::new(HapticsFrameQueue::default()),
            haptics_packets_sent: AtomicU64::new(0),
            haptics_levels: Arc::new(RwLock::new((0.0, 0.0))),
            audio_loopback: parking_lot::Mutex::new(None),
            rumble_gain,
            input_state: RwLock::new(initial_input),
            connected: RwLock::new(false),
            controller_name: RwLock::new("Disconnected".to_string()),
            udp_server: tokio::sync::Mutex::new(udp_server),
            cemuhook_server: tokio::sync::Mutex::new(cemuhook_server),
            cemuhook_port,
            profiles: RwLock::new(TriggerProfile::default_profiles()),
            custom_profiles: RwLock::new(Vec::new()),
            game_profile_rules: RwLock::new(Vec::new()),
            mic_muted: RwLock::new(false),
            prev_mic_btn: RwLock::new(false),
            prev_next_profile_btn: RwLock::new(false),
            active_profile_idx: RwLock::new(0),
            prev_touch1: RwLock::new(TouchPoint::default()),
            prev_touchpad_click: RwLock::new(false),
            gyro_toggle_active: RwLock::new(true),
            prev_gyro_toggle_btn: RwLock::new(false),
            gyro_ema_state: RwLock::new([0.0; 3]),
            audio_device_detected: RwLock::new(false),
            speaker_volume: RwLock::new(80),
            mic_toggle_callback: RwLock::new(None),
            virtual_bus: parking_lot::Mutex::new(virtual_bus),
            virtual_dualsense: parking_lot::Mutex::new(Box::new(UhidDualSenseBackend::new())),
            usbip_server: tokio::sync::Mutex::new(UsbipVirtualDualSenseServer::new(
                "127.0.0.1:3240",
            )),
            usbip_running_flag: RwLock::new(false),
        }
    }

    /// Replaces the pluggable Virtual DualSense backend (for custom drivers or E2E mock testing)
    pub fn set_virtual_dualsense_backend(&self, backend: Box<dyn VirtualControllerBackend>) {
        let mut guard = self.virtual_dualsense.lock();
        guard.unplug();
        *guard = backend;
    }

    pub fn is_extension_enabled(&self) -> bool {
        *self.extension_enabled.read()
    }

    pub fn set_extension_enabled(&self, enabled: bool) {
        *self.extension_enabled.write() = enabled;
        if !enabled {
            self.set_connected(false, "Disconnected");
            self.virtual_bus.lock().unplug();
            self.virtual_dualsense.lock().unplug();
            *self.audio_loopback.lock() = None;
            sync_hidhide_for_emulation(false);
        }
        tracing::info!("DualSense extension enabled state set to: {}", enabled);
    }

    pub fn is_autostart_enabled(&self) -> bool {
        *self.autostart_on_launch.read()
    }

    pub fn set_autostart_enabled(&self, autostart: bool) {
        *self.autostart_on_launch.write() = autostart;
        tracing::info!("DualSense extension autostart set to: {}", autostart);
    }

    pub async fn start_udp(&self) -> Result<(), std::io::Error> {
        let mut server = self.udp_server.lock().await;
        server.start().await
    }

    pub async fn stop_udp(&self) {
        let mut server = self.udp_server.lock().await;
        server.stop();
    }

    pub async fn start_cemuhook(&self) -> Result<(), std::io::Error> {
        let mut server = self.cemuhook_server.lock().await;
        server.start().await
    }

    pub async fn stop_cemuhook(&self) {
        let mut server = self.cemuhook_server.lock().await;
        server.stop();
    }

    pub async fn start_usbip_virtual_dualsense(
        &self,
    ) -> Result<std::net::SocketAddr, std::io::Error> {
        let mut server = self.usbip_server.lock().await;
        let addr = server.start().await?;
        *self.usbip_running_flag.write() = true;
        Ok(addr)
    }

    pub async fn stop_usbip_virtual_dualsense(&self) {
        let mut server = self.usbip_server.lock().await;
        server.stop();
        *self.usbip_running_flag.write() = false;
    }

    pub fn get_virtual_driver_status(&self) -> VirtualDualSenseDriverStatus {
        let emu_mode = self.emulation.read().mode;
        let vds = self.virtual_dualsense.lock();
        let usbip_running = *self.usbip_running_flag.read();
        let uhid_ok = is_uhid_available();
        let usbip_ok = is_usbip_client_installed();
        let vigem_ok = VirtualGamepadBus::is_vigem_driver_installed();
        let hidhide_ok = is_hidhide_driver_installed();

        let (available, backend_name, detail) = if vds.is_available() {
            (
                true,
                vds.backend_name().to_string(),
                format!("Virtual DualSense ready via {}", vds.backend_name()),
            )
        } else if cfg!(target_os = "windows") {
            (
                usbip_ok || usbip_running,
                "windows_usbip_dualsense".to_string(),
                if usbip_ok {
                    "USB/IP Virtual DualSense ready (usbip-win2 detected)".to_string()
                } else {
                    "Install usbip-win2 for native Virtual DualSense, or use ViGEmBus (X360/DS4)"
                        .to_string()
                },
            )
        } else if uhid_ok {
            (
                true,
                "linux_uhid_dualsense".to_string(),
                "Linux /dev/uhid Virtual DualSense ready".to_string(),
            )
        } else {
            (
                false,
                "none".to_string(),
                "No Virtual DualSense kernel driver detected".to_string(),
            )
        };

        VirtualDualSenseDriverStatus {
            available,
            active: emu_mode == EmulationMode::DualSense && (available || usbip_running),
            backend_name,
            vigem_installed: vigem_ok,
            hidhide_installed: hidhide_ok,
            detail,
        }
    }

    pub async fn get_status(&self) -> DualSenseStatus {
        let udp_stats = {
            let server = self.udp_server.lock().await;
            server.stats()
        };
        let cemuhook_stats = {
            let server = self.cemuhook_server.lock().await;
            server.stats()
        };

        let input = self.input_state.read().clone();
        let triggers = self.shared_state.triggers.read().clone();
        let lighting = self.lighting.read().clone();
        let emulation = self.emulation.read().clone();
        let mapping = self.mapping.read().clone();
        let haptics = self.haptics.read().clone();
        let (lvl_l, lvl_r) = *self.haptics_levels.read();
        let haptics_metrics = HapticsMetrics {
            active: haptics.enabled
                && matches!(
                    haptics.mode,
                    HapticsMode::BluetoothHdHaptics | HapticsMode::AudioToHaptics
                ),
            packets_sent: self.haptics_packets_sent.load(Ordering::Relaxed),
            left_level: lvl_l,
            right_level: lvl_r,
            sample_rate_hz: BT_HAPTICS_SAMPLE_RATE_HZ,
        };
        let rumble_gain = *self.rumble_gain.read();
        let extension_enabled = *self.extension_enabled.read();
        let autostart_on_launch = *self.autostart_on_launch.read();
        let connected = extension_enabled && *self.connected.read();
        let name = if connected {
            self.controller_name.read().clone()
        } else {
            "Disconnected".to_string()
        };
        let battery = if connected {
            input.battery
        } else {
            BatteryState::default()
        };
        let mut profiles = self.profiles.read().clone();
        profiles.extend(self.custom_profiles.read().iter().cloned());
        let mic_muted = *self.mic_muted.read();
        let audio_device_detected = connected && *self.audio_device_detected.read();
        let speaker_volume = *self.speaker_volume.read();
        let virtual_driver = self.get_virtual_driver_status();
        let game_profile_rules = self.game_profile_rules.read().clone();

        DualSenseStatus {
            extension_enabled,
            autostart_on_launch,
            connected,
            name,
            connection_type: input.connection,
            battery,
            triggers,
            lighting,
            emulation,
            udp_stats,
            profiles,
            mic_muted,
            audio_device_detected,
            speaker_volume,
            mapping,
            haptics,
            haptics_metrics,
            rumble_gain,
            cemuhook_stats,
            virtual_driver,
            game_profile_rules,
        }
    }

    pub fn toggle_microphone(&self) -> bool {
        let mut muted = self.mic_muted.write();
        *muted = !*muted;
        let is_muted = *muted;

        let mut lighting = self.lighting.write();
        lighting.mic_led = if is_muted { MicLed::Solid } else { MicLed::Off };

        tracing::info!(
            "DualSense microphone mute toggled: muted={}, mic_led={:?}",
            is_muted,
            lighting.mic_led
        );

        is_muted
    }

    pub fn set_microphone_mute(&self, muted: bool) {
        *self.mic_muted.write() = muted;
        let mut lighting = self.lighting.write();
        lighting.mic_led = if muted { MicLed::Solid } else { MicLed::Off };
        tracing::info!(
            "DualSense microphone mute explicitly set: muted={}, mic_led={:?}",
            muted,
            lighting.mic_led
        );
    }

    pub fn set_speaker_volume(&self, volume: u8) {
        let clamped = volume.min(100);
        *self.speaker_volume.write() = clamped;
        tracing::info!("DualSense speaker volume set to {}%", clamped);
    }

    pub fn set_triggers(&self, triggers: DualTriggerConfig) {
        tracing::info!(
            "DualSense adaptive triggers configured: L2={:?}, R2={:?}",
            triggers.left,
            triggers.right
        );
        *self.shared_state.triggers.write() = triggers;
    }

    pub fn set_lighting(&self, lighting: LightingConfig) {
        let input = self.input_state.read().clone();
        let resolved_color = lighting.lightbar.compute_color(
            0.0,
            input.battery.percentage,
            input.battery.is_charging,
        );
        *self.shared_state.lightbar.write() = resolved_color;
        tracing::info!(
            "DualSense lighting configured: mode={:?}, resolved_rgb=({},{},{}), player_led={:?} (0x{:02x}), mic_led={:?}",
            lighting.lightbar,
            resolved_color.r,
            resolved_color.g,
            resolved_color.b,
            lighting.player_led,
            lighting.player_led.to_bitmask(),
            lighting.mic_led
        );
        *self.lighting.write() = lighting;
    }

    pub fn set_mapping(&self, mapping: InputMappingConfig) {
        *self.mapping.write() = mapping;
        tracing::info!("DualSense input mapping & tuning updated");
    }

    pub fn get_mapping(&self) -> InputMappingConfig {
        self.mapping.read().clone()
    }

    pub fn set_haptics(&self, mut config: HapticsConfig) {
        config.gain = config.gain.clamp(0.0, 2.0);
        config.lowpass_cutoff_hz = config.lowpass_cutoff_hz.clamp(40.0, 500.0);
        config.left_balance = config.left_balance.clamp(0.0, 1.0);
        config.right_balance = config.right_balance.clamp(0.0, 1.0);
        let want_loopback = config.enabled && config.mode == HapticsMode::AudioToHaptics;
        if !config.enabled || config.mode == HapticsMode::Off {
            self.haptics_queue.clear();
            *self.haptics_levels.write() = (0.0, 0.0);
        }
        *self.haptics.write() = config;

        let mut worker = self.audio_loopback.lock();
        if want_loopback && worker.is_none() {
            *worker = Some(AudioLoopbackWorker::start(
                self.haptics.clone(),
                self.haptics_queue.clone(),
                self.haptics_levels.clone(),
            ));
        } else if !want_loopback && worker.is_some() {
            *worker = None;
        }
        tracing::info!("DualSense Bluetooth HD haptics config updated");
    }

    pub fn get_haptics(&self) -> HapticsConfig {
        self.haptics.read().clone()
    }

    pub fn is_bt_hd_haptics_active(&self) -> bool {
        let h = self.haptics.read();
        h.enabled
            && matches!(
                h.mode,
                HapticsMode::BluetoothHdHaptics | HapticsMode::AudioToHaptics
            )
    }

    /// Enqueues synthetic 3 kHz S8 haptic blocks for immediate Bluetooth 0x32 testing
    pub fn trigger_hd_haptic_pulse(&self, freq_hz: f32, amplitude: f32, duration_ms: u32) -> usize {
        let blocks = generate_test_haptic_blocks(freq_hz, amplitude, duration_ms);
        let count = blocks.len();
        self.haptics_queue.push_many(blocks);
        *self.haptics_levels.write() = (amplitude.clamp(0.0, 1.0), amplitude.clamp(0.0, 1.0));
        count
    }

    pub fn push_haptic_block(&self, block: [i8; BT_HAPTICS_BYTES_PER_PACKET], levels: (f32, f32)) {
        self.haptics_queue.push(block);
        *self.haptics_levels.write() = levels;
    }

    pub fn pop_next_haptic_block(&self) -> Option<[i8; BT_HAPTICS_BYTES_PER_PACKET]> {
        self.haptics_queue.pop()
    }

    pub fn record_haptic_packet_sent(&self) {
        self.haptics_packets_sent.fetch_add(1, Ordering::Relaxed);
    }

    pub fn set_rumble_gain(&self, gain: f32) {
        *self.rumble_gain.write() = gain.clamp(0.0, 2.0);
    }

    pub fn get_rumble_gain(&self) -> f32 {
        *self.rumble_gain.read()
    }

    pub fn set_rumble_direct(&self, heavy: u8, light: u8) {
        let gain = *self.rumble_gain.read();
        *self.shared_state.heavy_rumble.write() = apply_rumble_gain(heavy, gain);
        *self.shared_state.light_rumble.write() = apply_rumble_gain(light, gain);
    }

    pub fn set_emulation(&self, config: EmulationConfig) -> bool {
        tracing::info!(
            "DualSense emulation configured: mode={:?}, gyro_enabled={}, gyro_sensitivity={}",
            config.mode,
            config.gyro.enabled,
            config.gyro.sensitivity
        );
        let ok = match config.mode {
            EmulationMode::DualSense => {
                sync_hidhide_for_emulation(false);
                self.virtual_bus.lock().unplug();
                let mut vds = self.virtual_dualsense.lock();
                if vds.is_available() {
                    vds.plug(EmulationMode::DualSense).is_ok()
                } else {
                    // On Windows with USB/IP, server can be started asynchronously via start_usbip_virtual_dualsense
                    *self.usbip_running_flag.read() || is_usbip_client_installed()
                }
            }
            EmulationMode::Native => {
                sync_hidhide_for_emulation(false);
                self.virtual_dualsense.lock().unplug();
                self.virtual_bus.lock().set_mode(EmulationMode::Native)
            }
            other => {
                sync_hidhide_for_emulation(true);
                self.virtual_dualsense.lock().unplug();
                self.virtual_bus.lock().set_mode(other)
            }
        };
        *self.emulation.write() = config;
        ok
    }

    /// Applies bidirectional feedback received from a virtual controller (Virtual DualSense / DS4 / X360)
    pub fn apply_virtual_feedback(&self, event: VirtualFeedbackEvent) {
        let gain = *self.rumble_gain.read();
        if let Some(heavy) = event.heavy_rumble {
            *self.shared_state.heavy_rumble.write() = apply_rumble_gain(heavy, gain);
        }
        if let Some(light) = event.light_rumble {
            *self.shared_state.light_rumble.write() = apply_rumble_gain(light, gain);
        }
        if let Some(triggers) = event.triggers {
            *self.shared_state.triggers.write() = triggers;
        }
        if let Some(rgb) = event.lightbar {
            *self.shared_state.lightbar.write() = rgb;
            self.lighting.write().lightbar = LightbarMode::Static { color: rgb };
        }
        if let Some(player_led) = event.player_led {
            self.lighting.write().player_led = player_led;
        }
        if let Some(mic_led) = event.mic_led {
            self.lighting.write().mic_led = mic_led;
        }
    }

    /// Polls pending feedback events from the active Virtual DualSense backend and applies them
    pub fn poll_virtual_feedback(&self) -> usize {
        let mut applied = 0usize;
        loop {
            let next = self.virtual_dualsense.lock().poll_feedback();
            match next {
                Some(ev) => {
                    self.apply_virtual_feedback(ev);
                    applied += 1;
                }
                None => break,
            }
        }
        if let Ok(usbip) = self.usbip_server.try_lock() {
            while let Some(ev) = usbip.pop_feedback() {
                self.apply_virtual_feedback(ev);
                applied += 1;
            }
        }
        applied
    }

    pub fn add_custom_profile(&self, profile: TriggerProfile) {
        let mut custom = self.custom_profiles.write();
        if let Some(existing) = custom.iter_mut().find(|p| p.id == profile.id) {
            *existing = profile;
        } else {
            custom.push(profile);
        }
    }

    pub fn delete_custom_profile(&self, profile_id: &str) -> bool {
        let mut custom = self.custom_profiles.write();
        let before = custom.len();
        custom.retain(|p| p.id != profile_id);
        custom.len() < before
    }

    pub fn set_game_profile_rules(&self, rules: Vec<GameProfileRule>) {
        *self.game_profile_rules.write() = rules;
    }

    pub fn get_game_profile_rules(&self) -> Vec<GameProfileRule> {
        self.game_profile_rules.read().clone()
    }

    /// Cycles to the next built-in or custom trigger/lightbar profile and returns its ID.
    pub fn cycle_next_profile(&self) -> Option<String> {
        let mut all_ids: Vec<String> = self.profiles.read().iter().map(|p| p.id.clone()).collect();
        all_ids.extend(self.custom_profiles.read().iter().map(|p| p.id.clone()));
        if all_ids.is_empty() {
            return None;
        }
        let next_id = {
            let mut idx = self.active_profile_idx.write();
            *idx = (*idx + 1) % all_ids.len();
            all_ids[*idx].clone()
        };
        if self.apply_profile(&next_id) {
            Some(next_id)
        } else {
            None
        }
    }

    pub fn apply_profile(&self, profile_id: &str) -> bool {
        let found = self
            .profiles
            .read()
            .iter()
            .find(|p| p.id == profile_id)
            .cloned()
            .or_else(|| {
                self.custom_profiles
                    .read()
                    .iter()
                    .find(|p| p.id == profile_id)
                    .cloned()
            });

        if let Some(p) = found {
            *self.shared_state.triggers.write() = p.triggers.clone();
            *self.shared_state.lightbar.write() = p.lightbar;
            let mut lighting = self.lighting.write();
            lighting.lightbar = LightbarMode::Static { color: p.lightbar };
            tracing::info!(
                "DualSense preset profile applied: id='{}' ({}), L2={}, R2={}, RGB=({},{},{})",
                p.id,
                p.name,
                p.triggers.left.name(),
                p.triggers.right.name(),
                p.lightbar.r,
                p.lightbar.g,
                p.lightbar.b
            );
            true
        } else {
            tracing::warn!("DualSense preset profile not found: id='{}'", profile_id);
            false
        }
    }

    /// Generates current output report snapshot at elapsed_secs = 0.0
    pub fn build_current_output_report(&self) -> OutputReportBuilder {
        self.build_output_report_at(0.0)
    }

    /// Generates current output report evaluating animated lightbar modes (`Breathing`, `Rainbow`,
    /// `BatteryLevel` charging pulse) at `elapsed_secs` for the 40 Hz HID output loop.
    pub fn build_output_report_at(&self, elapsed_secs: f32) -> OutputReportBuilder {
        let triggers = self.shared_state.triggers.read().clone();
        let lighting = self.lighting.read().clone();
        let input = self.input_state.read().clone();
        let lightbar_color = match &lighting.lightbar {
            LightbarMode::Breathing { .. }
            | LightbarMode::Rainbow { .. }
            | LightbarMode::BatteryLevel => lighting.lightbar.compute_color(
                elapsed_secs,
                input.battery.percentage,
                input.battery.is_charging,
            ),
            LightbarMode::Off => crate::lightbar::RgbColor::BLACK,
            LightbarMode::Static { .. } => *self.shared_state.lightbar.read(),
        };
        let heavy = *self.shared_state.heavy_rumble.read();
        let light = *self.shared_state.light_rumble.read();
        let bt_hd_haptics = self.is_bt_hd_haptics_active();

        OutputReportBuilder::new()
            .with_triggers(triggers)
            .with_lightbar(lightbar_color)
            .with_player_led(lighting.player_led)
            .with_mic_led(lighting.mic_led)
            .with_rumble(heavy, light)
            .with_bt_hd_haptics(bt_hd_haptics)
    }

    pub fn get_triggers(&self) -> DualTriggerConfig {
        self.shared_state.triggers.read().clone()
    }

    pub fn get_lighting(&self) -> LightingConfig {
        self.lighting.read().clone()
    }

    /// Returns the latest mapped physical controller HID input state (sticks, triggers, buttons, IMU, touchpad)
    pub fn get_input_state(&self) -> DualSenseInputState {
        self.input_state.read().clone()
    }

    /// Returns whether the physical controller is currently connected and enabled
    pub fn is_connected(&self) -> bool {
        *self.extension_enabled.read() && *self.connected.read()
    }

    /// Returns the active controller display name
    pub fn controller_name(&self) -> String {
        if self.is_connected() {
            self.controller_name.read().clone()
        } else {
            "Disconnected".to_string()
        }
    }

    pub fn cemuhook_port(&self) -> u16 {
        self.cemuhook_port
    }

    /// Exports current configuration into a serializable `AppConfig` for disk persistence
    pub async fn export_config(&self) -> AppConfig {
        let udp_enabled = self.udp_server.lock().await.stats().running;
        let cemuhook_enabled = self.cemuhook_server.lock().await.stats().running;
        AppConfig {
            extension_enabled: *self.extension_enabled.read(),
            autostart_on_launch: *self.autostart_on_launch.read(),
            triggers: self.shared_state.triggers.read().clone(),
            lighting: self.lighting.read().clone(),
            emulation: self.emulation.read().clone(),
            mapping: self.mapping.read().clone(),
            haptics: self.haptics.read().clone(),
            rumble_gain: *self.rumble_gain.read(),
            udp_enabled,
            cemuhook_enabled,
            custom_profiles: self.custom_profiles.read().clone(),
            game_profile_rules: self.game_profile_rules.read().clone(),
        }
    }

    /// Applies a persisted `AppConfig` onto this manager
    pub fn apply_config(&self, cfg: &AppConfig) {
        self.set_extension_enabled(cfg.extension_enabled);
        self.set_autostart_enabled(cfg.autostart_on_launch);
        self.set_triggers(cfg.triggers.clone());
        self.set_lighting(cfg.lighting.clone());
        self.set_mapping(cfg.mapping.clone());
        self.set_haptics(cfg.haptics.clone());
        self.set_rumble_gain(cfg.rumble_gain);
        *self.custom_profiles.write() = cfg.custom_profiles.clone();
        *self.game_profile_rules.write() = cfg.game_profile_rules.clone();
        let _ = self.set_emulation(cfg.emulation.clone());
    }

    pub fn set_connected(&self, connected: bool, name: &str) {
        *self.connected.write() = connected;
        *self.controller_name.write() = name.to_string();
        *self.audio_device_detected.write() = connected;
        if !connected {
            *self.input_state.write() = DualSenseInputState::default();
            *self.shared_state.heavy_rumble.write() = 0;
            *self.shared_state.light_rumble.write() = 0;
        }
    }

    pub fn set_mic_toggle_handler(&self, handler: MicToggleHandler) {
        *self.mic_toggle_callback.write() = Some(handler);
    }

    pub fn update_input_state(&self, raw_state: DualSenseInputState) -> Option<bool> {
        let mapping_cfg = self.mapping.read().clone();
        let mut mapped_state = apply_input_mapping(&raw_state, &mapping_cfg);

        // Handle GyroActivation::Toggle on touchpad_click rising edge
        if mapping_cfg.gyro_activation == GyroActivation::Toggle {
            let prev_gt = *self.prev_gyro_toggle_btn.read();
            if raw_state.touchpad_click && !prev_gt {
                let mut active = self.gyro_toggle_active.write();
                *active = !*active;
            }
            *self.prev_gyro_toggle_btn.write() = raw_state.touchpad_click;
            if !*self.gyro_toggle_active.read() {
                mapped_state.motion.gyro_x = 0;
                mapped_state.motion.gyro_y = 0;
                mapped_state.motion.gyro_z = 0;
            }
        }

        // Apply exponential smoothing to gyroscope if configured
        if mapping_cfg.gyro_smoothing > 0.0 {
            let prev_ema = *self.gyro_ema_state.read();
            let (next_ema, (gx, gy, gz)) = smooth_gyro(
                prev_ema,
                (
                    mapped_state.motion.gyro_x,
                    mapped_state.motion.gyro_y,
                    mapped_state.motion.gyro_z,
                ),
                mapping_cfg.gyro_smoothing,
            );
            *self.gyro_ema_state.write() = next_ema;
            mapped_state.motion.gyro_x = gx;
            mapped_state.motion.gyro_y = gy;
            mapped_state.motion.gyro_z = gz;
        }

        // Handle ButtonAction::NextProfile rising edge
        let next_prof_pressed = has_next_profile_action_pressed(&raw_state, &mapping_cfg);
        let prev_next_prof = *self.prev_next_profile_btn.read();
        if next_prof_pressed && !prev_next_prof {
            self.cycle_next_profile();
        }
        *self.prev_next_profile_btn.write() = next_prof_pressed;

        // Handle TouchpadMode::Mouse OS cursor movement, click, and two-finger scroll
        if mapping_cfg.touchpad.mode == TouchpadMode::Mouse {
            let prev_t1 = self.prev_touch1.read().clone();
            let mut dx = 0i32;
            let mut dy = 0i32;
            let mut wheel = 0i32;
            if let Some((fdx, fdy)) =
                compute_touchpad_mouse_delta(&prev_t1, &raw_state.touch1, &mapping_cfg.touchpad)
            {
                if mapping_cfg.touchpad.two_finger_scroll && raw_state.touch2.active {
                    let sign = if mapping_cfg.touchpad.invert_scroll {
                        1.0
                    } else {
                        -1.0
                    };
                    wheel = (fdy * sign * 4.0).round() as i32;
                } else {
                    dx = (fdx * 0.65).round() as i32;
                    dy = (fdy * 0.65).round() as i32;
                }
            }
            let click_edge = if mapping_cfg.touchpad.tap_to_click {
                let prev_click = *self.prev_touchpad_click.read();
                if raw_state.touchpad_click != prev_click {
                    *self.prev_touchpad_click.write() = raw_state.touchpad_click;
                    Some(raw_state.touchpad_click)
                } else {
                    None
                }
            } else {
                None
            };
            dispatch_os_mouse(dx, dy, click_edge, wheel);
        }
        *self.prev_touch1.write() = raw_state.touch1.clone();

        let mut toggled_to = None;
        let prev_pressed = *self.prev_mic_btn.read();
        let curr_pressed = mapped_state.mic_mute;
        if curr_pressed && !prev_pressed {
            let new_muted = self.toggle_microphone();
            if let Some(cb) = self.mic_toggle_callback.read().as_ref() {
                cb(new_muted);
            }
            toggled_to = Some(new_muted);
        }
        let lightbar_mode = self.lighting.read().lightbar.clone();
        if matches!(lightbar_mode, LightbarMode::BatteryLevel) {
            *self.shared_state.lightbar.write() = lightbar_mode.compute_color(
                0.0,
                mapped_state.battery.percentage,
                mapped_state.battery.is_charging,
            );
        }
        *self.prev_mic_btn.write() = curr_pressed;
        *self.input_state.write() = mapped_state.clone();

        // Update Cemuhook motion server state (non-blocking)
        if let Ok(cemu) = self.cemuhook_server.try_lock() {
            cemu.update_state(*self.connected.read(), mapped_state.clone());
        }

        // Forward mapped input state to emulated virtual controller (Xbox 360 / DS4 / Virtual DualSense)
        let emu = self.emulation.read().clone();
        match emu.mode {
            EmulationMode::DualSense => {
                self.virtual_dualsense
                    .lock()
                    .submit_input(&mapped_state, &emu.gyro);
                if let Ok(usbip) = self.usbip_server.try_lock() {
                    usbip.update_input(mapped_state);
                }
            }
            EmulationMode::Xbox360 | EmulationMode::DualShock4 => {
                self.virtual_bus
                    .lock()
                    .submit_input(&mapped_state, &emu.gyro);
            }
            EmulationMode::Native => {}
        }

        toggled_to
    }
}

impl Default for DualSenseManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lightbar::RgbColor;
    use crate::triggers::TriggerEffect;

    #[tokio::test]
    async fn test_manager_status_and_updates() {
        let manager = DualSenseManager::new();
        let status = manager.get_status().await;
        assert!(!status.extension_enabled);
        assert!(!status.autostart_on_launch);
        assert!(!status.connected);
        assert_eq!(status.battery.percentage, 0);

        manager.set_extension_enabled(true);
        manager.set_autostart_enabled(true);
        manager.set_connected(true, "Sony DualSense Wireless Controller");
        let active_status = manager.get_status().await;
        assert!(active_status.extension_enabled);
        assert!(active_status.autostart_on_launch);
        assert!(active_status.connected);

        // Update triggers
        manager.set_triggers(DualTriggerConfig {
            left: TriggerEffect::Feedback {
                start_position: 1,
                force: 4,
            },
            right: TriggerEffect::Weapon {
                start_position: 2,
                end_position: 6,
                force: 7,
            },
        });

        let updated_status = manager.get_status().await;
        assert_eq!(
            updated_status.triggers.right,
            TriggerEffect::Weapon {
                start_position: 2,
                end_position: 6,
                force: 7
            }
        );

        // Apply profile
        let ok = manager.apply_profile("racing_abs");
        assert!(ok);
        assert!(!manager.apply_profile("unknown_id"));

        let racing_status = manager.get_status().await;
        assert_eq!(
            racing_status.lighting.lightbar,
            LightbarMode::Static {
                color: RgbColor::RED
            }
        );

        // Test output report builder
        let report = manager.build_current_output_report();
        let usb = report.build_usb();
        assert_eq!(usb[0], 0x02);

        // Test Microphone Mute toggle and LED sync
        assert!(!manager.get_status().await.mic_muted);
        let toggled = manager.toggle_microphone();
        assert!(toggled);
        assert!(manager.get_status().await.mic_muted);
        assert_eq!(manager.get_status().await.lighting.mic_led, MicLed::Solid);

        let toggled_back = manager.toggle_microphone();
        assert!(!toggled_back);
        assert!(!manager.get_status().await.mic_muted);
        assert_eq!(manager.get_status().await.lighting.mic_led, MicLed::Off);

        // Test physical button edge detection
        let input_press = DualSenseInputState {
            mic_mute: true,
            ..Default::default()
        };
        let event = manager.update_input_state(input_press);
        assert_eq!(event, Some(true));
        assert!(manager.get_status().await.mic_muted);

        let input_hold = DualSenseInputState {
            mic_mute: true,
            ..Default::default()
        };
        assert_eq!(manager.update_input_state(input_hold), None);

        let input_release = DualSenseInputState {
            mic_mute: false,
            ..Default::default()
        };
        assert_eq!(manager.update_input_state(input_release), None);

        let input_press2 = DualSenseInputState {
            mic_mute: true,
            ..Default::default()
        };
        assert_eq!(manager.update_input_state(input_press2), Some(false));
        assert!(!manager.get_status().await.mic_muted);
    }

    #[tokio::test]
    async fn test_gyro_toggle_smoothing_and_next_profile_cycle() {
        let manager = DualSenseManager::new();
        let mut mapping = InputMappingConfig {
            gyro_activation: GyroActivation::Toggle,
            gyro_smoothing: 0.5,
            ..Default::default()
        };
        mapping.button_remaps.insert(
            crate::mapping::ButtonId::Create,
            crate::ButtonAction::NextProfile,
        );
        manager.set_mapping(mapping);

        // Initially gyro toggle is active -> 0.5 EMA smoothing on 1000 yields 500
        let mut raw = DualSenseInputState::default();
        raw.motion.gyro_x = 1000;
        manager.update_input_state(raw.clone());
        assert_eq!(manager.get_input_state().motion.gyro_x, 500);
        assert!(*manager.gyro_toggle_active.read());

        // Press touchpad_click to toggle gyro OFF, and Create to cycle to profile 1
        raw.touchpad_click = true;
        raw.create = true;
        manager.update_input_state(raw.clone());
        assert!(!*manager.gyro_toggle_active.read());
        assert_eq!(manager.get_input_state().motion.gyro_x, 250);
        assert_eq!(*manager.active_profile_idx.read(), 1);

        // Release both; gyro stays OFF
        raw.touchpad_click = false;
        raw.create = false;
        manager.update_input_state(raw.clone());
        assert!(!*manager.gyro_toggle_active.read());
        assert_eq!(manager.get_input_state().motion.gyro_x, 125);

        // Press touchpad_click to toggle gyro back ON, and Create to cycle to profile 2
        raw.touchpad_click = true;
        raw.create = true;
        manager.update_input_state(raw);
        assert!(*manager.gyro_toggle_active.read());
        assert_eq!(*manager.active_profile_idx.read(), 2);
    }
}
