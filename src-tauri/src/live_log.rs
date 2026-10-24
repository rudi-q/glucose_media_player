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
    // Also shown, readably, in the terminal running `pnpm tauri dev`.
    eprintln!("{}", terminal_line(event, &record));
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&path) {
        let _ = writeln!(file, "{}", record);
    }
}

// One readable line (or two) for the terminal; the log file keeps the full record.
fn terminal_line(event: &str, r: &Value) -> String {
    let num = |key: &str| r.get(key).and_then(Value::as_f64);
    let text = |key: &str| r.get(key).and_then(Value::as_str);
    let secs = |key: &str| num(key).map(|ms| ms / 1000.0).unwrap_or(0.0);
    match event {
        "session_start" => {
            let model = text("model").unwrap_or("?");
            let load = if r.get("whisper_cached").and_then(Value::as_bool) == Some(true) {
                "already loaded".to_string()
            } else {
                format!("loaded in {:.1} s", secs("model_load_ms"))
            };
            let target = text("translate_to").map(|t| format!(" -> {}", t)).unwrap_or_default();
            format!(
                "[live] session {} started: {} from {} | {} ({}) | speech {}{} | {} threads",
                num("session").unwrap_or(0.0),
                text("video").unwrap_or("?"),
                clock(num("start_secs").unwrap_or(0.0)),
                model,
                load,
                text("language").unwrap_or("?"),
                target,
                num("threads").unwrap_or(0.0),
            )
        }
        "translator_load" => {
            let pair = format!("{}->{}", text("source").unwrap_or("?"), text("target").unwrap_or("?"));
            match text("error") {
                Some(e) => format!("[live] translation model {} FAILED after {} ms: {}", pair, num("ms").unwrap_or(0.0), e),
                None => format!("[live] translation model {} loaded in {} ms", pair, num("ms").unwrap_or(0.0)),
            }
        }
        "chunk" => {
            let translate = match (num("translate_ms"), text("translate_error")) {
                (_, Some(_)) => " | translation FAILED".to_string(),
                (Some(ms), None) => format!(" | translate {} ms", ms),
                (None, None) => String::new(),
            };
            let mut line = format!(
                "[live] {}-{} ({:.1} s of audio) | transcribe {:.1} s{} | {:.1}x real time | {} subtitles | {:.0} s ahead of you{}",
                clock(num("start").unwrap_or(0.0)),
                clock(num("end").unwrap_or(0.0)),
                num("audio_secs").unwrap_or(0.0),
                secs("whisper_ms"),
                translate,
                num("speed_x").unwrap_or(0.0),
                num("cues").unwrap_or(0.0),
                num("lead_secs").unwrap_or(0.0),
                if r.get("late").and_then(Value::as_bool) == Some(true) { " | LATE: you got there first" } else { "" },
            );
            let translated = r.get("original_reading").is_some_and(|v| !v.is_null());
            let label = if translated { "translated" } else { "subtitles" };
            if let Some(stats) = r.get("reading").and_then(reading_line) {
                line.push_str(&format!("\n[live]   {:<10} {}", label, stats));
            }
            if let Some(stats) = r.get("original_reading").and_then(reading_line) {
                line.push_str(&format!("\n[live]   {:<10} {}", "original", stats));
            }
            if r.get("first").and_then(Value::as_bool) == Some(true) {
                line.push_str(&format!("\n[live] first subtitles ready {:.1} s after start", secs("since_start_ms")));
            }
            line
        }
        "session_end" => match text("error") {
            Some(e) => format!("[live] session {} ended with an error: {}", num("session").unwrap_or(0.0), e),
            None => format!("[live] session {} {}", num("session").unwrap_or(0.0), text("reason").unwrap_or("ended")),
        },
        "ui_stall_ongoing" => format!(
            "[freeze] window not responding for {:.0} s (running: {})",
            secs("ms"),
            text("command").unwrap_or("no app command"),
        ),
        "ui_stall" => format!(
            "[freeze] window was stuck for {} ms (running: {})",
            num("ms").unwrap_or(0.0),
            text("command").unwrap_or("no app command"),
        ),
        "media_read_slow" => format!(
            "[file] slow read: {} took {:.1} s",
            text("file").unwrap_or("?"),
            secs("ms"),
        ),
        _ => format!("[glucose log] {}", r),
    }
}

// "CPS avg 15.2, max 24.1, 3 over 21 | CPL max 41, 0 lines over 42", or None for a
// chunk without subtitles.
fn reading_line(stats: &Value) -> Option<String> {
    let num = |key: &str| stats.get(key).and_then(Value::as_f64);
    Some(format!(
        "CPS avg {:.1}, max {:.1}, {} over 21 | CPL max {}, {} lines over 42",
        num("cps_avg")?,
        num("cps_max")?,
        num("over_21_cps")?,
        num("cpl_max")?,
        num("lines_over_42")?,
    ))
}

// A video position as m:ss, or h:mm:ss from an hour on.
fn clock(secs: f64) -> String {
    let total = secs.max(0.0).round() as u64;
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{}:{:02}:{:02}", h, m, s)
    } else {
        format!("{}:{:02}", m, s)
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

#[cfg(test)]
mod tests {
    use super::{clock, terminal_line};
    use serde_json::json;

    #[test]
    fn formats_clock() {
        assert_eq!(clock(0.0), "0:00");
        assert_eq!(clock(75.4), "1:15");
        assert_eq!(clock(3725.0), "1:02:05");
    }

    #[test]
    fn formats_chunk_line() {
        let reading = |avg: f64, max: f64, over: u64, cpl: u64| {
            json!({ "cps_avg": avg, "cps_max": max, "over_21_cps": over, "cpl_max": cpl, "lines_over_42": 0 })
        };
        let r = json!({
            "start": 20.0, "end": 39.6, "audio_secs": 19.6, "whisper_ms": 3900, "translate_ms": 72,
            "speed_x": 5.0, "cues": 14, "lead_secs": 31.0, "late": false, "first": true,
            "since_start_ms": 4300, "reading": reading(19.4, 27.3, 5, 41),
            "original_reading": reading(14.8, 21.9, 1, 38),
        });
        let line = terminal_line("chunk", &r);
        println!("{}", line);
        let lines: Vec<&str> = line.lines().collect();
        assert_eq!(
            lines[0],
            "[live] 0:20-0:40 (19.6 s of audio) | transcribe 3.9 s | translate 72 ms | 5.0x real time | 14 subtitles | 31 s ahead of you"
        );
        assert_eq!(lines[1], "[live]   translated CPS avg 19.4, max 27.3, 5 over 21 | CPL max 41, 0 lines over 42");
        assert_eq!(lines[2], "[live]   original   CPS avg 14.8, max 21.9, 1 over 21 | CPL max 38, 0 lines over 42");
        assert_eq!(lines[3], "[live] first subtitles ready 4.3 s after start");
    }
}
