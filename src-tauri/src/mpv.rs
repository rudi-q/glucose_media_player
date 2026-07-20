// SPIKE (branch: spike/mpv-lite-proof)
//
// "Lite mode": playback for codecs the WebView can't decode (ProRes etc.) via a
// separate mpv process. mpv is launched arms-length (no libmpv linking) so
// glucose stays EUPL-clean even against a GPL mpv build.
//
// Two layers:
//   * play_with_mpv      — Flavor 1: mpv opens in its own raw window. Proven.
//   * open_lite_window   — Flavor 2: mpv is embedded (--wid) inside a frameless,
//                          glucose-styled Tauri window, z-ordered *behind* the
//                          transparent WebView so our HTML chrome/controls paint
//                          on top. Controls drive mpv over its JSON IPC pipe.
//
// Robustness model (the lifecycle is the part most likely to bite):
//   * The single Lite session (mpv Child + IPC pipe) lives in LITE.
//   * Window closed by ANY means (button, Esc, Alt+F4, OS) -> on_window_event
//     kills + reaps mpv and clears the session. No orphan processes.
//   * mpv exits on its own (crash / EOF / user quit) -> a watcher thread reaps
//     it, clears the session, and closes the now-empty window. No dead frames.
//   * Every command returns Result/Option and never unwraps on external state;
//     a poisoned mutex is recovered, never panicked on.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

// Active Lite session. Holds the mpv Child so we can reap/kill it deterministically.
struct LiteSession {
    ipc: String,
    child: std::process::Child,
}
static LITE: Mutex<Option<LiteSession>> = Mutex::new(None);

const LITE_LABEL: &str = "lite";

fn lock_lite() -> std::sync::MutexGuard<'static, Option<LiteSession>> {
    LITE.lock().unwrap_or_else(|e| e.into_inner())
}

fn now_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0)
}

// Take the session out and make sure mpv is dead and reaped. Safe to call when
// there is no session. Never blocks for long: kill() is forceful, so the
// following wait() returns promptly.
fn kill_lite_session() {
    let session = { lock_lite().take() };
    if let Some(mut s) = session {
        let _ = s.child.kill();
        let _ = s.child.wait();
    }
}

// Confirm a path is a working mpv by running `--version` and sniffing the banner.
// Mirrors ffmpeg::validates_as_ffmpeg so behaviour/feel matches the rest of the app.
fn validates_as_mpv(path: &Path) -> bool {
    path.is_file()
        && crate::create_hidden_command(path.to_string_lossy().as_ref())
            .arg("--version")
            .output()
            .map(|o| {
                let stdout = String::from_utf8_lossy(&o.stdout);
                let stderr = String::from_utf8_lossy(&o.stderr);
                stdout.contains("mpv") || stderr.contains("mpv")
            })
            .unwrap_or(false)
}

// Resolution order, mirroring resolve_ffmpeg_path_info:
//   1. bundled location under LOCALAPPDATA\glucose\resources\mpv (Windows)
//   2. system PATH
fn resolve_mpv_path() -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
            let bundled = Path::new(&app_data)
                .join("glucose")
                .join("resources")
                .join("mpv")
                .join("mpv.exe");
            if validates_as_mpv(&bundled) {
                return Some(bundled.to_string_lossy().to_string());
            }
        }
    }

    let exe_name = if cfg!(target_os = "windows") {
        "mpv.exe"
    } else {
        "mpv"
    };

    let path_var = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path_var) {
        let candidate: PathBuf = dir.join(exe_name);
        if validates_as_mpv(&candidate) {
            return Some(candidate.to_string_lossy().to_string());
        }
    }

    None
}

// Frontend uses this to decide whether to offer Lite mode at all. Async so the
// (potentially process-spawning) probe never blocks the UI thread.
#[tauri::command]
pub async fn check_mpv_installed() -> Option<String> {
    tokio::task::spawn_blocking(resolve_mpv_path).await.ok().flatten()
}

// Flavor 1: launch mpv on the given file in its own window, detached.
#[tauri::command]
pub fn play_with_mpv(video_path: String) -> Result<(), String> {
    if !Path::new(&video_path).exists() {
        return Err(format!("File does not exist: {}", video_path));
    }

    let mpv = resolve_mpv_path().ok_or_else(mpv_missing_message)?;

    let mut cmd = crate::create_hidden_command(&mpv);
    cmd.arg("--force-window=yes")
        .arg("--title=glucose Lite — ${filename}")
        .arg(&video_path);

    cmd.spawn()
        .map(|_child| ())
        .map_err(|e| format!("Failed to launch mpv: {}", e))
}

fn mpv_missing_message() -> String {
    "mpv not found. Install mpv or place mpv.exe under \
     %LOCALAPPDATA%\\glucose\\resources\\mpv\\."
        .to_string()
}

// Flavor 2: embed mpv inside a frameless, glucose-styled Tauri window.
#[tauri::command]
pub fn open_lite_window(app: tauri::AppHandle, video_path: String) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    if !Path::new(&video_path).exists() {
        return Err(format!("File does not exist: {}", video_path));
    }
    let mpv = resolve_mpv_path().ok_or_else(mpv_missing_message)?;

    // One Lite session at a time. Kill any running mpv and destroy a leftover
    // window so the new label is free.
    kill_lite_session();
    if let Some(existing) = app.get_webview_window(LITE_LABEL) {
        let _ = existing.destroy();
    }

    let filename = Path::new(&video_path)
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("video")
        .to_string();

    // Pass the filename to the standalone page via query string (reliable; no
    // dependency on eval timing).
    let url = format!("lite.html?f={}", urlencoding::encode(&filename));
    let win = WebviewWindowBuilder::new(&app, LITE_LABEL, WebviewUrl::App(url.into()))
        .title(format!("glucose Lite — {}", filename))
        .inner_size(1280.0, 720.0)
        .min_inner_size(640.0, 360.0)
        .decorations(false)
        .transparent(true)
        .build()
        .map_err(|e| format!("Failed to create Lite window: {}", e))?;

    // Window closed by ANY path -> ensure mpv dies and the session is cleared.
    win.on_window_event(|event| {
        if matches!(
            event,
            tauri::WindowEvent::CloseRequested { .. } | tauri::WindowEvent::Destroyed
        ) {
            kill_lite_session();
        }
    });

    // Helper: tear down the half-built window if the rest of setup fails, so we
    // never leave an empty frame behind.
    let abort_window = || {
        if let Some(w) = app.get_webview_window(LITE_LABEL) {
            let _ = w.destroy();
        }
    };

    #[cfg(target_os = "windows")]
    {
        let hwnd = match win.hwnd() {
            Ok(h) => h,
            Err(e) => {
                abort_window();
                return Err(format!("Failed to get Lite window handle: {}", e));
            }
        };
        let wid = hwnd.0 as usize;
        let ipc = format!(r"\\.\pipe\glucose-mpv-{}-{}", std::process::id(), now_nanos());

        let mut cmd = crate::create_hidden_command(&mpv);
        cmd.arg(format!("--wid={}", wid))
            .arg(format!("--input-ipc-server={}", ipc))
            .arg("--osc=no") // we draw our own glucose controls
            .arg("--no-border")
            .arg("--keep-open=yes")
            .arg("--force-window=yes")
            .arg("--idle=no")
            .arg(&video_path);

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => {
                abort_window();
                return Err(format!("Failed to launch mpv: {}", e));
            }
        };

        *lock_lite() = Some(LiteSession {
            ipc,
            child,
        });

        // Single background thread, two phases:
        //   1. Push mpv's embedded child below the WebView (retry until it
        //      exists, ~4s budget) so our transparent chrome sits on top.
        //   2. Watch for mpv exit; when it dies, clear the session and close the
        //      now-empty window.
        let app_bg = app.clone();
        std::thread::spawn(move || {
            for _ in 0..40 {
                std::thread::sleep(std::time::Duration::from_millis(100));
                if lock_lite().is_none() {
                    return; // session ended during startup
                }
                if unsafe { push_mpv_child_to_bottom(wid) } {
                    break;
                }
            }

            loop {
                std::thread::sleep(std::time::Duration::from_millis(500));
                let exited = {
                    let mut guard = lock_lite();
                    match guard.as_mut() {
                        Some(s) => match s.child.try_wait() {
                            Ok(Some(_)) => true, // mpv exited; already reaped
                            Ok(None) => false,
                            Err(_) => true, // can't query -> treat as gone
                        },
                        None => return, // closed elsewhere
                    }
                };
                if exited {
                    *lock_lite() = None;
                    if let Some(w) = app_bg.get_webview_window(LITE_LABEL) {
                        let _ = w.close();
                    }
                    return;
                }
            }
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        let _ = (&win, &mpv, &filename, abort_window);
        abort_window();
        return Err("Lite window embedding is Windows-only in this spike".to_string());
    }

    #[cfg(target_os = "windows")]
    Ok(())
}

// Send a JSON IPC command to the active mpv (e.g. ["set_property","pause",true]).
// Async + spawn_blocking so a momentarily-busy pipe never stalls the UI thread.
#[tauri::command]
pub async fn lite_mpv_command(args: Vec<serde_json::Value>) -> Result<(), String> {
    let ipc = { lock_lite().as_ref().map(|s| s.ipc.clone()) }
        .ok_or("No active Lite session")?;

    let payload = serde_json::json!({ "command": args });
    let line = format!("{}\n", payload);

    tokio::task::spawn_blocking(move || -> Result<(), String> {
        #[cfg(target_os = "windows")]
        {
            use std::io::Write;
            let mut pipe = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(&ipc)
                .map_err(|e| format!("Failed to reach Lite player: {}", e))?;
            pipe.write_all(line.as_bytes())
                .map_err(|e| format!("Failed to send Lite command: {}", e))?;
        }
        #[cfg(not(target_os = "windows"))]
        {
            let _ = (&ipc, &line);
        }
        Ok(())
    })
    .await
    .map_err(|e| format!("Lite command task failed: {}", e))?
}

// Close the Lite window. mpv teardown happens in the window's close handler, so
// this is just a request; it's a no-op if nothing is open.
#[tauri::command]
pub fn close_lite_window(app: tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    if let Some(w) = app.get_webview_window(LITE_LABEL) {
        let _ = w.close();
    } else {
        // Window already gone but a session somehow lingered — clean it up.
        kill_lite_session();
    }
    Ok(())
}

// --- Windows z-order plumbing -------------------------------------------------

// Pushes the mpv-owned child window of `parent` below its siblings (the WebView)
// so our transparent chrome paints on top. Returns true once an mpv child was
// found and repositioned. Matching is by window class ("mpv") — robust even if
// the pid lookup race-loses, and avoids touching the WebView's own child.
#[cfg(target_os = "windows")]
unsafe extern "system" fn enum_child_cb(
    child: windows::Win32::Foundation::HWND,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::core::BOOL {
    use windows::core::BOOL;
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, SetWindowPos, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    };

    let found = &mut *(lparam.0 as *mut bool);

    let mut buf = [0u16; 64];
    let len = GetClassNameW(child, &mut buf);
    if len > 0 {
        let class = String::from_utf16_lossy(&buf[..len as usize]);
        if class == "mpv" {
            let _ = SetWindowPos(
                child,
                Some(HWND_BOTTOM),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            );
            *found = true;
            return BOOL(0); // stop enumerating
        }
    }
    BOOL(1)
}

#[cfg(target_os = "windows")]
unsafe fn push_mpv_child_to_bottom(parent: usize) -> bool {
    use windows::Win32::Foundation::{HWND, LPARAM};
    use windows::Win32::UI::WindowsAndMessaging::EnumChildWindows;

    if parent == 0 {
        return false;
    }
    let mut found = false;
    let parent_hwnd = HWND(parent as *mut core::ffi::c_void);
    let _ = EnumChildWindows(
        Some(parent_hwnd),
        Some(enum_child_cb),
        LPARAM(&mut found as *mut bool as isize),
    );
    found
}
