pub mod audio;
pub mod audio_capture;
pub mod cemuhook;
pub mod device;
pub mod emulation;
pub mod haptics_audio;
pub mod lightbar;
pub mod manager;
pub mod mapping;
pub mod profiles;
pub mod protocol;
pub mod storage;
pub mod triggers;
pub mod udp_server;
pub mod uhid_dualsense;
pub mod usbip_dualsense;
pub mod vigem;
pub mod virtual_device;

pub use audio::{execute_set_microphone_mute, execute_toggle_microphone};
pub use cemuhook::{CemuhookStats, CemuhookUdpServer, DEFAULT_CEMUHOOK_UDP_PORT};
pub use device::{
    identify_controller, is_supported_sony_controller, HidDeviceWorker, DUALSENSE_EDGE_PID,
    DUALSENSE_PID, DUALSHOCK4_V1_PID, DUALSHOCK4_V2_PID, SONY_VENDOR_ID,
};
pub use emulation::{EmulationConfig, EmulationMode, GyroSettings};
pub use haptics_audio::{
    generate_test_haptic_blocks, AudioToHapticsDsp, HapticsConfig, HapticsFrameQueue,
    HapticsMetrics, HapticsMode,
};
pub use lightbar::{LightbarMode, LightingConfig, MicLed, PlayerLed, RgbColor};
pub use manager::{DualSenseManager, DualSenseStatus};
pub use mapping::{
    apply_input_mapping, compute_touchpad_mouse_delta, transform_stick_pair, transform_trigger,
    ButtonAction, ButtonId, GyroActivation, InputMappingConfig, ResponseCurve, StickTuning,
    TouchpadConfig, TouchpadMode, TriggerTuning,
};
pub use profiles::TriggerProfile;
pub use protocol::{
    build_bluetooth_haptics_0x32, BatteryState, ConnectionType, DualSenseInputState, MotionData,
    OutputReportBuilder, TouchPoint, BT_HAPTICS_REPORT_SIZE, BT_OUTPUT_REPORT_SIZE,
    USB_OUTPUT_REPORT_SIZE,
};
pub use storage::{default_config_path, AppConfig, GameProfileRule};
pub use triggers::{DualTriggerConfig, TriggerEffect};
pub use udp_server::{DualSenseUdpServer, UdpServerStats, DEFAULT_DUALSENSEX_UDP_PORT};
pub use uhid_dualsense::{is_uhid_available, UhidDualSenseBackend};
pub use usbip_dualsense::{
    is_usbip_client_installed, UsbipVirtualDualSenseServer, DEFAULT_USBIP_PORT,
};
pub use vigem::{
    apply_rumble_gain, is_hidhide_driver_installed, Ds4RequestNotification, VirtualGamepadBus,
    XusbRequestNotification,
};
pub use virtual_device::{
    decode_dualsense_usb_output_report, decode_trigger_payload, encode_dualsense_usb_input_report,
    MockVirtualDualSenseBackend, VirtualControllerBackend, VirtualDualSenseDriverStatus,
    VirtualFeedbackEvent,
};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_full_crate_exports() {
        let manager = DualSenseManager::new();
        let status = manager.get_status().await;
        assert!(!status.extension_enabled);
        assert!(!status.connected);
        assert_eq!(status.battery.percentage, 0);
        assert_eq!(status.connection_type, ConnectionType::Usb);
    }
}
