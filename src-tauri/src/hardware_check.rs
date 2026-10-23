// Hardware check for the Features tab: reads what the machine has (processor, memory,
// graphics) and measures the two heavy jobs directly, so Glucose can say which features
// will run well here instead of guessing from specs:
// - live subtitles: Whisper with the model live mode would use, on 20 s of speech (the
//   size of one live chunk), with live mode's own settings;
// - smart subtitle position: one face detection on a video-sized frame.

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use serde::Serialize;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext};

use super::live_subtitles::{self, MAX_THREADS};

// 20 s of public-domain English speech, 16 kHz mono 16-bit PCM (see assets/hardware_check).
const SPEECH_SAMPLE: &[u8] = include_bytes!("../assets/hardware_check/speech_sample_16k_mono.pcm");
const SAMPLE_RATE: f64 = 16_000.0;

#[derive(Debug, Default, Serialize)]
pub struct HardwareReport {
    pub cpu_name: Option<String>,
    // Logical processors.
    pub threads: usize,
    pub ram_gb: Option<f64>,
    pub gpu_name: Option<String>,
    // Model the speed test used (the one live mode would use); None without a model.
    pub live_model: Option<String>,
    // How many times faster than real time Whisper transcribed the sample.
    pub live_speed_x: Option<f64>,
    // One face detection, in milliseconds.
    pub face_ms: Option<f64>,
    // Parts of the check that failed, in plain words.
    pub problems: Vec<String>,
}

// Set by cancel_hardware_check; stops the speech test, which is most of the check.
static CANCEL: AtomicBool = AtomicBool::new(false);

/// Runs the full check. Takes from a few seconds to about a minute on slow machines,
/// mostly the Whisper test, so it runs on a background thread. Fails with "cancelled"
/// when cancel_hardware_check stopped it.
#[tauri::command]
pub async fn run_hardware_check() -> Result<HardwareReport, String> {
    // A cancel that arrived after the previous check had finished does not stop this one.
    CANCEL.store(false, Ordering::Relaxed);
    let report = tauri::async_runtime::spawn_blocking(check)
        .await
        .map_err(|e| e.to_string());
    if CANCEL.swap(false, Ordering::Relaxed) {
        return Err("cancelled".into());
    }
    report
}

/// Stops a running check, for example because a video was opened: the speech test keeps
/// the processor busy and would make it stutter.
#[tauri::command]
pub fn cancel_hardware_check() {
    CANCEL.store(true, Ordering::Relaxed);
}

fn check() -> HardwareReport {
    let threads = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1);
    let mut report = HardwareReport {
        cpu_name: system::cpu_name(),
        threads,
        ram_gb: system::ram_gb(),
        gpu_name: system::gpu_name(),
        ..Default::default()
    };

    match live_subtitles::find_live_model() {
        // Both would compete for the processor: the result would understate this machine
        // and slow the subtitles down.
        Some((label, _)) if live_subtitles::session_running() => {
            report.live_model = Some(label.to_string());
            report.problems.push(
                "Live subtitles are running, so speech speed was not tested. Turn them off and check again."
                    .into(),
            );
        }
        Some((label, path)) => {
            report.live_model = Some(label.to_string());
            match whisper_speed(label, &path, threads.min(MAX_THREADS)) {
                Ok(speed) => report.live_speed_x = Some(speed),
                Err(e) => report.problems.push(e),
            }
        }
        None => report
            .problems
            .push("No speech model for live subtitles is installed, so speech speed was not tested.".into()),
    }

    match super::face_detect::time_detection() {
        Ok(ms) => report.face_ms = Some(ms),
        Err(e) => report.problems.push(e),
    }
    report
}

fn speech_samples() -> Vec<f32> {
    SPEECH_SAMPLE
        .chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect()
}

// Transcribes the sample the way live mode transcribes a chunk and returns audio seconds
// per second of work. Model loading is not counted: live mode loads it once per session.
// Runs twice and keeps the faster run, so a short burst of work from another program does
// not decide the result. Uses live mode's copy of the model when it has one loaded.
fn whisper_speed(label: &str, path: &str, threads: usize) -> Result<f64, String> {
    let ctx = live_subtitles::load_shared_model(path)
        .map_err(|e| format!("Could not load the speech model for the speed test: {e}"))?;
    let samples = speech_samples();
    let mut best = f64::MAX;
    for _ in 0..2 {
        if CANCEL.load(Ordering::Relaxed) {
            return Err("The check was cancelled.".into());
        }
        best = best.min(transcribe_once(&ctx, &samples, threads)?);
    }
    if cfg!(debug_assertions) {
        eprintln!("[hardware check] {label}, {threads} threads: {best:.2} s for 20 s of speech");
    }
    Ok(samples.len() as f64 / SAMPLE_RATE / best.max(1e-3))
}

// Seconds one transcription of the sample takes, with live mode's settings.
fn transcribe_once(ctx: &WhisperContext, samples: &[f32], threads: usize) -> Result<f64, String> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_translate(false);
    params.set_language(Some("en"));
    params.set_n_threads(threads as i32);
    params.set_no_context(true);
    super::subtitle_format::request_word_timestamps(&mut params);
    params.set_suppress_blank(true);
    params.set_abort_callback_safe(|| CANCEL.load(Ordering::Relaxed));

    let mut state = ctx.create_state().map_err(|e| format!("Speed test failed: {e}"))?;
    let started = Instant::now();
    state.full(params, samples).map_err(|e| format!("Speed test failed: {e}"))?;
    Ok(started.elapsed().as_secs_f64())
}

#[cfg(target_os = "windows")]
mod system {
    use windows::core::w;
    use windows::Win32::Graphics::Dxgi::{CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE};
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};

    pub fn cpu_name() -> Option<String> {
        let mut buf = [0u16; 256];
        let mut size = (buf.len() * 2) as u32;
        // SAFETY: the buffer and its size in bytes are passed together.
        let status = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                w!("HARDWARE\\DESCRIPTION\\System\\CentralProcessor\\0"),
                w!("ProcessorNameString"),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut size),
            )
        };
        if status.is_err() {
            return None;
        }
        let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
        let name = String::from_utf16_lossy(&buf[..len]).trim().to_string();
        (!name.is_empty()).then_some(name)
    }

    pub fn ram_gb() -> Option<f64> {
        let mut status = MEMORYSTATUSEX { dwLength: size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
        // SAFETY: dwLength is set as the API requires.
        unsafe { GlobalMemoryStatusEx(&mut status) }.ok()?;
        Some(status.ullTotalPhys as f64 / (1024.0 * 1024.0 * 1024.0))
    }

    // The graphics card with the most dedicated memory, skipping Windows' software adapter.
    pub fn gpu_name() -> Option<String> {
        // SAFETY: plain DXGI enumeration; every returned interface is owned.
        unsafe {
            let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
            let mut best: Option<(usize, String)> = None;
            let mut i = 0;
            while let Ok(adapter) = factory.EnumAdapters1(i) {
                i += 1;
                let Ok(desc) = adapter.GetDesc1() else { continue };
                if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                    continue;
                }
                let len = desc.Description.iter().position(|&c| c == 0).unwrap_or(desc.Description.len());
                let name = String::from_utf16_lossy(&desc.Description[..len]).trim().to_string();
                if best.as_ref().is_none_or(|(mem, _)| desc.DedicatedVideoMemory > *mem) {
                    best = Some((desc.DedicatedVideoMemory, name));
                }
            }
            best.map(|(_, name)| name)
        }
    }
}

#[cfg(not(target_os = "windows"))]
mod system {
    pub fn cpu_name() -> Option<String> {
        None
    }
    pub fn ram_gb() -> Option<f64> {
        None
    }
    pub fn gpu_name() -> Option<String> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn speech_sample_is_twenty_seconds() {
        let secs = speech_samples().len() as f64 / SAMPLE_RATE;
        assert!((secs - 20.0).abs() < 0.01, "{secs}");
    }

    // Runs the whole check, including the Whisper test (needs a model installed); run with
    // `cargo test --lib full_check -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn full_check() {
        println!("{:#?}", check());
    }

    #[test]
    #[cfg(target_os = "windows")]
    fn reads_system_details() {
        let (cpu, ram, gpu) = (system::cpu_name(), system::ram_gb(), system::gpu_name());
        println!("cpu {cpu:?}, ram {ram:?} GB, gpu {gpu:?}");
        assert!(cpu.is_some_and(|c| !c.is_empty()));
        assert!(ram.is_some_and(|r| r > 0.5));
    }
}
