use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use byteorder::{ByteOrder, LittleEndian};
use crc32fast::Hasher;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tokio::net::UdpSocket;
use tokio::task::JoinHandle;

use crate::protocol::{ConnectionType, DualSenseInputState};

pub const DEFAULT_CEMUHOOK_UDP_PORT: u16 = 26760;
pub const DSU_PROTOCOL_VERSION: u16 = 1001;

pub const MSG_TYPE_VERSION: u32 = 0x0010_0000;
pub const MSG_TYPE_PORTS: u32 = 0x0010_0001;
pub const MSG_TYPE_DATA: u32 = 0x0010_0002;

/// Statistics for the Cemuhook / DSU Motion UDP Server
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CemuhookStats {
    pub running: bool,
    pub port: u16,
    pub active_clients: usize,
    pub packets_sent: u64,
    pub packets_received: u64,
}

impl Default for CemuhookStats {
    fn default() -> Self {
        Self {
            running: false,
            port: DEFAULT_CEMUHOOK_UDP_PORT,
            active_clients: 0,
            packets_sent: 0,
            packets_received: 0,
        }
    }
}

/// Computes and writes the DSU header + CRC32 checksum over a server response packet
pub fn finalize_dsu_packet(buf: &mut [u8], server_id: u32, msg_type: u32) {
    assert!(buf.len() >= 20, "DSU packet must be at least 20 bytes");
    buf[0..4].copy_from_slice(b"DSUS");
    LittleEndian::write_u16(&mut buf[4..6], DSU_PROTOCOL_VERSION);
    let payload_len = (buf.len() - 16) as u16;
    LittleEndian::write_u16(&mut buf[6..8], payload_len);
    LittleEndian::write_u32(&mut buf[8..12], 0); // Zero CRC field before hashing
    LittleEndian::write_u32(&mut buf[12..16], server_id);
    LittleEndian::write_u32(&mut buf[16..20], msg_type);

    let mut hasher = Hasher::new();
    hasher.update(buf);
    let crc = hasher.finalize();
    LittleEndian::write_u32(&mut buf[8..12], crc);
}

/// Validates a client DSU request (`DSUC`) and returns its `msg_type` and payload slice
pub fn parse_dsu_request(buf: &[u8]) -> Option<(u32, &[u8])> {
    if buf.len() < 20 || &buf[0..4] != b"DSUC" {
        return None;
    }
    let received_crc = LittleEndian::read_u32(&buf[8..12]);
    let mut copy = buf.to_vec();
    copy[8..12].copy_from_slice(&[0, 0, 0, 0]);
    let mut hasher = Hasher::new();
    hasher.update(&copy);
    if hasher.finalize() != received_crc {
        return None;
    }
    let msg_type = LittleEndian::read_u32(&buf[16..20]);
    Some((msg_type, &buf[20..]))
}

/// Builds a DSU `MSG_TYPE_VERSION` response (22 bytes)
pub fn build_version_response(server_id: u32) -> [u8; 22] {
    let mut buf = [0u8; 22];
    LittleEndian::write_u16(&mut buf[20..22], DSU_PROTOCOL_VERSION);
    finalize_dsu_packet(&mut buf, server_id, MSG_TYPE_VERSION);
    buf
}

fn dsu_battery_code(state: &DualSenseInputState, connected: bool) -> u8 {
    if !connected {
        return 0x00;
    }
    if state.battery.is_charging {
        return 0xEE;
    }
    if state.battery.is_full || state.battery.percentage >= 90 {
        return 0x05;
    }
    match state.battery.percentage {
        60..=89 => 0x04,
        30..=59 => 0x03,
        15..=29 => 0x02,
        _ => 0x01,
    }
}

/// Builds a DSU `MSG_TYPE_PORTS` slot-info response (32 bytes)
pub fn build_port_info_response(
    server_id: u32,
    slot: u8,
    connected: bool,
    state: &DualSenseInputState,
) -> [u8; 32] {
    let mut buf = [0u8; 32];
    buf[20] = slot;
    if slot == 0 && connected {
        buf[21] = 2; // Connected
        buf[22] = 2; // Full Gyro model
        buf[23] = match state.connection {
            ConnectionType::Usb => 1,
            ConnectionType::Bluetooth => 2,
        };
        buf[24..30].copy_from_slice(&[0xD5, 0x50, 0x05, 0x4C, 0x0C, 0xE6]);
        buf[30] = dsu_battery_code(state, true);
    } else {
        buf[21] = 0; // Disconnected
    }
    buf[31] = 0; // Terminator
    finalize_dsu_packet(&mut buf, server_id, MSG_TYPE_PORTS);
    buf
}

/// Builds a DSU `MSG_TYPE_DATA` (100-byte) motion + controller telemetry packet
pub fn build_pad_data_response(
    server_id: u32,
    slot: u8,
    packet_num: u32,
    timestamp_us: u64,
    connected: bool,
    state: &DualSenseInputState,
) -> [u8; 100] {
    let mut buf = [0u8; 100];
    // Shared slot header (bytes 20..31)
    buf[20] = slot;
    buf[21] = if slot == 0 && connected { 2 } else { 0 };
    buf[22] = 2; // Full Gyro
    buf[23] = match state.connection {
        ConnectionType::Usb => 1,
        ConnectionType::Bluetooth => 2,
    };
    buf[24..30].copy_from_slice(&[0xD5, 0x50, 0x05, 0x4C, 0x0C, 0xE6]);
    buf[30] = dsu_battery_code(state, connected);
    buf[31] = u8::from(connected);

    LittleEndian::write_u32(&mut buf[32..36], packet_num);

    // Digital buttons bitmask 1 (byte 36)
    let mut b1 = 0u8;
    if state.create {
        b1 |= 0x01;
    }
    if state.l3 {
        b1 |= 0x02;
    }
    if state.r3 {
        b1 |= 0x04;
    }
    if state.options {
        b1 |= 0x08;
    }
    if state.dpad_up {
        b1 |= 0x10;
    }
    if state.dpad_right {
        b1 |= 0x20;
    }
    if state.dpad_down {
        b1 |= 0x40;
    }
    if state.dpad_left {
        b1 |= 0x80;
    }
    buf[36] = b1;

    // Digital buttons bitmask 2 (byte 37)
    let mut b2 = 0u8;
    if state.l2_click {
        b2 |= 0x01;
    }
    if state.r2_click {
        b2 |= 0x02;
    }
    if state.l1 {
        b2 |= 0x04;
    }
    if state.r1 {
        b2 |= 0x08;
    }
    if state.triangle {
        b2 |= 0x10;
    }
    if state.circle {
        b2 |= 0x20;
    }
    if state.cross {
        b2 |= 0x40;
    }
    if state.square {
        b2 |= 0x80;
    }
    buf[37] = b2;

    buf[38] = u8::from(state.ps_button);
    buf[39] = u8::from(state.touchpad_click);

    // Analog sticks (DSU Y is inverted: 255 = up, 0 = down)
    buf[40] = state.left_stick_x;
    buf[41] = 255u8.saturating_sub(state.left_stick_y);
    buf[42] = state.right_stick_x;
    buf[43] = 255u8.saturating_sub(state.right_stick_y);

    // Analog triggers
    buf[54] = state.l2_trigger;
    buf[55] = state.r2_trigger;

    // Touchpad 1 (bytes 56..62)
    buf[56] = u8::from(state.touch1.active);
    buf[57] = state.touch1.id;
    LittleEndian::write_u16(&mut buf[58..60], state.touch1.x);
    LittleEndian::write_u16(&mut buf[60..62], state.touch1.y);

    // Touchpad 2 (bytes 62..68)
    buf[62] = u8::from(state.touch2.active);
    buf[63] = state.touch2.id;
    LittleEndian::write_u16(&mut buf[64..66], state.touch2.x);
    LittleEndian::write_u16(&mut buf[66..68], state.touch2.y);

    // Motion timestamp (microseconds)
    LittleEndian::write_u64(&mut buf[68..76], timestamp_us);

    // DualSense IMU scaling: Accel ~8192 LSB/G, Gyro ~16.384 LSB/(deg/s) -> ~0.061 deg/s per LSB
    const ACCEL_SCALE: f32 = 1.0 / 8192.0;
    const GYRO_SCALE: f32 = 2000.0 / 32768.0;

    LittleEndian::write_f32(&mut buf[76..80], state.motion.accel_x as f32 * ACCEL_SCALE);
    LittleEndian::write_f32(&mut buf[80..84], state.motion.accel_y as f32 * ACCEL_SCALE);
    LittleEndian::write_f32(&mut buf[84..88], state.motion.accel_z as f32 * ACCEL_SCALE);

    LittleEndian::write_f32(&mut buf[88..92], state.motion.gyro_x as f32 * GYRO_SCALE);
    LittleEndian::write_f32(&mut buf[92..96], state.motion.gyro_y as f32 * GYRO_SCALE);
    LittleEndian::write_f32(&mut buf[96..100], state.motion.gyro_z as f32 * GYRO_SCALE);

    finalize_dsu_packet(&mut buf, server_id, MSG_TYPE_DATA);
    buf
}

/// Live Cemuhook / DSU Motion UDP Server
pub struct CemuhookUdpServer {
    port: u16,
    bound_port: Option<u16>,
    server_id: u32,
    running: Arc<AtomicBool>,
    packets_sent: Arc<AtomicU64>,
    packets_received: Arc<AtomicU64>,
    packet_counter: Arc<AtomicU32>,
    subscribers: Arc<RwLock<HashMap<SocketAddr, Instant>>>,
    latest_state: Arc<RwLock<(bool, DualSenseInputState)>>,
    worker_handle: Option<JoinHandle<()>>,
}

impl CemuhookUdpServer {
    pub fn new(port: u16) -> Self {
        Self {
            port,
            bound_port: None,
            server_id: 0x4453_5354, // "DSST"
            running: Arc::new(AtomicBool::new(false)),
            packets_sent: Arc::new(AtomicU64::new(0)),
            packets_received: Arc::new(AtomicU64::new(0)),
            packet_counter: Arc::new(AtomicU32::new(0)),
            subscribers: Arc::new(RwLock::new(HashMap::new())),
            latest_state: Arc::new(RwLock::new((false, DualSenseInputState::default()))),
            worker_handle: None,
        }
    }

    pub fn update_state(&self, connected: bool, state: DualSenseInputState) {
        *self.latest_state.write() = (connected, state);
    }

    pub fn stats(&self) -> CemuhookStats {
        let now = Instant::now();
        let active_clients = self
            .subscribers
            .read()
            .values()
            .filter(|t| now.duration_since(**t) < Duration::from_secs(5))
            .count();
        CemuhookStats {
            running: self.running.load(Ordering::SeqCst),
            port: self.bound_port.unwrap_or(self.port),
            active_clients,
            packets_sent: self.packets_sent.load(Ordering::SeqCst),
            packets_received: self.packets_received.load(Ordering::SeqCst),
        }
    }

    pub async fn start(&mut self) -> Result<(), std::io::Error> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        let socket = Arc::new(UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0], self.port))).await?);
        let actual_port = socket.local_addr()?.port();
        self.bound_port = Some(actual_port);
        self.running.store(true, Ordering::SeqCst);

        let running = self.running.clone();
        let server_id = self.server_id;
        let sent = self.packets_sent.clone();
        let recv = self.packets_received.clone();
        let counter = self.packet_counter.clone();
        let subs = self.subscribers.clone();
        let latest = self.latest_state.clone();

        let handle = tokio::spawn(async move {
            let start_time = Instant::now();
            let mut buf = [0u8; 512];
            let mut tick = tokio::time::interval(Duration::from_millis(10)); // 100 Hz motion feed

            while running.load(Ordering::SeqCst) {
                tokio::select! {
                    res = socket.recv_from(&mut buf) => {
                        if let Ok((len, peer)) = res {
                            recv.fetch_add(1, Ordering::SeqCst);
                            if let Some((msg_type, payload)) = parse_dsu_request(&buf[..len]) {
                                match msg_type {
                                    MSG_TYPE_VERSION => {
                                        let resp = build_version_response(server_id);
                                        if socket.send_to(&resp, peer).await.is_ok() {
                                            sent.fetch_add(1, Ordering::SeqCst);
                                        }
                                    }
                                    MSG_TYPE_PORTS => {
                                        let count = if payload.len() >= 4 {
                                            LittleEndian::read_i32(&payload[0..4]).clamp(0, 4) as usize
                                        } else {
                                            0
                                        };
                                        let (connected, state) = latest.read().clone();
                                        for i in 0..count {
                                            if let Some(&slot) = payload.get(4 + i) {
                                                let resp = build_port_info_response(server_id, slot, connected, &state);
                                                if socket.send_to(&resp, peer).await.is_ok() {
                                                    sent.fetch_add(1, Ordering::SeqCst);
                                                }
                                            }
                                        }
                                    }
                                    MSG_TYPE_DATA => {
                                        subs.write().insert(peer, Instant::now());
                                        let (connected, state) = latest.read().clone();
                                        let pkt_num = counter.fetch_add(1, Ordering::SeqCst);
                                        let ts = start_time.elapsed().as_micros() as u64;
                                        let resp = build_pad_data_response(server_id, 0, pkt_num, ts, connected, &state);
                                        if socket.send_to(&resp, peer).await.is_ok() {
                                            sent.fetch_add(1, Ordering::SeqCst);
                                        }
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                    _ = tick.tick() => {
                        let now = Instant::now();
                        let active_peers: Vec<SocketAddr> = {
                            let mut map = subs.write();
                            map.retain(|_, last| now.duration_since(*last) < Duration::from_secs(5));
                            map.keys().copied().collect()
                        };
                        if !active_peers.is_empty() {
                            let (connected, state) = latest.read().clone();
                            let pkt_num = counter.fetch_add(1, Ordering::SeqCst);
                            let ts = start_time.elapsed().as_micros() as u64;
                            let resp = build_pad_data_response(server_id, 0, pkt_num, ts, connected, &state);
                            for peer in active_peers {
                                if socket.send_to(&resp, peer).await.is_ok() {
                                    sent.fetch_add(1, Ordering::SeqCst);
                                }
                            }
                        }
                    }
                }
            }
        });

        self.worker_handle = Some(handle);
        tracing::info!("Cemuhook Motion UDP server started on port {}", actual_port);
        Ok(())
    }

    pub fn stop(&mut self) {
        if self.running.swap(false, Ordering::SeqCst) {
            if let Some(handle) = self.worker_handle.take() {
                handle.abort();
            }
            self.bound_port = None;
            self.subscribers.write().clear();
            tracing::info!("Cemuhook Motion UDP server stopped");
        }
    }
}

impl Drop for CemuhookUdpServer {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn build_client_request(msg_type: u32, payload: &[u8]) -> Vec<u8> {
        let mut buf = vec![0u8; 20 + payload.len()];
        buf[0..4].copy_from_slice(b"DSUC");
        LittleEndian::write_u16(&mut buf[4..6], DSU_PROTOCOL_VERSION);
        LittleEndian::write_u16(&mut buf[6..8], (4 + payload.len()) as u16);
        LittleEndian::write_u32(&mut buf[12..16], 1);
        LittleEndian::write_u32(&mut buf[16..20], msg_type);
        buf[20..].copy_from_slice(payload);
        let mut hasher = Hasher::new();
        hasher.update(&buf);
        let crc = hasher.finalize();
        LittleEndian::write_u32(&mut buf[8..12], crc);
        buf
    }

    #[test]
    fn test_dsu_packet_encoding_and_crc() {
        let ver = build_version_response(42);
        assert_eq!(&ver[0..4], b"DSUS");
        assert_eq!(LittleEndian::read_u16(&ver[20..22]), DSU_PROTOCOL_VERSION);

        let mut state = DualSenseInputState {
            cross: true,
            ..Default::default()
        };
        state.motion.accel_z = 8192; // 1.0 G
        state.motion.gyro_x = 16384; // 1000 deg/s

        let pad = build_pad_data_response(42, 0, 7, 123_456, true, &state);
        assert_eq!(&pad[0..4], b"DSUS");
        assert_eq!(pad[37] & 0x40, 0x40); // Cross
        assert!((LittleEndian::read_f32(&pad[84..88]) - 1.0).abs() < 1e-4);
        assert!((LittleEndian::read_f32(&pad[88..92]) - 1000.0).abs() < 1e-2);
    }

    #[tokio::test]
    async fn test_cemuhook_udp_server_roundtrip() {
        let mut server = CemuhookUdpServer::new(0);
        let mut state = DualSenseInputState::default();
        state.battery.percentage = 85;
        state.motion.gyro_y = 3276;
        server.update_state(true, state);
        server.start().await.expect("start cemuhook server");
        let port = server.stats().port;
        assert_ne!(port, 0);

        let client = UdpSocket::bind("127.0.0.1:0").await.unwrap();
        let target = SocketAddr::from(([127, 0, 0, 1], port));

        let req = build_client_request(MSG_TYPE_VERSION, &[]);
        client.send_to(&req, target).await.unwrap();

        let mut resp = [0u8; 128];
        let (n, _) = tokio::time::timeout(Duration::from_secs(2), client.recv_from(&mut resp))
            .await
            .expect("timeout waiting for DSU version")
            .unwrap();
        assert_eq!(n, 22);
        assert_eq!(&resp[0..4], b"DSUS");
        assert_eq!(LittleEndian::read_u32(&resp[16..20]), MSG_TYPE_VERSION);

        server.stop();
        assert!(!server.stats().running);
    }
}
