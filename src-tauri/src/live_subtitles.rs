// Live subtitles: transcribes audio ahead of the playhead in chunks and streams the
// resulting cues to the frontend, so subtitles appear while the video plays instead of
// after a full batch run. See docs/spikes/live-subtitles.md.

use crate::live_log;
use crate::subtitle_format;
use crate::translate::{self, CueTranslator};
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

/// The model live mode would use, or None when no live-capable model is installed. Live
/// mode has its own model list, so the batch models' "installed" list cannot answer this.
/// Async so the file checks do not run on the window thread.
#[tauri::command]
pub async fn live_subtitle_model() -> Option<String> {
    preferred_model().map(str::to_string)
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
    // Same for translation models, one per source/target pair used so far.
    translators: Vec<Arc<CueTranslator>>,
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
    // The untranslated cues when `cues` are a translation, else empty. Shown under the
    // translation when dual subtitles are on.
    original_cues: Vec<LiveCue>,
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

// Called from commands, which run on the window thread: FFmpeg is killed and reaped on
// a separate thread so a slow exit under heavy load cannot freeze the window.
fn stop_session(session: Session) {
    session.cancel.store(true, Ordering::Relaxed);
    if let Some(mut child) = session.ffmpeg.lock().unwrap().take() {
        std::thread::spawn(move || {
            let _ = child.kill();
            let _ = child.wait();
        });
    }
}

#[tauri::command]
pub fn start_live_subtitles(
    app_handle: tauri::AppHandle,
    video_path: String,
    start_secs: f64,
    audio_stream_index: Option<i64>,
    language: String,
    // Language code to translate speech into, if any: "de", "fr", "es", "ru", "tr", "it",
    // "nl" for English speech, or "en" for speech in the languages translate.rs covers.
    translate_to: Option<String>,
) -> Result<LiveSessionInfo, String> {
    if let Some(target) = &translate_to {
        // Every model either translates from English or into English, so the speech
        // language is English unless English is the target; then it is the language set
        // in Settings, or unknown when that is "auto".
        let source = if target != "en" {
            Some("en")
        } else if language != "auto" {
            Some(language.as_str())
        } else {
            None
        };
        translate::check_installed(source, target)?;
    }
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
    let cached_translators: Vec<_> = st
        .translators
        .iter()
        .filter(|t| Some(&t.target) == translate_to.as_ref())
        .cloned()
        .collect();
    drop(st);

    PLAYHEAD.store(start_secs.max(0.0).to_bits(), Ordering::Relaxed);

    let job = Job {
        app: app_handle,
        session_id,
        video_path,
        start_secs: start_secs.max(0.0),
        audio_stream_index,
        language,
        translate_to,
        model_path,
        model_label: model_label.to_string(),
        cached,
        translators: Mutex::new(cached_translators),
        failed_translations: Mutex::new(Vec::new()),
        cancel,
        ffmpeg,
    };
    std::thread::spawn(move || {
        let app = job.app.clone();
        let cancel = job.cancel.clone();
        let result = job.run();
        let reason = match &result {
            Err(_) => "error",
            Ok(()) if cancel.load(Ordering::Relaxed) => "cancelled",
            Ok(()) => "done",
        };
        live_log::log(
            "session_end",
            serde_json::json!({ "session": session_id, "reason": reason, "error": result.as_ref().err() }),
        );
        if let Err(e) = result {
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
        st.translators.clear();
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
    translate_to: Option<String>,
    model_path: String,
    // Short model name ("small-q5_1"), for the performance log.
    model_label: String,
    cached: Option<Arc<WhisperContext>>,
    // Translators this session has loaded or reused, at most one per source language.
    translators: Mutex<Vec<Arc<CueTranslator>>>,
    // Source languages whose translator failed to load, so it is not retried every chunk.
    failed_translations: Mutex<Vec<String>>,
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
        emit_status(&self.app, self.session_id, "loading", "Loading speech model...");
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

    // Returns the translator from `source` speech into the session's target language,
    // loading it on first use. None when not translating, when the speech is already in
    // the target language, or when no model covers this language pair; the original text
    // is shown then. `during_run` restores the "running" status after a mid-session load.
    fn translator_for(
        &self,
        source: &str,
        threads: usize,
        during_run: bool,
    ) -> Result<Option<Arc<CueTranslator>>, String> {
        let Some(target) = &self.translate_to else {
            return Ok(None);
        };
        if source == target
            || !translate::is_available(source, target)
            || self.failed_translations.lock().unwrap().iter().any(|s| s == source)
        {
            return Ok(None);
        }
        if let Some(t) = self.translators.lock().unwrap().iter().find(|t| t.source == source) {
            return Ok(Some(t.clone()));
        }
        emit_status(
            &self.app,
            self.session_id,
            "loading",
            format!("Loading {} translation model...", translate::language_name(target)),
        );
        let load_started = std::time::Instant::now();
        let loaded = CueTranslator::load(source, target, threads);
        live_log::log(
            "translator_load",
            serde_json::json!({
                "session": self.session_id,
                "source": source,
                "target": target,
                "ms": live_log::ms(load_started),
                "during_run": during_run,
                "error": loaded.as_ref().err(),
            }),
        );
        let translator = match loaded {
            Ok(t) => Arc::new(t),
            Err(e) => {
                // Remember the failure and clear the loading status; the caller shows the
                // original text instead.
                self.failed_translations.lock().unwrap().push(source.to_string());
                if during_run {
                    emit_status(&self.app, self.session_id, "running", "");
                }
                return Err(e);
            }
        };
        self.translators.lock().unwrap().push(translator.clone());
        let mut st = state().lock().unwrap();
        if st.session.as_ref().map(|s| s.id) == Some(self.session_id) {
            st.translators.push(translator.clone());
        }
        drop(st);
        if during_run {
            emit_status(&self.app, self.session_id, "running", "");
        }
        Ok(Some(translator))
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
        let session_started = std::time::Instant::now();
        let threads = std::thread::available_parallelism()
            .map(|n| n.get().min(MAX_THREADS))
            .unwrap_or(4) as i32;
        let model_started = std::time::Instant::now();
        let whisper_cached = self.cached.is_some();
        let ctx = self.load_model()?;
        let model_load_ms = live_log::ms(model_started);
        // Translation runs on this thread right after Whisper, so it can use the same
        // thread budget without competing with it. Load the model up front when the
        // speech language is known: English for translating out of English, or the
        // language chosen in settings for translating into English. Otherwise it loads
        // when Whisper first detects a language it covers.
        if let Some(target) = &self.translate_to {
            let source = if target != "en" { "en" } else { self.language.as_str() };
            if source != "auto" {
                // A model that fails to load must not stop the session: chunks then show
                // the original text.
                if let Err(e) = self.translator_for(source, threads as usize, false) {
                    eprintln!("[live subtitles] {}", e);
                }
            }
        }
        live_log::log(
            "session_start",
            serde_json::json!({
                "session": self.session_id,
                "video": live_log::file_name(&self.video_path),
                "start_secs": self.start_secs,
                "audio_stream": self.audio_stream_index,
                "model": self.model_label,
                "whisper_cached": whisper_cached,
                "model_load_ms": model_load_ms,
                "language": self.language,
                "translate_to": self.translate_to,
                "threads": threads,
            }),
        );
        if self.cancelled() {
            return Ok(());
        }
        let mut stdout = self.spawn_ffmpeg()?;
        emit_status(&self.app, self.session_id, "running", "");

        let window = WINDOW_SECS * SAMPLE_RATE;
        let mut buffer: Vec<f32> = Vec::with_capacity(window);
        // Absolute time of buffer[0].
        let mut buffer_start = self.start_secs;
        let mut first_chunk = true;
        let mut eof = false;
        // Set when FFmpeg stopped with an error rather than at the end of the audio; the
        // audio read so far is still transcribed, then the session ends with this error.
        let mut ffmpeg_error: Option<String> = None;
        let mut bytes = vec![0u8; 64 * 1024];
        let mut pending = Vec::<u8>::new();

        while !self.cancelled() {
            // Fill the window from the FFmpeg pipe.
            while !eof && buffer.len() < window {
                match stdout.read(&mut bytes) {
                    Ok(0) => {
                        eof = true;
                        ffmpeg_error = self.ffmpeg_exit_error();
                    }
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
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {}
                    Err(e) => {
                        eof = true;
                        ffmpeg_error = Some(format!("Could not read audio from FFmpeg: {e}"));
                    }
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

            let wait_started = std::time::Instant::now();
            self.wait_for_playhead(buffer_start);
            let wait_ms = live_log::ms(wait_started);
            if self.cancelled() {
                return Ok(());
            }

            let (cues, original_cues, resume, stats) =
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
            let audio_secs = chunk_end - buffer_start;
            let playhead = f64::from_bits(PLAYHEAD.load(Ordering::Relaxed));
            let work_ms = stats.whisper_ms + stats.translate_ms.unwrap_or(0);
            live_log::log(
                "chunk",
                serde_json::json!({
                    "session": self.session_id,
                    "start": buffer_start,
                    "end": chunk_end,
                    "audio_secs": audio_secs,
                    "wait_ms": wait_ms,
                    "whisper_ms": stats.whisper_ms,
                    "translate_ms": stats.translate_ms,
                    "speed_x": if work_ms > 0 { audio_secs * 1000.0 / work_ms as f64 } else { 0.0 },
                    "speech_lang": stats.speech_lang,
                    "translated": stats.translated,
                    "translate_error": stats.translate_error,
                    "cues": cues.len(),
                    "rewound": resume.is_some(),
                    "playhead": playhead,
                    // How far the subtitles now reach past the playhead; negative means
                    // the viewer is already past the end of this chunk.
                    "lead_secs": chunk_end - playhead,
                    // The playhead had passed the chunk start before its subtitles were
                    // ready, so the viewer saw a gap (or waited, with "pause until ready").
                    "late": playhead > buffer_start,
                    // Time from the session starting to this chunk's subtitles; for the
                    // first chunk, how long the viewer waited for any subtitles.
                    "since_start_ms": live_log::ms(session_started),
                    "first": first_chunk,
                    // Reading speed and line length of the subtitles shown, and of the
                    // untranslated ones when translating (null otherwise).
                    "reading": reading_stats(&cues),
                    "original_reading": reading_stats(&original_cues),
                }),
            );
            first_chunk = false;
            let _ = self.app.emit(
                "live-subtitle-chunk",
                LiveChunk {
                    session_id: self.session_id,
                    covered_start: buffer_start,
                    covered_end: chunk_end,
                    cues,
                    original_cues,
                },
            );

            buffer.drain(..cut);
            buffer_start = chunk_end;
            if eof && buffer.is_empty() {
                break;
            }
        }

        if self.cancelled() {
            return Ok(());
        }
        if let Some(e) = ffmpeg_error {
            return Err(e);
        }
        emit_status(&self.app, self.session_id, "done", "");
        Ok(())
    }

    // Waits for FFmpeg after its output ended and describes how it failed, or None when it
    // finished normally (or was stopped on purpose).
    fn ffmpeg_exit_error(&self) -> Option<String> {
        let mut child = self.ffmpeg.lock().unwrap().take()?;
        match child.wait() {
            Ok(status) if status.success() || self.cancelled() => None,
            Ok(status) => Some(match status.code() {
                Some(code) => format!("FFmpeg stopped with an error (exit code {code})"),
                None => "FFmpeg stopped unexpectedly".to_string(),
            }),
            Err(e) => Some(format!("Could not check how FFmpeg finished: {e}")),
        }
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
    ) -> Result<(Vec<LiveCue>, Vec<LiveCue>, Option<f64>, ChunkStats), String> {
        let mut stats = ChunkStats::default();
        let whisper_started = std::time::Instant::now();
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
                return Ok((Vec::new(), Vec::new(), None, stats));
            }
            return Err(format!("Transcription failed: {}", e));
        }
        stats.whisper_ms = live_log::ms(whisper_started);

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
        // A session cancelled during transcription (for example by a seek) should not
        // spend CPU on translation while its replacement starts.
        if self.cancelled() {
            return Ok((Vec::new(), Vec::new(), None, stats));
        }
        // Translate when a model covers this chunk's speech language; otherwise, and if
        // translation fails, the original text is shown, which beats showing nothing.
        let source = if self.language != "auto" {
            self.language.clone()
        } else {
            whisper_rs::get_lang_str(state.full_lang_id_from_state())
                .unwrap_or_default()
                .to_string()
        };
        stats.speech_lang = source.clone();
        let mut originals = Vec::new();
        match self.translator_for(&source, threads as usize, true) {
            Ok(Some(translator)) => {
                let limit = resume.map(|t| t - subtitle_format::MIN_GAP).unwrap_or(limit);
                let translate_started = std::time::Instant::now();
                match translator.translate_cues(&cues, limit) {
                    Ok(translated) => {
                        originals = std::mem::replace(&mut cues, translated);
                        stats.translated = true;
                    }
                    Err(e) => {
                        eprintln!("[live subtitles] {}", e);
                        stats.translate_error = Some(e);
                    }
                }
                stats.translate_ms = Some(live_log::ms(translate_started));
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("[live subtitles] {}", e);
                stats.translate_error = Some(e);
            }
        }
        let to_live = |cues: Vec<subtitle_format::Cue>| -> Vec<LiveCue> {
            cues.into_iter()
                .map(|c| LiveCue {
                    start: c.start,
                    end: c.end,
                    text: c.text,
                })
                .collect()
        };
        Ok((to_live(cues), to_live(originals), resume, stats))
    }
}

// Reading speed (CPS) and line length (CPL) of a chunk's cues, for the performance log.
// CPS counts every character except line breaks; 21 CPS is the limit Papi et al. (2023)
// use, a little above the formatter's own 20 CPS target.
fn reading_stats(cues: &[LiveCue]) -> serde_json::Value {
    const CPS_LIMIT: f64 = 21.0;
    if cues.is_empty() {
        return serde_json::Value::Null;
    }
    let (mut cps_sum, mut cps_max, mut over_cps) = (0.0, 0.0_f64, 0);
    let (mut cpl_max, mut long_lines) = (0, 0);
    for cue in cues {
        let chars = cue.text.chars().filter(|c| *c != '\n').count();
        let cps = chars as f64 / (cue.end - cue.start).max(1e-3);
        cps_sum += cps;
        cps_max = cps_max.max(cps);
        over_cps += usize::from(cps > CPS_LIMIT);
        for line in cue.text.split('\n') {
            let len = line.chars().count();
            cpl_max = cpl_max.max(len);
            long_lines += usize::from(len > subtitle_format::MAX_LINE_CHARS);
        }
    }
    serde_json::json!({
        "cps_avg": cps_sum / cues.len() as f64,
        "cps_max": cps_max,
        "over_21_cps": over_cps,
        "cpl_max": cpl_max,
        "lines_over_42": long_lines,
    })
}

// What happened to one chunk, for the performance log.
#[derive(Default)]
struct ChunkStats {
    whisper_ms: u64,
    // None when the chunk was not sent for translation.
    translate_ms: Option<u64>,
    // The language used for the chunk: the one set in Settings, or Whisper's detection.
    speech_lang: String,
    translated: bool,
    translate_error: Option<String>,
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
