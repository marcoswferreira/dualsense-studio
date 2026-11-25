use std::collections::VecDeque;

use crate::emulation::{EmulationMode, GyroSettings};
use crate::protocol::DualSenseInputState;
use crate::virtual_device::{
    decode_dualsense_usb_output_report, encode_dualsense_usb_input_report,
    VirtualControllerBackend, VirtualFeedbackEvent,
};

#[cfg(target_os = "linux")]
use std::fs::{File, OpenOptions};
#[cfg(target_os = "linux")]
use std::io::{Read, Write};
#[cfg(target_os = "linux")]
use std::os::unix::fs::OpenOptionsExt;

#[cfg(target_os = "linux")]
use crate::device::{DUALSENSE_PID, SONY_VENDOR_ID};
#[cfg(target_os = "linux")]
use crate::virtual_device::DUALSENSE_USB_HID_DESCRIPTOR;

/// Standard DualSense Feature Report 0x05 (Calibration), 0x09 (MAC), and 0x20 (Firmware Info)
/// responses for Virtual DualSense enumeration so games & Steam Input accept the virtual pad.
pub fn build_dualsense_feature_report(report_id: u8) -> Option<Vec<u8>> {
    match report_id {
        0x05 => {
            // 41-byte calibration report with neutral IMU biases and unit scales
            let mut rep = vec![0u8; 41];
            rep[0] = 0x05;
            // Pitch/Yaw/Roll plus/minus ~13500
            for idx in [7usize, 9, 11, 15, 17, 19] {
                rep[idx] = 0xB4;
                rep[idx + 1] = 0x34;
            }
            for idx in [13usize, 21, 23, 25, 27, 29, 31, 33, 35] {
                rep[idx] = 0x00;
                rep[idx + 1] = 0x20;
            }
            Some(rep)
        }
        0x09 => {
            // 20-byte Pairing / MAC info report
            let mut rep = vec![0u8; 20];
            rep[0] = 0x09;
            rep[1..7].copy_from_slice(&[0xE6, 0x0C, 0x4C, 0x05, 0x50, 0xD5]);
            Some(rep)
        }
        0x20 => {
            // 64-byte Firmware info report
            let mut rep = vec![0u8; 64];
            rep[0] = 0x20;
            rep[1..12].copy_from_slice(b"Oct 28 2023");
            rep[12..20].copy_from_slice(b"00:00:00");
            Some(rep)
        }
        _ => None,
    }
}

/// Checks whether Linux `/dev/uhid` is accessible on this host
pub fn is_uhid_available() -> bool {
    #[cfg(target_os = "linux")]
    {
        std::path::Path::new("/dev/uhid").exists()
    }
    #[cfg(not(target_os = "linux"))]
    {
        false
    }
}

/// Native Linux `/dev/uhid` Virtual DualSense backend
pub struct UhidDualSenseBackend {
    #[cfg(target_os = "linux")]
    file: Option<File>,
    active: bool,
    seq: u8,
    feedback_queue: VecDeque<VirtualFeedbackEvent>,
}

impl Default for UhidDualSenseBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl UhidDualSenseBackend {
    pub fn new() -> Self {
        Self {
            #[cfg(target_os = "linux")]
            file: None,
            active: false,
            seq: 0,
            feedback_queue: VecDeque::new(),
        }
    }

    /// Processes a raw UHID output payload (testable cross-platform)
    pub fn ingest_output_report(&mut self, data: &[u8]) {
        if let Some(ev) = decode_dualsense_usb_output_report(data) {
            self.feedback_queue.push_back(ev);
        }
    }
}

#[cfg(target_os = "linux")]
mod linux_uhid {
    pub const UHID_DESTROY: u32 = 1;
    pub const UHID_OUTPUT: u32 = 6;
    pub const UHID_GET_REPORT: u32 = 9;
    pub const UHID_GET_REPORT_REPLY: u32 = 10;
    pub const UHID_CREATE2: u32 = 11;
    pub const UHID_INPUT2: u32 = 12;
    pub const BUS_USB: u16 = 0x03;

    #[repr(C)]
    pub struct UhidCreate2Req {
        pub event_type: u32,
        pub name: [u8; 128],
        pub phys: [u8; 64],
        pub uniq: [u8; 64],
        pub rd_size: u16,
        pub bus: u16,
        pub vendor: u32,
        pub product: u32,
        pub version: u32,
        pub country: u32,
        pub rd_data: [u8; 4096],
    }

    #[repr(C)]
    pub struct UhidInput2Req {
        pub event_type: u32,
        pub size: u16,
        pub data: [u8; 4096],
    }

    #[repr(C)]
    pub struct UhidDestroyReq {
        pub event_type: u32,
    }

    #[repr(C)]
    pub struct UhidGetReportReplyReq {
        pub event_type: u32,
        pub id: u32,
        pub err: u16,
        pub size: u16,
        pub data: [u8; 4096],
    }
}

impl VirtualControllerBackend for UhidDualSenseBackend {
    fn backend_name(&self) -> &'static str {
        "linux_uhid_dualsense"
    }

    fn is_available(&self) -> bool {
        is_uhid_available()
    }

    fn plug(&mut self, mode: EmulationMode) -> Result<(), String> {
        if mode != EmulationMode::DualSense {
            return Err("UhidDualSenseBackend only supports EmulationMode::DualSense".to_string());
        }

        #[cfg(target_os = "linux")]
        {
            self.unplug();
            let mut file = OpenOptions::new()
                .read(true)
                .write(true)
                .custom_flags(0x800) // O_NONBLOCK
                .open("/dev/uhid")
                .map_err(|e| format!("Failed to open /dev/uhid: {}", e))?;

            let mut req: linux_uhid::UhidCreate2Req = unsafe { std::mem::zeroed() };
            req.event_type = linux_uhid::UHID_CREATE2;
            let name = b"DualSense Wireless Controller";
            req.name[..name.len()].copy_from_slice(name);
            let phys = b"dualsense-studio/virtual0";
            req.phys[..phys.len()].copy_from_slice(phys);
            let uniq = b"D5:50:05:4C:0C:E6";
            req.uniq[..uniq.len()].copy_from_slice(uniq);
            req.rd_size = DUALSENSE_USB_HID_DESCRIPTOR.len() as u16;
            req.bus = linux_uhid::BUS_USB;
            req.vendor = SONY_VENDOR_ID as u32;
            req.product = DUALSENSE_PID as u32;
            req.version = 0x0100;
            req.rd_data[..DUALSENSE_USB_HID_DESCRIPTOR.len()]
                .copy_from_slice(DUALSENSE_USB_HID_DESCRIPTOR);

            let bytes = unsafe {
                std::slice::from_raw_parts(
                    (&req as *const linux_uhid::UhidCreate2Req) as *const u8,
                    std::mem::size_of::<linux_uhid::UhidCreate2Req>(),
                )
            };
            file.write_all(bytes)
                .map_err(|e| format!("Failed to write UHID_CREATE2: {}", e))?;

            self.file = Some(file);
            self.active = true;
            tracing::info!("Linux UHID Virtual DualSense plugged via /dev/uhid");
            Ok(())
        }

        #[cfg(not(target_os = "linux"))]
        {
            Err("Linux /dev/uhid Virtual DualSense is only available on Linux".to_string())
        }
    }

    fn unplug(&mut self) {
        #[cfg(target_os = "linux")]
        if let Some(mut file) = self.file.take() {
            let req = linux_uhid::UhidDestroyReq {
                event_type: linux_uhid::UHID_DESTROY,
            };
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    (&req as *const linux_uhid::UhidDestroyReq) as *const u8,
                    std::mem::size_of::<linux_uhid::UhidDestroyReq>(),
                )
            };
            let _ = file.write_all(bytes);
        }
        self.active = false;
    }

    fn submit_input(&mut self, input: &DualSenseInputState, _gyro: &GyroSettings) {
        if !self.active {
            return;
        }
        let report = encode_dualsense_usb_input_report(input, self.seq);
        self.seq = self.seq.wrapping_add(1);

        #[cfg(target_os = "linux")]
        if let Some(file) = self.file.as_mut() {
            let mut req: linux_uhid::UhidInput2Req = unsafe { std::mem::zeroed() };
            req.event_type = linux_uhid::UHID_INPUT2;
            req.size = report.len() as u16;
            req.data[..report.len()].copy_from_slice(&report);
            let bytes = unsafe {
                std::slice::from_raw_parts(
                    (&req as *const linux_uhid::UhidInput2Req) as *const u8,
                    std::mem::size_of::<linux_uhid::UhidInput2Req>(),
                )
            };
            let _ = file.write_all(bytes);
        }
        #[cfg(not(target_os = "linux"))]
        let _ = report;
    }

    fn poll_feedback(&mut self) -> Option<VirtualFeedbackEvent> {
        #[cfg(target_os = "linux")]
        if let Some(file) = self.file.as_mut() {
            let mut raw = [0u8; 4380];
            while let Ok(n) = file.read(&mut raw) {
                if n < 4 {
                    break;
                }
                let ev_type = u32::from_ne_bytes([raw[0], raw[1], raw[2], raw[3]]);
                if ev_type == linux_uhid::UHID_OUTPUT && n >= 4 + 4096 + 2 {
                    let size = u16::from_ne_bytes([raw[4100], raw[4101]]) as usize;
                    let end = (4 + size).min(n);
                    let payload = raw[4..end].to_vec();
                    self.ingest_output_report(&payload);
                } else if ev_type == linux_uhid::UHID_GET_REPORT && n >= 10 {
                    let req_id = u32::from_ne_bytes([raw[4], raw[5], raw[6], raw[7]]);
                    let rnum = raw[8];
                    let mut reply: linux_uhid::UhidGetReportReplyReq =
                        unsafe { std::mem::zeroed() };
                    reply.event_type = linux_uhid::UHID_GET_REPORT_REPLY;
                    reply.id = req_id;
                    if let Some(feat) = build_dualsense_feature_report(rnum) {
                        reply.err = 0;
                        reply.size = feat.len() as u16;
                        reply.data[..feat.len()].copy_from_slice(&feat);
                    } else {
                        reply.err = 5; // EIO
                    }
                    let bytes = unsafe {
                        std::slice::from_raw_parts(
                            (&reply as *const linux_uhid::UhidGetReportReplyReq) as *const u8,
                            std::mem::size_of::<linux_uhid::UhidGetReportReplyReq>(),
                        )
                    };
                    let _ = file.write_all(bytes);
                }
            }
        }

        self.feedback_queue.pop_front()
    }
}

impl Drop for UhidDualSenseBackend {
    fn drop(&mut self) {
        self.unplug();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lightbar::RgbColor;
    use crate::protocol::OutputReportBuilder;

    #[test]
    fn test_feature_reports_and_output_ingestion() {
        let cal = build_dualsense_feature_report(0x05).unwrap();
        assert_eq!(cal.len(), 41);
        assert_eq!(cal[0], 0x05);

        let mac = build_dualsense_feature_report(0x09).unwrap();
        assert_eq!(mac.len(), 20);
        assert_eq!(mac[0], 0x09);

        let fw = build_dualsense_feature_report(0x20).unwrap();
        assert_eq!(fw.len(), 64);
        assert_eq!(fw[0], 0x20);

        let mut backend = UhidDualSenseBackend::new();
        let game_out = OutputReportBuilder::new()
            .with_lightbar(RgbColor::AMBER)
            .with_rumble(150, 75)
            .build_usb();
        backend.ingest_output_report(&game_out);

        let ev = backend.poll_feedback().unwrap();
        assert_eq!(ev.heavy_rumble, Some(150));
        assert_eq!(ev.light_rumble, Some(75));
        assert_eq!(ev.lightbar, Some(RgbColor::AMBER));
    }
}
