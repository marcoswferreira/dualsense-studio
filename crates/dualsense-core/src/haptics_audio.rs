use std::collections::VecDeque;
use std::f32::consts::PI;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

/// DualSense Bluetooth HD Haptics Report 0x32 sample rate (3 kHz, 32 stereo frames per packet)
pub const BT_HAPTICS_SAMPLE_RATE_HZ: u32 = 3000;
pub const BT_HAPTICS_FRAMES_PER_PACKET: usize = 32;
pub const BT_HAPTICS_BYTES_PER_PACKET: usize = BT_HAPTICS_FRAMES_PER_PACKET * 2;
pub const HOST_AUDIO_SAMPLE_RATE_HZ: u32 = 48000;
pub const DECIMATION_FACTOR: usize =
    (HOST_AUDIO_SAMPLE_RATE_HZ / BT_HAPTICS_SAMPLE_RATE_HZ) as usize;

/// Operating mode for haptic feedback
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HapticsMode {
    #[default]
    LegacyRumble,
    BluetoothHdHaptics,
    AudioToHaptics,
    Off,
}

/// Configuration for Bluetooth HD Haptics (Report 0x32) and Audio-to-Haptics DSP
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HapticsConfig {
    pub mode: HapticsMode,
    pub enabled: bool,
    /// Output gain multiplier (0.0..=2.0)
    pub gain: f32,
    /// Low-pass filter cutoff frequency in Hz (40.0..=500.0)
    pub lowpass_cutoff_hz: f32,
    /// Left actuator balance (0.0..=1.0)
    pub left_balance: f32,
    /// Right actuator balance (0.0..=1.0)
    pub right_balance: f32,
    /// Selected host audio capture source name
    pub audio_source: String,
}

impl Default for HapticsConfig {
    fn default() -> Self {
        Self {
            mode: HapticsMode::LegacyRumble,
            enabled: false,
            gain: 1.0,
            lowpass_cutoff_hz: 180.0,
            left_balance: 1.0,
            right_balance: 1.0,
            audio_source: "default_loopback".to_string(),
        }
    }
}

/// Runtime telemetry for the Haptics & Audio engine
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct HapticsMetrics {
    pub active: bool,
    pub packets_sent: u64,
    pub left_level: f32,
    pub right_level: f32,
    pub sample_rate_hz: u32,
}

/// Real-time 48 kHz -> 3 kHz stereo low-pass filter, decimator, and S8 quantizer
/// producing 64-byte (32 stereo frames) blocks for DualSense Bluetooth Report 0x32.
#[derive(Debug, Clone)]
pub struct AudioToHapticsDsp {
    lp_left: f32,
    lp_right: f32,
    decim_accum_l: f32,
    decim_accum_r: f32,
    decim_count: usize,
    pending: Vec<i8>,
    last_left_rms: f32,
    last_right_rms: f32,
}

impl Default for AudioToHapticsDsp {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioToHapticsDsp {
    pub fn new() -> Self {
        Self {
            lp_left: 0.0,
            lp_right: 0.0,
            decim_accum_l: 0.0,
            decim_accum_r: 0.0,
            decim_count: 0,
            pending: Vec::with_capacity(BT_HAPTICS_BYTES_PER_PACKET * 2),
            last_left_rms: 0.0,
            last_right_rms: 0.0,
        }
    }

    pub fn levels(&self) -> (f32, f32) {
        (self.last_left_rms, self.last_right_rms)
    }

    pub fn reset(&mut self) {
        self.lp_left = 0.0;
        self.lp_right = 0.0;
        self.decim_accum_l = 0.0;
        self.decim_accum_r = 0.0;
        self.decim_count = 0;
        self.pending.clear();
        self.last_left_rms = 0.0;
        self.last_right_rms = 0.0;
    }

    /// Processes interleaved stereo 48 kHz `f32` PCM samples (`[L0, R0, L1, R1, ...]`)
    /// and emits zero or more 64-byte `S8` 3 kHz haptic blocks ready for Report `0x32`.
    pub fn process_stereo_48k(
        &mut self,
        interleaved_48k: &[f32],
        cfg: &HapticsConfig,
    ) -> Vec<[i8; BT_HAPTICS_BYTES_PER_PACKET]> {
        let cutoff = cfg.lowpass_cutoff_hz.clamp(40.0, 500.0);
        let rc = 1.0 / (2.0 * PI * cutoff);
        let dt = 1.0 / (HOST_AUDIO_SAMPLE_RATE_HZ as f32);
        let alpha = dt / (rc + dt);

        let gain = cfg.gain.clamp(0.0, 2.0);
        let bal_l = cfg.left_balance.clamp(0.0, 1.0);
        let bal_r = cfg.right_balance.clamp(0.0, 1.0);

        let mut packets = Vec::new();
        let mut sq_sum_l = 0.0f32;
        let mut sq_sum_r = 0.0f32;
        let mut frames_in = 0usize;

        for frame in interleaved_48k.as_chunks::<2>().0 {
            let in_l = frame[0].clamp(-1.0, 1.0);
            let in_r = frame[1].clamp(-1.0, 1.0);

            // First-order IIR low-pass filter to isolate tactile sub-bass/impact band
            self.lp_left += alpha * (in_l - self.lp_left);
            self.lp_right += alpha * (in_r - self.lp_right);

            self.decim_accum_l += self.lp_left;
            self.decim_accum_r += self.lp_right;
            self.decim_count += 1;

            if self.decim_count >= DECIMATION_FACTOR {
                let avg_l = (self.decim_accum_l / DECIMATION_FACTOR as f32) * gain * bal_l;
                let avg_r = (self.decim_accum_r / DECIMATION_FACTOR as f32) * gain * bal_r;
                self.decim_accum_l = 0.0;
                self.decim_accum_r = 0.0;
                self.decim_count = 0;

                sq_sum_l += avg_l * avg_l;
                sq_sum_r += avg_r * avg_r;
                frames_in += 1;

                let q_l = (avg_l * 127.0).round().clamp(-128.0, 127.0) as i8;
                let q_r = (avg_r * 127.0).round().clamp(-128.0, 127.0) as i8;
                self.pending.push(q_l);
                self.pending.push(q_r);

                if self.pending.len() >= BT_HAPTICS_BYTES_PER_PACKET {
                    let mut block = [0i8; BT_HAPTICS_BYTES_PER_PACKET];
                    block.copy_from_slice(&self.pending[..BT_HAPTICS_BYTES_PER_PACKET]);
                    self.pending.drain(..BT_HAPTICS_BYTES_PER_PACKET);
                    packets.push(block);
                }
            }
        }

        if frames_in > 0 {
            self.last_left_rms = (sq_sum_l / frames_in as f32).sqrt().clamp(0.0, 1.0);
            self.last_right_rms = (sq_sum_r / frames_in as f32).sqrt().clamp(0.0, 1.0);
        }

        packets
    }
}

/// Generates a synthetic sine-wave HD haptic pulse (3 kHz S8 blocks) for hardware testing over Bluetooth 0x32
pub fn generate_test_haptic_blocks(
    freq_hz: f32,
    amplitude: f32,
    duration_ms: u32,
) -> Vec<[i8; BT_HAPTICS_BYTES_PER_PACKET]> {
    let freq = freq_hz.clamp(25.0, 400.0);
    let amp = amplitude.clamp(0.0, 1.0);
    let total_frames = ((BT_HAPTICS_SAMPLE_RATE_HZ as u64 * duration_ms.min(2000) as u64) / 1000)
        .max(BT_HAPTICS_FRAMES_PER_PACKET as u64) as usize;
    let num_blocks = total_frames.div_ceil(BT_HAPTICS_FRAMES_PER_PACKET);

    let mut blocks = Vec::with_capacity(num_blocks);
    let mut sample_idx = 0usize;

    for _ in 0..num_blocks {
        let mut block = [0i8; BT_HAPTICS_BYTES_PER_PACKET];
        for f in 0..BT_HAPTICS_FRAMES_PER_PACKET {
            let t = sample_idx as f32 / BT_HAPTICS_SAMPLE_RATE_HZ as f32;
            // Smooth fade-out envelope on last 20% of pulse
            let progress = sample_idx as f32 / (num_blocks * BT_HAPTICS_FRAMES_PER_PACKET) as f32;
            let env = if progress > 0.8 {
                ((1.0 - progress) / 0.2).clamp(0.0, 1.0)
            } else {
                1.0
            };
            let v = ((2.0 * PI * freq * t).sin() * amp * env * 127.0)
                .round()
                .clamp(-128.0, 127.0) as i8;
            block[f * 2] = v;
            block[f * 2 + 1] = v;
            sample_idx += 1;
        }
        blocks.push(block);
    }

    blocks
}

/// Thread-safe bounded queue of 3 kHz S8 stereo haptic packets shared between audio DSP and HidDeviceWorker
#[derive(Debug)]
pub struct HapticsFrameQueue {
    inner: Mutex<VecDeque<[i8; BT_HAPTICS_BYTES_PER_PACKET]>>,
    max_depth: usize,
}

impl Default for HapticsFrameQueue {
    fn default() -> Self {
        Self::new(64)
    }
}

impl HapticsFrameQueue {
    pub fn new(max_depth: usize) -> Self {
        Self {
            inner: Mutex::new(VecDeque::with_capacity(max_depth)),
            max_depth: max_depth.max(4),
        }
    }

    pub fn push(&self, block: [i8; BT_HAPTICS_BYTES_PER_PACKET]) {
        let mut q = self.inner.lock();
        while q.len() >= self.max_depth {
            q.pop_front();
        }
        q.push_back(block);
    }

    pub fn push_many(&self, blocks: impl IntoIterator<Item = [i8; BT_HAPTICS_BYTES_PER_PACKET]>) {
        let mut q = self.inner.lock();
        for block in blocks {
            while q.len() >= self.max_depth {
                q.pop_front();
            }
            q.push_back(block);
        }
    }

    pub fn pop(&self) -> Option<[i8; BT_HAPTICS_BYTES_PER_PACKET]> {
        self.inner.lock().pop_front()
    }

    pub fn len(&self) -> usize {
        self.inner.lock().len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.lock().is_empty()
    }

    pub fn clear(&self) {
        self.inner.lock().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dsp_decimation_48k_to_3k_produces_exact_blocks() {
        let mut dsp = AudioToHapticsDsp::new();
        let cfg = HapticsConfig {
            mode: HapticsMode::AudioToHaptics,
            enabled: true,
            gain: 1.5,
            lowpass_cutoff_hz: 200.0,
            left_balance: 1.0,
            right_balance: 0.5,
            audio_source: "test".to_string(),
        };

        // 512 stereo frames at 48 kHz = 32 stereo frames at 3 kHz = exactly 1 Report 0x32 block
        let mut pcm = Vec::with_capacity(512 * 2);
        for i in 0..512 {
            let t = i as f32 / 48000.0;
            let sample = (2.0 * PI * 80.0 * t).sin();
            pcm.push(sample);
            pcm.push(sample);
        }

        let blocks = dsp.process_stereo_48k(&pcm, &cfg);
        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].iter().any(|&s| s != 0));

        let (lvl_l, lvl_r) = dsp.levels();
        assert!(lvl_l > lvl_r);
        assert!(lvl_l > 0.1);
    }

    #[test]
    fn test_generate_test_haptic_blocks_and_queue_bounds() {
        let blocks = generate_test_haptic_blocks(120.0, 0.8, 100);
        assert!(blocks.len() >= 9); // ~94 Hz -> ~10 blocks for 100ms
        assert!(blocks[0].iter().any(|&v| v.abs() > 20));

        let queue = HapticsFrameQueue::new(4);
        queue.push_many(blocks);
        assert_eq!(queue.len(), 4);
        assert!(queue.pop().is_some());
        assert_eq!(queue.len(), 3);
        queue.clear();
        assert!(queue.is_empty());
    }
}
