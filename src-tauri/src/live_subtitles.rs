// Live subtitles: transcribes audio ahead of the playhead in chunks and streams the
// resulting cues to the frontend, so subtitles appear while the video plays instead of
// after a full batch run. See docs/spikes/live-subtitles.md.

use crate::subtitle_format;
use serde::Serialize;
use std::ffi::c_void;
use std::io::Read;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use tauri::Emitter;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const SAMPLE_RATE: usize = 16_000;
// Whisper pads every input to 30 s, so shorter windows cost about the same and only add
// boundaries. 20 s leaves room to move the cut back to a pause.
const WINDOW_SECS: usize = 20;
// The cut is moved to the quietest point in the last part of the window, so chunk
// boundaries fall between words rather than through them.
const CUT_SEARCH_SECS: usize = 3;
const ENERGY_FRAME: usize = SAMPLE_RATE / 20; // 50 ms
// Stop working this far ahead of the playhead and resume when it catches up.
const MAX_LEAD_SECS: f64 = 180.0;
// Thread count beyond this did not speed up transcription in the spike benchmark.
const MAX_THREADS: usize = 8;

// Preferred models for live mode, fastest acceptable first. small-q5_1 matches small's
// speed with a third of the size; the rest are fallbacks for whatever is installed.
const LIVE_MODELS: &[(&str, &str)] = &[
    ("ggml-small-q5_1.bin", "small-q5_1"),
    ("ggml-small.bin", "small"),
    ("ggml-base.bin", "base"),
    ("ggml-tiny.bin", "tiny"),
];

// Label of the model live mode would use right now, if any is installed.
pub(crate) fn preferred_model() -> Option<&'static str> {
    LIVE_MODELS
        .iter()
        .find(|(file, _)| super::find_model_path(file).is_some())
        .map(|(_, label)| *label)
}

// Position in the preference order; lower is better. None for unknown labels.
pub(crate) fn model_rank(label: &str) -> Option<usize> {
    LIVE_MODELS.iter().position(|(_, l)| *l == label)
}

struct Session {
    id: u64,
    cancel: Arc<AtomicBool>,
    ffmpeg: Arc<Mutex<Option<Child>>>,
}

#[derive(Default)]
struct LiveState {
    next_id: u64,
    session: Option<Session>,
    // Kept across sessions so seeking does not reload the model; dropped on unload.
    model: Option<(String, Arc<WhisperContext>)>,
}

fn state() -> &'static Mutex<LiveState> {
    static STATE: OnceLock<Mutex<LiveState>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}

// Playhead position in seconds (f64 bits), reported by the frontend for backpressure.
static PLAYHEAD: AtomicU64 = AtomicU64::new(0);

#[derive(Clone, Serialize)]
struct LiveCue {
    start: f64,
    end: f64,
    text: String,
}

#[derive(Clone, Serialize)]
struct LiveChunk {
    session_id: u64,
    covered_start: f64,
    covered_end: f64,
    cues: Vec<LiveCue>,
}

#[derive(Clone, Serialize)]
struct LiveStatus {
    session_id: u64,
    // "loading" | "running" | "paused" | "done" | "error"
    state: String,
    message: String,
}

#[derive(Serialize)]
pub struct LiveSessionInfo {
    session_id: u64,
    model: String,
}

fn emit_status(app: &tauri::AppHandle, session_id: u64, state: &str, message: impl Into<String>) {
    let _ = app.emit(
        "live-subtitle-status",
        LiveStatus {
            session_id,
            state: state.to_string(),
            message: message.into(),
        },
    );
}

fn stop_session(session: Session) {
    session.cancel.store(true, Ordering::Relaxed);
    if let Some(mut child) = session.ffmpeg.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[tauri::command]
pub fn start_live_subtitles(
    app_handle: tauri::AppHandle,
    video_path: String,
    start_secs: f64,
    audio_stream_index: Option<i64>,
    language: String,
) -> Result<LiveSessionInfo, String> {
    let (model_file, model_label) = LIVE_MODELS
        .iter()
        .find(|(file, _)| super::find_model_path(file).is_some())
        .ok_or("No Whisper model installed. Download one from the Settings page.")?;
    let model_path = super::find_model_path(model_file)
        .unwrap()
        .to_string_lossy()
        .to_string();

    let mut st = state().lock().unwrap();
    if let Some(prev) = st.session.take() {
        stop_session(prev);
    }
    st.next_id += 1;
    let session_id = st.next_id;
    let cancel = Arc::new(AtomicBool::new(false));
    let ffmpeg = Arc::new(Mutex::new(None));
    st.session = Some(Session {
        id: session_id,
        cancel: cancel.clone(),
        ffmpeg: ffmpeg.clone(),
    });
    let cached = st
        .model
        .as_ref()
        .filter(|(path, _)| *path == model_path)
        .map(|(_, ctx)| ctx.clone());
    drop(st);

    PLAYHEAD.store(start_secs.max(0.0).to_bits(), Ordering::Relaxed);

    let job = Job {
        app: app_handle,
        session_id,
        video_path,
        start_secs: start_secs.max(0.0),
        audio_stream_index,
        language,
        model_path,
        cached,
        cancel,
        ffmpeg,
    };
    std::thread::spawn(move || {
        let app = job.app.clone();
        if let Err(e) = job.run() {
            emit_status(&app, session_id, "error", e);
        }
    });

    Ok(LiveSessionInfo {
        session_id,
        model: model_label.to_string(),
    })
}

// Stops the running session. `unload` also releases the Whisper model; pass false when
// restarting at a new position so the model stays in memory.
#[tauri::command]
pub fn stop_live_subtitles(unload: bool) {
    let mut st = state().lock().unwrap();
    if let Some(session) = st.session.take() {
        stop_session(session);
    }
    if unload {
        st.model = None;
    }
}

#[tauri::command]
pub fn update_live_subtitles_playhead(time: f64) {
    PLAYHEAD.store(time.max(0.0).to_bits(), Ordering::Relaxed);
}

struct Job {
    app: tauri::AppHandle,
    session_id: u64,
    video_path: String,
    start_secs: f64,
    audio_stream_index: Option<i64>,
    language: String,
    model_path: String,
    cached: Option<Arc<WhisperContext>>,
    cancel: Arc<AtomicBool>,
    ffmpeg: Arc<Mutex<Option<Child>>>,
}

// Reaps FFmpeg however the job ends (finished, cancelled or failed).
impl Drop for Job {
    fn drop(&mut self) {
        if let Some(mut child) = self.ffmpeg.lock().unwrap().take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl Job {
    fn cancelled(&self) -> bool {
        self.cancel.load(Ordering::Relaxed)
    }

    fn load_model(&self) -> Result<Arc<WhisperContext>, String> {
        if let Some(ctx) = &self.cached {
            return Ok(ctx.clone());
        }
        emit_status(&self.app, self.session_id, "loading", "Loading Whisper model...");
        let ctx = Arc::new(
            WhisperContext::new_with_params(&self.model_path, WhisperContextParameters::default())
                .map_err(|e| format!("Failed to load Whisper model: {}", e))?,
        );
        // Only cache if this session is still current, so a stopped session cannot
        // repopulate the cache after an unload.
        let mut st = state().lock().unwrap();
        if st.session.as_ref().map(|s| s.id) == Some(self.session_id) {
            st.model = Some((self.model_path.clone(), ctx.clone()));
        }
        Ok(ctx)
    }

    fn spawn_ffmpeg(&self) -> Result<std::process::ChildStdout, String> {
        let mut cmd = super::get_ffmpeg_command();
        cmd.args(["-v", "error", "-ss", &format!("{:.3}", self.start_secs)]);
        cmd.args(["-i", &self.video_path]);
        if let Some(index) = self.audio_stream_index {
            cmd.args(["-map", &format!("0:{}", index)]);
        }
        cmd.args(["-vn", "-ac", "1", "-ar", "16000", "-f", "f32le", "-"]);
        cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start FFmpeg: {}", e))?;
        let stdout = child.stdout.take().ok_or("Failed to read FFmpeg output")?;
        *self.ffmpeg.lock().unwrap() = Some(child);
        Ok(stdout)
    }

    fn run(self) -> Result<(), String> {
        let ctx = self.load_model()?;
        if self.cancelled() {
            return Ok(());
        }
        let mut stdout = self.spawn_ffmpeg()?;
        emit_status(&self.app, self.session_id, "running", "");

        let threads = std::thread::available_parallelism()
            .map(|n| n.get().min(MAX_THREADS))
            .unwrap_or(4) as i32;
        let window = WINDOW_SECS * SAMPLE_RATE;
        let mut buffer: Vec<f32> = Vec::with_capacity(window);
        // Absolute time of buffer[0].
        let mut buffer_start = self.start_secs;
        let mut eof = false;
        let mut bytes = vec![0u8; 64 * 1024];
        let mut pending = Vec::<u8>::new();

        while !self.cancelled() {
            // Fill the window from the FFmpeg pipe.
            while !eof && buffer.len() < window {
                match stdout.read(&mut bytes) {
                    Ok(0) => eof = true,
                    Ok(n) => {
                        pending.extend_from_slice(&bytes[..n]);
                        let whole = pending.len() / 4 * 4;
                        buffer.extend(
                            pending[..whole]
                                .chunks_exact(4)
                                .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])),
                        );
                        pending.drain(..whole);
                    }
                    Err(_) => eof = true,
                }
                if self.cancelled() {
                    return Ok(());
                }
            }
            if buffer.is_empty() {
                break;
            }

            let cut = if eof && buffer.len() <= window {
                buffer.len()
            } else {
                quietest_cut(&buffer[..window.min(buffer.len())])
            };
            let is_last = eof && cut == buffer.len();

            self.wait_for_playhead(buffer_start);
            if self.cancelled() {
                return Ok(());
            }

            let (cues, resume) =
                self.transcribe(&ctx, &buffer[..cut], buffer_start, threads, is_last)?;
            if self.cancelled() {
                return Ok(());
            }
            // A cue cut off by the chunk end is transcribed again, whole, in the next chunk.
            let cut = match resume {
                // Rounded up so the next chunk never starts before `time`.
                Some(time) => ((time - buffer_start) * SAMPLE_RATE as f64).ceil() as usize,
                None => cut,
            }
            .min(cut);
            let chunk_end = buffer_start + cut as f64 / SAMPLE_RATE as f64;
            let _ = self.app.emit(
                "live-subtitle-chunk",
                LiveChunk {
                    session_id: self.session_id,
                    covered_start: buffer_start,
                    covered_end: chunk_end,
                    cues,
                },
            );

            buffer.drain(..cut);
            buffer_start = chunk_end;
            if eof && buffer.is_empty() {
                break;
            }
        }

        if !self.cancelled() {
            emit_status(&self.app, self.session_id, "done", "");
        }
        Ok(())
    }

    // Blocks while the transcription is far enough ahead of the playhead.
    fn wait_for_playhead(&self, next_start: f64) {
        let mut paused = false;
        while !self.cancelled() {
            let playhead = f64::from_bits(PLAYHEAD.load(Ordering::Relaxed));
            if next_start - playhead < MAX_LEAD_SECS {
                break;
            }
            if !paused {
                emit_status(&self.app, self.session_id, "paused", "");
                paused = true;
            }
            std::thread::sleep(Duration::from_millis(500));
        }
        if paused && !self.cancelled() {
            emit_status(&self.app, self.session_id, "running", "");
        }
    }

    fn transcribe(
        &self,
        ctx: &WhisperContext,
        samples: &[f32],
        offset: f64,
        threads: i32,
        is_last: bool,
    ) -> Result<(Vec<LiveCue>, Option<f64>), String> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_translate(false);
        params.set_language(Some(&self.language));
        params.set_n_threads(threads);
        // Each chunk stands alone: carrying context over can trigger repetition loops.
        params.set_no_context(true);
        subtitle_format::request_word_timestamps(&mut params);
        params.set_suppress_blank(true);
        // SAFETY: the flag is owned by `self`, which outlives `state.full` below.
        unsafe {
            params.set_abort_callback(Some(abort_when_cancelled));
            params.set_abort_callback_user_data(Arc::as_ptr(&self.cancel) as *mut c_void);
        }

        let mut state = ctx
            .create_state()
            .map_err(|e| format!("Failed to create Whisper state: {}", e))?;
        if let Err(e) = state.full(params, samples) {
            if self.cancelled() {
                return Ok((Vec::new(), None));
            }
            return Err(format!("Transcription failed: {}", e));
        }

        let chunk_secs = samples.len() as f64 / SAMPLE_RATE as f64;
        let words = subtitle_format::words_from_state(&state, offset, chunk_secs);
        // Cues may not run past this chunk, and leave the usual gap before it ends: the
        // next chunk's speech is not known yet.
        let limit = offset + chunk_secs - subtitle_format::MIN_GAP;
        let mut cues = subtitle_format::build_cues(&words, limit);
        let resume = if is_last {
            None
        } else {
            subtitle_format::resume_point(&words, &cues, offset, offset + chunk_secs)
        };
        if let Some(time) = resume {
            cues.retain(|c| c.start < time);
        }
        let cues = cues
            .into_iter()
            .map(|c| LiveCue {
                start: c.start,
                end: c.end,
                text: c.text,
            })
            .collect();
        Ok((cues, resume))
    }
}

// whisper-rs 0.16's `set_abort_callback_safe` casts its boxed closure back to the closure's
// own type, so any closure that captures state reads unrelated memory as its result and
// aborts at random (the encoder then fails with -6). The raw callback with a pointer to
// the session's cancel flag avoids that.
unsafe extern "C" fn abort_when_cancelled(data: *mut c_void) -> bool {
    unsafe { (*(data as *const AtomicBool)).load(Ordering::Relaxed) }
}

// Returns a cut position near the end of `window` that falls on the quietest 50 ms frame
// in the last few seconds, so the next chunk starts in a pause rather than mid-word.
fn quietest_cut(window: &[f32]) -> usize {
    let search = CUT_SEARCH_SECS * SAMPLE_RATE;
    if window.len() <= search + ENERGY_FRAME {
        return window.len();
    }
    let from = window.len() - search;
    let mut best = window.len();
    let mut best_energy = f32::MAX;
    let mut pos = from;
    while pos + ENERGY_FRAME <= window.len() {
        let energy: f32 = window[pos..pos + ENERGY_FRAME].iter().map(|s| s * s).sum();
        if energy < best_energy {
            best_energy = energy;
            best = pos + ENERGY_FRAME / 2;
        }
        pos += ENERGY_FRAME;
    }
    best
}
