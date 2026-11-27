use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use byteorder::{BigEndian, ByteOrder};
use parking_lot::{Mutex, RwLock};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

use crate::device::{DUALSENSE_PID, SONY_VENDOR_ID};
use crate::protocol::DualSenseInputState;
use crate::uhid_dualsense::build_dualsense_feature_report;
use crate::virtual_device::{
    decode_dualsense_usb_output_report, encode_dualsense_usb_input_report, VirtualFeedbackEvent,
    DUALSENSE_USB_HID_DESCRIPTOR,
};

pub const DEFAULT_USBIP_PORT: u16 = 3240;
pub const USBIP_PROTOCOL_VERSION: u16 = 0x0111;
pub const OP_REQ_DEVLIST: u16 = 0x8005;
pub const OP_REP_DEVLIST: u16 = 0x0005;
pub const OP_REQ_IMPORT: u16 = 0x8003;
pub const OP_REP_IMPORT: u16 = 0x0003;
pub const USBIP_CMD_SUBMIT: u32 = 0x0000_0001;
pub const USBIP_CMD_UNLINK: u32 = 0x0000_0002;
pub const USBIP_RET_SUBMIT: u32 = 0x0000_0003;
pub const USBIP_RET_UNLINK: u32 = 0x0000_0004;

pub const VIRTUAL_BUS_ID: &str = "1-1";

/// USB 2.0 Device Descriptor for Sony DualSense Wireless Controller (054C:0CE6)
pub fn dualsense_usb_device_descriptor() -> [u8; 18] {
    [
        18,   // bLength
        0x01, // bDescriptorType (Device)
        0x00,
        0x02, // bcdUSB 2.00
        0x00,
        0x00,
        0x00, // Class/SubClass/Protocol (defined per interface)
        64,   // bMaxPacketSize0
        (SONY_VENDOR_ID & 0xFF) as u8,
        (SONY_VENDOR_ID >> 8) as u8, // idVendor 0x054C
        (DUALSENSE_PID & 0xFF) as u8,
        (DUALSENSE_PID >> 8) as u8, // idProduct 0x0CE6
        0x00,
        0x01, // bcdDevice 1.00
        0x01, // iManufacturer
        0x02, // iProduct
        0x00, // iSerialNumber
        0x01, // bNumConfigurations
    ]
}

/// USB Configuration + Interface (HID Gamepad, Interface 3, EP 0x84 IN / 0x03 OUT) Descriptor
pub fn dualsense_usb_config_descriptor() -> Vec<u8> {
    let hid_len = DUALSENSE_USB_HID_DESCRIPTOR.len() as u16;
    let total_len: u16 = 9 + 9 + 9 + 7 + 7; // 41 bytes
    vec![
        // Configuration Descriptor (9 bytes)
        9,
        0x02,
        (total_len & 0xFF) as u8,
        (total_len >> 8) as u8,
        1,    // bNumInterfaces
        1,    // bConfigurationValue
        0,    // iConfiguration
        0xC0, // bmAttributes (Self powered)
        250,  // bMaxPower (500 mA)
        // Interface Descriptor (9 bytes) — HID
        9,
        0x04,
        3,    // bInterfaceNumber (3 matches real DualSense HID interface)
        0,    // bAlternateSetting
        2,    // bNumEndpoints (IN + OUT)
        0x03, // bInterfaceClass (HID)
        0x00, // bInterfaceSubClass
        0x00, // bInterfaceProtocol
        0,    // iInterface
        // HID Descriptor (9 bytes)
        9,
        0x21,
        0x11,
        0x01, // bcdHID 1.11
        0x00, // bCountryCode
        1,    // bNumDescriptors
        0x22, // bDescriptorType (Report)
        (hid_len & 0xFF) as u8,
        (hid_len >> 8) as u8,
        // Endpoint Descriptor Interrupt IN (0x84)
        7,
        0x05,
        0x84,
        0x03, // Interrupt
        64,
        0x00, // wMaxPacketSize = 64
        4,    // bInterval (4ms = 250 Hz)
        // Endpoint Descriptor Interrupt OUT (0x03)
        7,
        0x05,
        0x03,
        0x03, // Interrupt
        64,
        0x00,
        4,
    ]
}

/// Builds a 312-byte USB/IP device metadata record describing our Virtual DualSense
pub fn build_usbip_device_record() -> [u8; 312] {
    let mut rec = [0u8; 312];
    let path = b"/sys/devices/virtual/dualsense-studio/1-1";
    rec[..path.len()].copy_from_slice(path);
    let busid = VIRTUAL_BUS_ID.as_bytes();
    rec[256..256 + busid.len()].copy_from_slice(busid);

    BigEndian::write_u32(&mut rec[288..292], 1); // busnum
    BigEndian::write_u32(&mut rec[292..296], 1); // devnum
    BigEndian::write_u32(&mut rec[296..300], 3); // USB_SPEED_HIGH
    BigEndian::write_u16(&mut rec[300..302], SONY_VENDOR_ID);
    BigEndian::write_u16(&mut rec[302..304], DUALSENSE_PID);
    BigEndian::write_u16(&mut rec[304..306], 0x0100); // bcdDevice
    rec[306] = 0x00; // bDeviceClass
    rec[307] = 0x00; // bDeviceSubClass
    rec[308] = 0x00; // bDeviceProtocol
    rec[309] = 1; // bConfigurationValue
    rec[310] = 1; // bNumConfigurations
    rec[311] = 1; // bNumInterfaces
    rec
}

/// Checks whether the open-source `usbip-win2` CLI / VHCI driver is installed on Windows
pub fn is_usbip_client_installed() -> bool {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        std::process::Command::new("usbip")
            .arg("version")
            .creation_flags(CREATE_NO_WINDOW)
            .output()
            .is_ok()
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::path::Path::new("/sys/devices/platform/vhci_hcd.0").exists()
    }
}

/// Open-source USB/IP v1.1.1 Virtual DualSense Server (compatible with `usbip-win2` on Windows
/// and `vhci-hcd` on Linux). Bridges live `DualSenseInputState` to virtual USB Interrupt IN
/// transfers and decodes game USB `0x02` Interrupt OUT transfers into `VirtualFeedbackEvent`s.
pub struct UsbipVirtualDualSenseServer {
    bind_addr: String,
    running: Arc<AtomicBool>,
    latest_input: Arc<RwLock<DualSenseInputState>>,
    feedback_queue: Arc<Mutex<VecDeque<VirtualFeedbackEvent>>>,
    bound_addr: Option<SocketAddr>,
    handle: Option<JoinHandle<()>>,
}

impl UsbipVirtualDualSenseServer {
    pub fn new(bind_addr: impl Into<String>) -> Self {
        Self {
            bind_addr: bind_addr.into(),
            running: Arc::new(AtomicBool::new(false)),
            latest_input: Arc::new(RwLock::new(DualSenseInputState::default())),
            feedback_queue: Arc::new(Mutex::new(VecDeque::new())),
            bound_addr: None,
            handle: None,
        }
    }

    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.bound_addr
    }

    pub fn is_running(&self) -> bool {
        self.running.load(Ordering::SeqCst)
    }

    pub fn update_input(&self, input: DualSenseInputState) {
        *self.latest_input.write() = input;
    }

    pub fn pop_feedback(&self) -> Option<VirtualFeedbackEvent> {
        self.feedback_queue.lock().pop_front()
    }

    pub async fn start(&mut self) -> std::io::Result<SocketAddr> {
        if self.running.load(Ordering::SeqCst) {
            if let Some(addr) = self.bound_addr {
                return Ok(addr);
            }
        }

        let listener = TcpListener::bind(&self.bind_addr).await?;
        let local = listener.local_addr()?;
        self.bound_addr = Some(local);
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let input_state = self.latest_input.clone();
        let feedback_q = self.feedback_queue.clone();

        let handle = tokio::spawn(async move {
            while running.load(Ordering::SeqCst) {
                match listener.accept().await {
                    Ok((stream, peer)) => {
                        debug!("USB/IP client connected from {}", peer);
                        let inp = input_state.clone();
                        let fb = feedback_q.clone();
                        tokio::spawn(async move {
                            if let Err(e) = handle_usbip_client(stream, inp, fb).await {
                                debug!("USB/IP session {} ended: {}", peer, e);
                            }
                        });
                    }
                    Err(e) => {
                        if running.load(Ordering::SeqCst) {
                            warn!("USB/IP accept error: {}", e);
                        }
                        break;
                    }
                }
            }
        });

        self.handle = Some(handle);
        info!("USB/IP Virtual DualSense server listening on {}", local);
        Ok(local)
    }

    pub fn stop(&mut self) {
        if self.running.swap(false, Ordering::SeqCst) {
            if let Some(h) = self.handle.take() {
                h.abort();
            }
            self.bound_addr = None;
            info!("USB/IP Virtual DualSense server stopped");
        }
    }
}

impl Drop for UsbipVirtualDualSenseServer {
    fn drop(&mut self) {
        self.stop();
    }
}

async fn handle_usbip_client(
    mut stream: TcpStream,
    latest_input: Arc<RwLock<DualSenseInputState>>,
    feedback_queue: Arc<Mutex<VecDeque<VirtualFeedbackEvent>>>,
) -> std::io::Result<()> {
    let mut op_hdr = [0u8; 8];
    stream.read_exact(&mut op_hdr).await?;
    let _version = BigEndian::read_u16(&op_hdr[0..2]);
    let code = BigEndian::read_u16(&op_hdr[2..4]);

    match code {
        OP_REQ_DEVLIST => {
            let mut resp = Vec::with_capacity(12 + 312 + 4);
            let mut hdr = [0u8; 12];
            BigEndian::write_u16(&mut hdr[0..2], USBIP_PROTOCOL_VERSION);
            BigEndian::write_u16(&mut hdr[2..4], OP_REP_DEVLIST);
            BigEndian::write_u32(&mut hdr[4..8], 0); // status OK
            BigEndian::write_u32(&mut hdr[8..12], 1); // 1 exported device
            resp.extend_from_slice(&hdr);
            resp.extend_from_slice(&build_usbip_device_record());
            resp.extend_from_slice(&[0x03, 0x00, 0x00, 0x00]); // HID interface record
            stream.write_all(&resp).await?;
            stream.flush().await?;
            Ok(())
        }
        OP_REQ_IMPORT => {
            let mut busid_buf = [0u8; 32];
            stream.read_exact(&mut busid_buf).await?;
            let busid = String::from_utf8_lossy(&busid_buf)
                .trim_end_matches('\0')
                .to_string();

            let mut hdr = [0u8; 8];
            BigEndian::write_u16(&mut hdr[0..2], USBIP_PROTOCOL_VERSION);
            BigEndian::write_u16(&mut hdr[2..4], OP_REP_IMPORT);

            if busid != VIRTUAL_BUS_ID {
                BigEndian::write_u32(&mut hdr[4..8], 1); // error
                stream.write_all(&hdr).await?;
                stream.flush().await?;
                return Ok(());
            }

            BigEndian::write_u32(&mut hdr[4..8], 0); // OK
            let mut resp = Vec::with_capacity(8 + 312);
            resp.extend_from_slice(&hdr);
            resp.extend_from_slice(&build_usbip_device_record());
            stream.write_all(&resp).await?;
            stream.flush().await?;

            // Enter USB/IP URB loop
            let mut seq_counter: u8 = 0;
            let mut urb_hdr = [0u8; 48];
            loop {
                if stream.read_exact(&mut urb_hdr).await.is_err() {
                    break;
                }
                let cmd = BigEndian::read_u32(&urb_hdr[0..4]);
                let seqnum = BigEndian::read_u32(&urb_hdr[4..8]);
                let direction = BigEndian::read_u32(&urb_hdr[12..16]); // 1 = IN, 0 = OUT
                let ep = BigEndian::read_u32(&urb_hdr[16..20]);
                let transfer_len = BigEndian::read_u32(&urb_hdr[24..28]) as usize;
                let setup = &urb_hdr[40..48];

                if cmd == USBIP_CMD_UNLINK {
                    let mut ret = [0u8; 48];
                    BigEndian::write_u32(&mut ret[0..4], USBIP_RET_UNLINK);
                    BigEndian::write_u32(&mut ret[4..8], seqnum);
                    stream.write_all(&ret).await?;
                    continue;
                }

                if cmd != USBIP_CMD_SUBMIT {
                    continue;
                }

                // Read OUT transfer payload if present
                let mut out_data = Vec::new();
                if direction == 0 && transfer_len > 0 && transfer_len <= 4096 {
                    out_data.resize(transfer_len, 0);
                    stream.read_exact(&mut out_data).await?;
                }

                let mut in_data = Vec::new();
                if ep == 0 {
                    // Control transfer
                    in_data = handle_usb_control(setup, transfer_len);
                } else if direction == 1 {
                    // Interrupt IN -> return 64-byte DualSense USB 0x01 input report
                    let snapshot = latest_input.read().clone();
                    let report = encode_dualsense_usb_input_report(&snapshot, seq_counter);
                    seq_counter = seq_counter.wrapping_add(1);
                    let take = transfer_len.min(report.len());
                    in_data.extend_from_slice(&report[..take]);
                } else if !out_data.is_empty() {
                    // Interrupt OUT -> game sent DualSense USB 0x02 output report (triggers/LED/rumble)
                    if let Some(ev) = decode_dualsense_usb_output_report(&out_data) {
                        feedback_queue.lock().push_back(ev);
                    }
                }

                let mut ret = [0u8; 48];
                BigEndian::write_u32(&mut ret[0..4], USBIP_RET_SUBMIT);
                BigEndian::write_u32(&mut ret[4..8], seqnum);
                BigEndian::write_u32(&mut ret[12..16], direction);
                BigEndian::write_u32(&mut ret[16..20], ep);
                BigEndian::write_u32(&mut ret[20..24], 0); // status = 0 (OK)
                let actual = if direction == 1 {
                    in_data.len()
                } else {
                    out_data.len()
                };
                BigEndian::write_u32(&mut ret[24..28], actual as u32);
                stream.write_all(&ret).await?;
                if direction == 1 && !in_data.is_empty() {
                    stream.write_all(&in_data).await?;
                }
                stream.flush().await?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn handle_usb_control(setup: &[u8], max_len: usize) -> Vec<u8> {
    if setup.len() < 8 {
        return Vec::new();
    }
    let bm_req_type = setup[0];
    let b_request = setup[1];
    let w_value = u16::from_le_bytes([setup[2], setup[3]]);

    // Standard GET_DESCRIPTOR (0x06)
    if (bm_req_type & 0x60) == 0x00 && b_request == 0x06 {
        let desc_type = (w_value >> 8) as u8;
        let desc_idx = (w_value & 0xFF) as u8;
        let data = match desc_type {
            0x01 => dualsense_usb_device_descriptor().to_vec(),
            0x02 => dualsense_usb_config_descriptor(),
            0x22 => DUALSENSE_USB_HID_DESCRIPTOR.to_vec(),
            0x03 => build_usb_string_descriptor(desc_idx),
            _ => Vec::new(),
        };
        return data.into_iter().take(max_len).collect();
    }

    // HID Class GET_REPORT (0x01) for Feature Reports 0x05, 0x09, 0x20
    if (bm_req_type & 0x60) == 0x20 && b_request == 0x01 {
        let report_id = (w_value & 0xFF) as u8;
        if let Some(feat) = build_dualsense_feature_report(report_id) {
            return feat.into_iter().take(max_len).collect();
        }
    }

    Vec::new()
}

fn build_usb_string_descriptor(idx: u8) -> Vec<u8> {
    if idx == 0 {
        return vec![4, 0x03, 0x09, 0x04]; // English (US)
    }
    let text = match idx {
        1 => "Sony Interactive Entertainment",
        _ => "DualSense Wireless Controller",
    };
    let utf16: Vec<u16> = text.encode_utf16().collect();
    let mut out = Vec::with_capacity(2 + utf16.len() * 2);
    out.push((2 + utf16.len() * 2) as u8);
    out.push(0x03);
    for ch in utf16 {
        out.extend_from_slice(&ch.to_le_bytes());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lightbar::RgbColor;
    use crate::protocol::OutputReportBuilder;
    use crate::triggers::{DualTriggerConfig, TriggerEffect};

    #[tokio::test]
    async fn test_usbip_virtual_dualsense_devlist_import_and_bidirectional_hid() {
        let mut server = UsbipVirtualDualSenseServer::new("127.0.0.1:0");
        let input = DualSenseInputState {
            cross: true,
            r2_trigger: 210,
            ..Default::default()
        };
        server.update_input(input);

        let addr = server.start().await.expect("start USB/IP server");
        assert!(server.is_running());

        // 1. OP_REQ_DEVLIST
        {
            let mut client = TcpStream::connect(addr).await.unwrap();
            let mut req = [0u8; 8];
            BigEndian::write_u16(&mut req[0..2], USBIP_PROTOCOL_VERSION);
            BigEndian::write_u16(&mut req[2..4], OP_REQ_DEVLIST);
            client.write_all(&req).await.unwrap();

            let mut rep_hdr = [0u8; 12];
            client.read_exact(&mut rep_hdr).await.unwrap();
            assert_eq!(BigEndian::read_u16(&rep_hdr[2..4]), OP_REP_DEVLIST);
            assert_eq!(BigEndian::read_u32(&rep_hdr[8..12]), 1);

            let mut dev = [0u8; 316];
            client.read_exact(&mut dev).await.unwrap();
            assert_eq!(BigEndian::read_u16(&dev[300..302]), SONY_VENDOR_ID);
            assert_eq!(BigEndian::read_u16(&dev[302..304]), DUALSENSE_PID);
        }

        // 2. OP_REQ_IMPORT + Interrupt IN (read DualSense input) + Interrupt OUT (send game trigger/RGB/rumble)
        {
            let mut client = TcpStream::connect(addr).await.unwrap();
            let mut req = [0u8; 40];
            BigEndian::write_u16(&mut req[0..2], USBIP_PROTOCOL_VERSION);
            BigEndian::write_u16(&mut req[2..4], OP_REQ_IMPORT);
            req[8..8 + VIRTUAL_BUS_ID.len()].copy_from_slice(VIRTUAL_BUS_ID.as_bytes());
            client.write_all(&req).await.unwrap();

            let mut rep = [0u8; 320];
            client.read_exact(&mut rep).await.unwrap();
            assert_eq!(BigEndian::read_u16(&rep[2..4]), OP_REP_IMPORT);
            assert_eq!(BigEndian::read_u32(&rep[4..8]), 0);

            // Submit Interrupt IN URB (ep 4, direction 1, len 64)
            let mut urb_in = [0u8; 48];
            BigEndian::write_u32(&mut urb_in[0..4], USBIP_CMD_SUBMIT);
            BigEndian::write_u32(&mut urb_in[4..8], 1);
            BigEndian::write_u32(&mut urb_in[12..16], 1); // IN
            BigEndian::write_u32(&mut urb_in[16..20], 4);
            BigEndian::write_u32(&mut urb_in[24..28], 64);
            client.write_all(&urb_in).await.unwrap();

            let mut ret_hdr = [0u8; 48];
            client.read_exact(&mut ret_hdr).await.unwrap();
            assert_eq!(BigEndian::read_u32(&ret_hdr[0..4]), USBIP_RET_SUBMIT);
            let mut in_report = [0u8; 64];
            client.read_exact(&mut in_report).await.unwrap();
            assert_eq!(in_report[0], 0x01);
            assert_eq!(in_report[6], 210); // R2 trigger
            assert_eq!(in_report[8] & 0x20, 0x20); // Cross

            // Submit Interrupt OUT URB with a game USB 0x02 Output Report
            let game_out = OutputReportBuilder::new()
                .with_triggers(DualTriggerConfig {
                    left: TriggerEffect::Off,
                    right: TriggerEffect::Weapon {
                        start_position: 2,
                        end_position: 7,
                        force: 8,
                    },
                })
                .with_lightbar(RgbColor::RED)
                .with_rumble(190, 80)
                .build_usb();

            let mut urb_out = [0u8; 48];
            BigEndian::write_u32(&mut urb_out[0..4], USBIP_CMD_SUBMIT);
            BigEndian::write_u32(&mut urb_out[4..8], 2);
            BigEndian::write_u32(&mut urb_out[12..16], 0); // OUT
            BigEndian::write_u32(&mut urb_out[16..20], 3);
            BigEndian::write_u32(&mut urb_out[24..28], game_out.len() as u32);
            client.write_all(&urb_out).await.unwrap();
            client.write_all(&game_out).await.unwrap();

            let mut out_ret = [0u8; 48];
            client.read_exact(&mut out_ret).await.unwrap();
            assert_eq!(BigEndian::read_u32(&out_ret[0..4]), USBIP_RET_SUBMIT);
        }

        let fb = server
            .pop_feedback()
            .expect("expected decoded game feedback");
        assert_eq!(fb.heavy_rumble, Some(190));
        assert_eq!(fb.light_rumble, Some(80));
        assert_eq!(fb.lightbar, Some(RgbColor::RED));
        assert_eq!(
            fb.triggers.unwrap().right,
            TriggerEffect::Weapon {
                start_position: 2,
                end_position: 7,
                force: 8,
            }
        );

        server.stop();
        assert!(!server.is_running());
    }
}
