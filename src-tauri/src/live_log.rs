// Development-only performance log for live subtitles: one JSON object per line in
// <local app data>/glucose/logs/live-subtitles.jsonl, for analysing test sessions.
// It records timings, settings and counts only: no subtitle text and no full file paths.

use serde_json::{json, Value};
use std::io::Write;
use std::sync::Mutex;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// The command being dispatched on the window thread right now, if any.
static CURRENT_COMMAND: Mutex<Option<String>> = Mutex::new(None);

// Wraps the app's command handler. Non-async commands run inside it on the window
// thread, so the freeze detector below can name the one that was running.
pub(crate) fn with_command_log<R: tauri::Runtime>(
    handler: impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static,
) -> impl Fn(tauri::ipc::Invoke<R>) -> bool + Send + Sync + 'static {
    move |invoke| {
        if !cfg!(debug_assertions) {
            return handler(invoke);
        }
        set_current_command(Some(invoke.message.command().to_string()));
        let handled = handler(invoke);
        set_current_command(None);
        handled
    }
}

fn set_current_command(name: Option<String>) {
    *CURRENT_COMMAND.lock().unwrap_or_else(|e| e.into_inner()) = name;
}

fn current_command() -> Option<String> {
    CURRENT_COMMAND.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

// Once the log passes this size it is moved aside (one previous file is kept).
const MAX_BYTES: u64 = 5 * 1024 * 1024;

// Appends one event. Does nothing in release builds, and never fails the caller.
pub(crate) fn log(event: &str, fields: Value) {
    if !cfg!(debug_assertions) {
        return;
    }
    let Some(dir) = dirs::data_local_dir().map(|d| d.join("glucose").join("logs")) else {
        return;
    };
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    let path = dir.join("live-subtitles.jsonl");
    if std::fs::metadata(&path).map(|m| m.len() > MAX_BYTES).unwrap_or(false) {
        let _ = std::fs::rename(&path, dir.join("live-subtitles.old.jsonl"));
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut record = json!({ "ts": ts, "event": event });
    if let (Some(record), Value::Object(fields)) = (record.as_object_mut(), fields) {
        record.extend(fields);
    }
    // Also shown in the terminal running `pnpm tauri dev`.
    eprintln!("[glucose log] {}", record);
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{}", record);
    }
}

// Development only: checks every half second that the window thread still answers.
// Windows shows "Not responding" after about 5 seconds, so this records what led up to
// one: "ui_stall_ongoing" once a second while it is stuck (in case the app is closed
// mid-freeze), and "ui_stall" when it answers again after more than STALL_MS. Both name
// the command that was running on the window thread, or null when it was something else
// (the webview, window handling).
pub(crate) fn watch_ui_thread(app: tauri::AppHandle) {
    const STALL_MS: u64 = 300;
    if !cfg!(debug_assertions) {
        return;
    }
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(500));
        let (tx, rx) = std::sync::mpsc::channel();
        let started = Instant::now();
        if app.run_on_main_thread(move || {
            let _ = tx.send(());
        })
        .is_err()
        {
            return;
        }
        let mut command = None;
        let mut logged_secs = 0;
        loop {
            match rx.recv_timeout(Duration::from_millis(250)) {
                Ok(()) => break,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    command = current_command().or(command);
                    let secs = ms(started) / 1000;
                    if secs > logged_secs {
                        logged_secs = secs;
                        log("ui_stall_ongoing", json!({ "ms": ms(started), "command": command }));
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
            }
        }
        let waited = ms(started);
        if waited > STALL_MS {
            log("ui_stall", json!({ "ms": waited, "command": command }));
        }
    });
}

// Milliseconds since `start`, for timing fields.
pub(crate) fn ms(start: Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

// The file name only, so the log never holds a full local path.
pub(crate) fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default()
}
