// SPIKE (branch: spike/mpv-lite-proof)
//
// Subprocess-based "Lite" playback for codecs the WebView cannot decode
// (Apple ProRes being the motivating case). mpv is launched as a *separate
// process* — arms-length aggregation — so glucose stays EUPL-clean even when
// pointed at a GPL-licensed mpv build. No libmpv linking, no DLLs in our binary.
//
// This is intentionally minimal: resolve an mpv binary, then spawn it on the
// file with its own window. It exists to answer one question on real hardware:
// does shelling out to mpv actually play the file glucose's <video> can't?

use std::path::{Path, PathBuf};

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
// (No custom-config path yet — that's a productisation detail, not needed to
// prove the concept.)
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

// Frontend uses this to decide whether to offer the "Open in Lite player" action.
#[tauri::command]
pub fn check_mpv_installed() -> Option<String> {
    resolve_mpv_path()
}

// Launch mpv on the given file in its own window, detached. We deliberately do
// not wait on the child: glucose keeps running while mpv plays separately.
#[tauri::command]
pub fn play_with_mpv(video_path: String) -> Result<(), String> {
    if !Path::new(&video_path).exists() {
        return Err(format!("File does not exist: {}", video_path));
    }

    let mpv = resolve_mpv_path().ok_or_else(|| {
        "mpv not found. Install mpv or place mpv.exe under \
         %LOCALAPPDATA%\\glucose\\resources\\mpv\\."
            .to_string()
    })?;

    let mut cmd = crate::create_hidden_command(&mpv);
    // --force-window guarantees a visible window even before the first frame
    // decodes; --title makes the spawned window identifiable as glucose Lite.
    cmd.arg("--force-window=yes")
        .arg("--title=glucose Lite — ${filename}")
        .arg(&video_path);

    cmd.spawn()
        .map(|_child| ())
        .map_err(|e| format!("Failed to launch mpv: {}", e))
}
