use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use parking_lot::RwLock;
#[cfg_attr(not(target_os = "windows"), allow(unused_imports))]
use tracing::{debug, info, warn};

use crate::haptics_audio::{
    AudioToHapticsDsp, HapticsConfig, HapticsFrameQueue, HapticsMode, HOST_AUDIO_SAMPLE_RATE_HZ,
};

/// Converts arbitrary-channel, arbitrary-sample-rate interleaved `f32` PCM into 48 kHz stereo `f32`.
pub fn normalize_to_stereo_48k(samples: &[f32], channels: usize, src_rate: u32) -> Vec<f32> {
    if samples.is_empty() || channels == 0 || src_rate == 0 {
        return Vec::new();
    }

    let frame_count = samples.len() / channels;
    if frame_count == 0 {
        return Vec::new();
    }

    // Extract stereo frames first
    let mut stereo = Vec::with_capacity(frame_count * 2);
    for frame in samples.chunks_exact(channels) {
        let l = frame[0];
        let r = if channels >= 2 { frame[1] } else { frame[0] };
        stereo.push(l);
        stereo.push(r);
    }

    if src_rate == HOST_AUDIO_SAMPLE_RATE_HZ {
        return stereo;
    }

    // Linear interpolation resampler to 48 kHz
    let ratio = src_rate as f64 / HOST_AUDIO_SAMPLE_RATE_HZ as f64;
    let out_frames = ((frame_count as f64) / ratio).floor() as usize;
    let mut out = Vec::with_capacity(out_frames * 2);
    for i in 0..out_frames {
        let src_pos = i as f64 * ratio;
        let idx = (src_pos.floor() as usize).min(frame_count - 1);
        let next_idx = (idx + 1).min(frame_count - 1);
        let frac = (src_pos - idx as f64) as f32;
        let l = stereo[idx * 2] + (stereo[next_idx * 2] - stereo[idx * 2]) * frac;
        let r = stereo[idx * 2 + 1] + (stereo[next_idx * 2 + 1] - stereo[idx * 2 + 1]) * frac;
        out.push(l);
        out.push(r);
    }
    out
}

/// Background worker capturing OS loopback audio and feeding `AudioToHapticsDsp` -> `HapticsFrameQueue`.
pub struct AudioLoopbackWorker {
    running: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl AudioLoopbackWorker {
    pub fn start(
        haptics_cfg: Arc<RwLock<HapticsConfig>>,
        queue: Arc<HapticsFrameQueue>,
        levels: Arc<RwLock<(f32, f32)>>,
    ) -> Self {
        let running = Arc::new(AtomicBool::new(true));
        let flag = running.clone();

        let handle = std::thread::Builder::new()
            .name("dualsense-audio-haptics".to_string())
            .spawn(move || {
                info!("Starting Audio-to-Haptics loopback capture worker");
                #[cfg(target_os = "windows")]
                windows_wasapi::run_loopback_capture(flag, haptics_cfg, queue, levels);

                #[cfg(target_os = "linux")]
                linux_pulse::run_loopback_capture(flag, haptics_cfg, queue, levels);

                #[cfg(not(any(target_os = "windows", target_os = "linux")))]
                let _ = (flag, haptics_cfg, queue, levels);

                info!("Audio-to-Haptics loopback capture worker stopped");
            })
            .ok();

        Self { running, handle }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
    }
}

impl Drop for AudioLoopbackWorker {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(target_os = "linux")]
mod linux_pulse {
    use std::io::Read;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use parking_lot::RwLock;

    use super::*;

    pub fn run_loopback_capture(
        running: Arc<AtomicBool>,
        haptics_cfg: Arc<RwLock<HapticsConfig>>,
        queue: Arc<HapticsFrameQueue>,
        levels: Arc<RwLock<(f32, f32)>>,
    ) {
        let Ok(mut child) = Command::new("parec")
            .args([
                "--format=float32le",
                "--rate=48000",
                "--channels=2",
                "--latency-msec=20",
            ])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
        else {
            debug!("parec not available for Linux Audio-to-Haptics loopback");
            return;
        };

        let Some(mut stdout) = child.stdout.take() else {
            let _ = child.kill();
            return;
        };

        let mut dsp = AudioToHapticsDsp::new();
        let mut raw = [0u8; 512 * 2 * 4];
        while running.load(Ordering::Relaxed) {
            if stdout.read_exact(&mut raw).is_err() {
                break;
            }
            let cfg = haptics_cfg.read().clone();
            if !cfg.enabled || cfg.mode != HapticsMode::AudioToHaptics {
                continue;
            }
            let samples: Vec<f32> = raw
                .as_chunks::<4>()
                .0
                .iter()
                .map(|c| f32::from_le_bytes(*c))
                .collect();
            let blocks = dsp.process_stereo_48k(&samples, &cfg);
            if !blocks.is_empty() {
                queue.push_many(blocks);
                *levels.write() = dsp.levels();
            }
        }
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(target_os = "windows")]
mod windows_wasapi {
    use std::ffi::c_void;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    use parking_lot::RwLock;
    use tracing::warn;

    use super::*;

    type HRESULT = i32;

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct GUID {
        data1: u32,
        data2: u16,
        data3: u16,
        data4: [u8; 8],
    }

    #[repr(C)]
    struct WAVEFORMATEX {
        w_format_tag: u16,
        n_channels: u16,
        n_samples_per_sec: u32,
        n_avg_bytes_per_sec: u32,
        n_block_align: u16,
        w_bits_per_sample: u16,
        cb_size: u16,
    }

    const CLSCTX_ALL: u32 = 23;
    const COINIT_MULTITHREADED: u32 = 0x0;
    const AUDCLNT_SHAREMODE_SHARED: u32 = 0;
    const AUDCLNT_STREAMFLAGS_LOOPBACK: u32 = 0x0002_0000;
    const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
    const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

    const CLSID_MMDEVICE_ENUMERATOR: GUID = GUID {
        data1: 0xBCDE0395,
        data2: 0xE52F,
        data3: 0x467C,
        data4: [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E],
    };
    const IID_IMMDEVICE_ENUMERATOR: GUID = GUID {
        data1: 0xA95664D2,
        data2: 0x9614,
        data3: 0x4F35,
        data4: [0xA7, 0x46, 0xDE, 0x8D, 0xB6, 0x36, 0x17, 0xE6],
    };
    const IID_IAUDIO_CLIENT: GUID = GUID {
        data1: 0x1CB9AD4C,
        data2: 0xDBFA,
        data3: 0x4C32,
        data4: [0xB1, 0x78, 0xC2, 0xF5, 0x68, 0xA7, 0x03, 0xB2],
    };
    const IID_IAUDIO_CAPTURE_CLIENT: GUID = GUID {
        data1: 0xC8ADBD64,
        data2: 0xE71E,
        data3: 0x48A0,
        data4: [0xA4, 0xDE, 0x18, 0x5C, 0x39, 0x5C, 0xD3, 0x17],
    };

    #[link(name = "ole32")]
    extern "system" {
        fn CoInitializeEx(pv_reserved: *mut c_void, dw_co_init: u32) -> HRESULT;
        fn CoUninitialize();
        fn CoCreateInstance(
            rclsid: *const GUID,
            p_unk_outer: *mut c_void,
            dw_cls_context: u32,
            riid: *const GUID,
            ppv: *mut *mut c_void,
        ) -> HRESULT;
        fn CoTaskMemFree(pv: *mut c_void);
    }

    unsafe fn vtable_fn<T: Copy>(obj: *mut c_void, index: usize) -> T {
        let vtbl = *(obj as *const *const usize);
        std::mem::transmute_copy(&*vtbl.add(index))
    }

    unsafe fn release_com(obj: *mut c_void) {
        if !obj.is_null() {
            let release: unsafe extern "system" fn(*mut c_void) -> u32 = vtable_fn(obj, 2);
            release(obj);
        }
    }

    pub fn run_loopback_capture(
        running: Arc<AtomicBool>,
        haptics_cfg: Arc<RwLock<HapticsConfig>>,
        queue: Arc<HapticsFrameQueue>,
        levels: Arc<RwLock<(f32, f32)>>,
    ) {
        unsafe {
            let hr_init = CoInitializeEx(std::ptr::null_mut(), COINIT_MULTITHREADED);
            let need_uninit = hr_init >= 0;

            let mut enumerator: *mut c_void = std::ptr::null_mut();
            let hr = CoCreateInstance(
                &CLSID_MMDEVICE_ENUMERATOR,
                std::ptr::null_mut(),
                CLSCTX_ALL,
                &IID_IMMDEVICE_ENUMERATOR,
                &mut enumerator,
            );
            if hr < 0 || enumerator.is_null() {
                warn!("WASAPI MMDeviceEnumerator init failed: 0x{:08x}", hr);
                if need_uninit {
                    CoUninitialize();
                }
                return;
            }

            let get_default: unsafe extern "system" fn(
                *mut c_void,
                i32,
                i32,
                *mut *mut c_void,
            ) -> HRESULT = vtable_fn(enumerator, 4);
            let mut device: *mut c_void = std::ptr::null_mut();
            let hr = get_default(enumerator, 0, 0, &mut device);
            release_com(enumerator);
            if hr < 0 || device.is_null() {
                warn!("WASAPI GetDefaultAudioEndpoint failed: 0x{:08x}", hr);
                if need_uninit {
                    CoUninitialize();
                }
                return;
            }

            let activate: unsafe extern "system" fn(
                *mut c_void,
                *const GUID,
                u32,
                *mut c_void,
                *mut *mut c_void,
            ) -> HRESULT = vtable_fn(device, 3);
            let mut audio_client: *mut c_void = std::ptr::null_mut();
            let hr = activate(
                device,
                &IID_IAUDIO_CLIENT,
                CLSCTX_ALL,
                std::ptr::null_mut(),
                &mut audio_client,
            );
            release_com(device);
            if hr < 0 || audio_client.is_null() {
                warn!("WASAPI IAudioClient activate failed: 0x{:08x}", hr);
                if need_uninit {
                    CoUninitialize();
                }
                return;
            }

            let get_mix_format: unsafe extern "system" fn(
                *mut c_void,
                *mut *mut WAVEFORMATEX,
            ) -> HRESULT = vtable_fn(audio_client, 8);
            let mut fmt_ptr: *mut WAVEFORMATEX = std::ptr::null_mut();
            let hr = get_mix_format(audio_client, &mut fmt_ptr);
            if hr < 0 || fmt_ptr.is_null() {
                warn!("WASAPI GetMixFormat failed: 0x{:08x}", hr);
                release_com(audio_client);
                if need_uninit {
                    CoUninitialize();
                }
                return;
            }

            let channels = (*fmt_ptr).n_channels as usize;
            let sample_rate = (*fmt_ptr).n_samples_per_sec;
            let bits = (*fmt_ptr).w_bits_per_sample;
            let tag = (*fmt_ptr).w_format_tag;

            let initialize: unsafe extern "system" fn(
                *mut c_void,
                u32,
                u32,
                i64,
                i64,
                *const WAVEFORMATEX,
                *const GUID,
            ) -> HRESULT = vtable_fn(audio_client, 3);
            // 20 ms buffer (200_000 in 100ns units)
            let hr = initialize(
                audio_client,
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_LOOPBACK,
                200_000,
                0,
                fmt_ptr,
                std::ptr::null(),
            );
            CoTaskMemFree(fmt_ptr as *mut c_void);
            if hr < 0 {
                warn!(
                    "WASAPI IAudioClient::Initialize loopback failed: 0x{:08x}",
                    hr
                );
                release_com(audio_client);
                if need_uninit {
                    CoUninitialize();
                }
                return;
            }

            let get_service: unsafe extern "system" fn(
                *mut c_void,
                *const GUID,
                *mut *mut c_void,
            ) -> HRESULT = vtable_fn(audio_client, 14);
            let mut capture_client: *mut c_void = std::ptr::null_mut();
            let hr = get_service(
                audio_client,
                &IID_IAUDIO_CAPTURE_CLIENT,
                &mut capture_client,
            );
            if hr < 0 || capture_client.is_null() {
                warn!(
                    "WASAPI GetService(IAudioCaptureClient) failed: 0x{:08x}",
                    hr
                );
                release_com(audio_client);
                if need_uninit {
                    CoUninitialize();
                }
                return;
            }

            let start_fn: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                vtable_fn(audio_client, 10);
            let stop_fn: unsafe extern "system" fn(*mut c_void) -> HRESULT =
                vtable_fn(audio_client, 11);
            let get_next_packet_size: unsafe extern "system" fn(*mut c_void, *mut u32) -> HRESULT =
                vtable_fn(capture_client, 5);
            let get_buffer: unsafe extern "system" fn(
                *mut c_void,
                *mut *mut u8,
                *mut u32,
                *mut u32,
                *mut u64,
                *mut u64,
            ) -> HRESULT = vtable_fn(capture_client, 3);
            let release_buffer: unsafe extern "system" fn(*mut c_void, u32) -> HRESULT =
                vtable_fn(capture_client, 4);

            let _ = start_fn(audio_client);
            let mut dsp = AudioToHapticsDsp::new();
            let is_float =
                (tag == WAVE_FORMAT_IEEE_FLOAT || tag == WAVE_FORMAT_EXTENSIBLE) && bits == 32;

            while running.load(Ordering::Relaxed) {
                std::thread::sleep(std::time::Duration::from_millis(8));
                let mut pkt_len: u32 = 0;
                while get_next_packet_size(capture_client, &mut pkt_len) >= 0 && pkt_len > 0 {
                    let mut data_ptr: *mut u8 = std::ptr::null_mut();
                    let mut frames: u32 = 0;
                    let mut flags: u32 = 0;
                    if get_buffer(
                        capture_client,
                        &mut data_ptr,
                        &mut frames,
                        &mut flags,
                        std::ptr::null_mut(),
                        std::ptr::null_mut(),
                    ) < 0
                    {
                        break;
                    }

                    let cfg = haptics_cfg.read().clone();
                    if cfg.enabled
                        && cfg.mode == HapticsMode::AudioToHaptics
                        && !data_ptr.is_null()
                        && frames > 0
                    {
                        let sample_count = (frames as usize) * channels;
                        let pcm_f32: Vec<f32> = if is_float {
                            let slice =
                                std::slice::from_raw_parts(data_ptr as *const f32, sample_count);
                            slice.to_vec()
                        } else if bits == 16 {
                            let slice =
                                std::slice::from_raw_parts(data_ptr as *const i16, sample_count);
                            slice.iter().map(|&s| (s as f32) / 32768.0).collect()
                        } else {
                            Vec::new()
                        };

                        if !pcm_f32.is_empty() {
                            let stereo_48k =
                                normalize_to_stereo_48k(&pcm_f32, channels, sample_rate);
                            let blocks = dsp.process_stereo_48k(&stereo_48k, &cfg);
                            if !blocks.is_empty() {
                                queue.push_many(blocks);
                                *levels.write() = dsp.levels();
                            }
                        }
                    }

                    let _ = release_buffer(capture_client, frames);
                }
            }

            let _ = stop_fn(audio_client);
            release_com(capture_client);
            release_com(audio_client);
            if need_uninit {
                CoUninitialize();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_mono_44100_to_stereo_48k() {
        let mono_441: Vec<f32> = (0..441).map(|i| (i as f32 / 441.0).sin()).collect();
        let stereo_48k = normalize_to_stereo_48k(&mono_441, 1, 44100);
        // ~480 stereo frames = ~960 f32 samples
        assert!(stereo_48k.len() >= 950 && stereo_48k.len() <= 970);
        assert_eq!(stereo_48k[0], stereo_48k[1]);
    }
}
