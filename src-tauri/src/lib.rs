mod pip_window;

use pip_window::{enter_pip_mode, exit_pip_mode, save_pip_window_layout, settle_pip_window};
use serde::Serialize;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::SystemTime;

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv",
];
const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "flac", "wav", "aac", "ogg", "opus", "m4a", "aiff", "wma",
];
fn is_media_extension(ext: &str) -> bool {
    VIDEO_EXTENSIONS.contains(&ext) || AUDIO_EXTENSIONS.contains(&ext)
}
use tauri::Emitter;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use tauri::RunEvent;

// Helper to create a Command with hidden console window on Windows
fn create_hidden_command(program: &str) -> Command {
    let mut cmd = Command::new(program);

    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }

    cmd
}

// Resolves the ffmpeg binary path: bundled AppData location first, then system PATH
fn get_ffmpeg_command() -> Command {
    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
            let ffmpeg_exe = std::path::Path::new(&app_data)
                .join("glucose")
                .join("resources")
                .join("ffmpeg")
                .join("bin")
                .join("ffmpeg.exe");
            if ffmpeg_exe.exists() {
                return create_hidden_command(ffmpeg_exe.to_str().unwrap_or("ffmpeg"));
            }
        }
    }

    create_hidden_command("ffmpeg")
}

fn get_ffprobe_command() -> Command {
    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
            let ffprobe_exe = std::path::Path::new(&app_data)
                .join("glucose")
                .join("resources")
                .join("ffmpeg")
                .join("bin")
                .join("ffprobe.exe");
            if ffprobe_exe.exists() {
                return create_hidden_command(ffprobe_exe.to_str().unwrap_or("ffprobe"));
            }
        }
    }

    create_hidden_command("ffprobe")
}

// Global state to store pending file paths
static PENDING_FILES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static FILE_PROCESSED: Mutex<bool> = Mutex::new(false);

// Serializes all config.json read-modify-write operations to prevent lost updates
pub(crate) static CONFIG_MUTEX: Mutex<()> = Mutex::new(());

// Configuration constants
const MAX_FILE_LOADING_ATTEMPTS: u32 = 30;
const FRONTEND_READY_WAIT_MS: u64 = 500;
const INITIAL_ATTEMPT_DELAY_MS: u64 = 2000;
const MIDDLE_ATTEMPT_DELAY_MS: u64 = 1000;
const FINAL_ATTEMPT_DELAY_MS: u64 = 500;
const INITIAL_ATTEMPT_COUNT: u32 = 5;
const MIDDLE_ATTEMPT_COUNT: u32 = 15;

fn default_gallery_paths() -> Vec<String> {
    let mut paths = Vec::new();
    for dir in [
        dirs::video_dir(),
        dirs::download_dir(),
        dirs::desktop_dir(),
        dirs::document_dir(),
    ] {
        if let Some(p) = dir {
            paths.push(p.to_string_lossy().to_string());
        }
    }
    paths
}

#[tauri::command]
fn get_gallery_paths() -> Result<Vec<String>, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_file = home.join(".glucose").join("config.json");

    if !config_file.exists() {
        return Ok(default_gallery_paths());
    }

    let content =
        fs::read_to_string(&config_file).map_err(|e| format!("Failed to read config: {}", e))?;
    let config: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;

    if let Some(arr) = config.get("gallery_paths").and_then(|v| v.as_array()) {
        let result: Vec<String> = arr
            .iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect();
        return Ok(result);
    }

    Ok(default_gallery_paths())
}

#[tauri::command]
fn save_gallery_paths(paths: Vec<String>) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let config_file = config_dir.join("config.json");

    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    let _guard = CONFIG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let mut config: serde_json::Value = if config_file.exists() {
        let content = fs::read_to_string(&config_file)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?
    } else {
        serde_json::json!({})
    };

    let config_object = config
        .as_object_mut()
        .ok_or_else(|| "Config root must be a JSON object".to_string())?;
    config_object.insert("gallery_paths".to_string(), serde_json::json!(paths));

    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    let temp_file = config_file.with_extension("json.tmp");
    fs::write(&temp_file, &content).map_err(|e| format!("Failed to write temp config: {}", e))?;
    fs::rename(&temp_file, &config_file).map_err(|e| format!("Failed to replace config: {}", e))?;

    Ok(())
}

#[tauri::command]
async fn open_folder_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = tokio::task::spawn_blocking(move || app.dialog().file().blocking_pick_folder())
        .await
        .map_err(|e| e.to_string())?;

    match result {
        Some(folder) => {
            let path_buf = folder.into_path().map_err(|e| e.to_string())?;
            Ok(Some(path_buf.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}

// Path sanitization function
fn sanitize_path(path: &str) -> String {
    let mut clean_path = path.trim().to_string();

    // Remove surrounding quotes if present
    if clean_path.starts_with('"') && clean_path.ends_with('"') {
        clean_path = clean_path[1..clean_path.len() - 1].to_string();
    }

    // Handle Windows UNC paths and long path names
    if clean_path.starts_with("\\\\?\\") {
        clean_path = clean_path[4..].to_string();
    }

    clean_path
}

#[tauri::command]
async fn open_file_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let all_exts: Vec<&'static str> = VIDEO_EXTENSIONS
        .iter()
        .chain(AUDIO_EXTENSIONS.iter())
        .copied()
        .collect();
    let result = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Media Files", &all_exts)
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    match result {
        Some(file) => {
            let path_buf = file.into_path().map_err(|e| e.to_string())?;
            let path = path_buf.to_string_lossy().to_string();
            Ok(Some(path))
        }
        None => Ok(None),
    }
}

#[tauri::command]
async fn open_subtitle_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("Subtitle Files", &["srt", "vtt", "ass", "ssa", "sub"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    match result {
        Some(file) => {
            let path_buf = file.into_path().map_err(|e| e.to_string())?;
            let path = path_buf.to_string_lossy().to_string();
            Ok(Some(path))
        }
        None => Ok(None),
    }
}

#[tauri::command]
fn convert_file_path(path: String) -> Result<String, String> {
    Ok(format!("https://asset.localhost/{}", path))
}

#[tauri::command]
fn get_pending_file() -> Option<String> {
    let mut pending = PENDING_FILES.lock().unwrap();
    pending.pop_front()
}

#[tauri::command]
fn mark_file_processed() {
    let mut processed = FILE_PROCESSED.lock().unwrap();
    *processed = true;
}

#[tauri::command]
fn frontend_ready(app_handle: tauri::AppHandle) -> Result<(), String> {
    #[cfg(debug_assertions)]
    println!("Frontend is ready to receive events");

    // Check if there are any pending files and emit them now
    let pending_files: Vec<String> = {
        let mut pending = PENDING_FILES.lock().unwrap();
        pending.drain(..).collect()
    };

    if !pending_files.is_empty() {
        #[cfg(debug_assertions)]
        println!(
            "Frontend ready - processing {} pending files",
            pending_files.len()
        );
        process_video_files(&app_handle, pending_files);
    }

    Ok(())
}

#[tauri::command]
fn exit_app(app_handle: tauri::AppHandle) {
    #[cfg(debug_assertions)]
    println!("Exit app command called");
    app_handle.exit(0);
}

#[tauri::command]
fn find_subtitle_for_video(video_path: String) -> Result<Option<String>, String> {
    use std::path::Path;

    let video_path_obj = Path::new(&video_path);
    let video_dir = video_path_obj
        .parent()
        .ok_or("Could not get video directory")?;
    let video_stem = video_path_obj
        .file_stem()
        .ok_or("Could not get video filename")?;
    let video_stem_str = video_stem.to_string_lossy();
    let video_stem_lower = video_stem_str.to_lowercase();

    // Subtitle extensions to check
    let subtitle_exts = vec!["srt", "vtt", "ass", "ssa", "sub"];

    // First try: exact case match (fastest, works on case-insensitive filesystems)
    for ext in &subtitle_exts {
        let subtitle_path = video_dir.join(format!("{}.{}", video_stem_str, ext));
        if subtitle_path.exists() {
            #[cfg(debug_assertions)]
            println!("Found subtitle file (exact match): {:?}", subtitle_path);
            return Ok(Some(subtitle_path.to_string_lossy().to_string()));
        }
    }

    // Second try: case-insensitive search (for case-sensitive filesystems)
    // Read directory entries and match case-insensitively
    if let Ok(entries) = fs::read_dir(video_dir) {
        for entry in entries.flatten() {
            let entry_path = entry.path();

            // Get the file stem and extension
            if let (Some(file_stem), Some(file_ext)) = (
                entry_path.file_stem().and_then(|s| s.to_str()),
                entry_path.extension().and_then(|s| s.to_str()),
            ) {
                let file_stem_lower = file_stem.to_lowercase();
                let file_ext_lower = file_ext.to_lowercase();

                // Check if stem matches (case-insensitive) and extension is a subtitle format
                if file_stem_lower == video_stem_lower
                    && subtitle_exts.contains(&file_ext_lower.as_str())
                {
                    #[cfg(debug_assertions)]
                    println!(
                        "Found subtitle file (case-insensitive match): {:?}",
                        entry_path
                    );
                    return Ok(Some(entry_path.to_string_lossy().to_string()));
                }
            }
        }
    }

    #[cfg(debug_assertions)]
    println!("No subtitle file found for video: {}", video_path);
    Ok(None)
}

async fn run_with_timeout(
    cmd: std::process::Command,
    timeout: std::time::Duration,
    label: &str,
) -> Result<std::process::Output, String> {
    let mut child = tokio::process::Command::from(cmd)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| format!("Failed to spawn {}: {}", label, e))?;

    let mut stdout = match child.stdout.take() {
        Some(s) => s,
        None => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(format!(
                "run_with_timeout: Failed to capture stdout for {}",
                label
            ));
        }
    };

    let mut stderr = match child.stderr.take() {
        Some(s) => s,
        None => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(format!(
                "run_with_timeout: Failed to capture stderr for {}",
                label
            ));
        }
    };

    let output_result = tokio::time::timeout(timeout, async {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let (status_res, out_res, err_res) = tokio::join!(
            child.wait(),
            tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut out),
            tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut err)
        );

        let status = match status_res {
            Ok(s) => s,
            Err(e) => return Err(format!("Failed to wait for {}: {}", label, e)),
        };
        if let Err(e) = out_res {
            return Err(format!("Failed to read stdout for {}: {}", label, e));
        }
        if let Err(e) = err_res {
            return Err(format!("Failed to read stderr for {}: {}", label, e));
        }

        Ok(std::process::Output {
            status,
            stdout: out,
            stderr: err,
        })
    })
    .await;

    match output_result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err(e)
        }
        Err(_) => {
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err(format!(
                "{} timed out after {} seconds",
                label,
                timeout.as_secs()
            ))
        }
    }
}

// Return the list of text-based subtitle streams embedded in a video file.
// Bitmap formats (PGS, VobSub, DVB) are silently skipped because they cannot
// be converted to a text format that the browser can render.
#[tauri::command]
async fn get_embedded_subtitle_tracks(
    video_path: String,
) -> Result<Vec<EmbeddedSubtitleTrack>, String> {
    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    const SUPPORTED: &[&str] = &["subrip", "ass", "ssa", "webvtt", "mov_text", "text"];

    let mut cmd = get_ffprobe_command();
    cmd.args([
        "-v",
        "error",
        "-select_streams",
        "s",
        "-show_entries",
        "stream=index,codec_name:stream_tags=language,title",
        "-of",
        "json",
        &video_path,
    ]);

    let output = run_with_timeout(cmd, TIMEOUT, "ffprobe").await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!(
                "Failed to parse ffprobe JSON: {} (stdout: {})",
                e, stdout
            ))
        }
    };

    let streams = match parsed["streams"].as_array() {
        Some(s) => s,
        None => return Err("ffprobe JSON missing 'streams' array".to_string()),
    };

    let mut tracks = Vec::new();
    for stream in streams {
        let codec_name = stream["codec_name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();

        if !SUPPORTED.contains(&codec_name.as_str()) {
            continue;
        }

        let Some(index) = stream["index"].as_i64() else {
            continue;
        };
        tracks.push(EmbeddedSubtitleTrack {
            index,
            codec_name,
            language: stream["tags"]["language"].as_str().map(|s| s.to_string()),
            title: stream["tags"]["title"].as_str().map(|s| s.to_string()),
        });
    }

    #[cfg(debug_assertions)]
    println!(
        "Found {} embedded subtitle track(s) in: {}",
        tracks.len(),
        video_path
    );

    Ok(tracks)
}

// Extract a single subtitle stream from a video file and return its content as
// an SRT string. FFmpeg handles codec conversion (e.g. ASS → SRT) automatically
// when the output format is forced to `srt`.  Sending output to `pipe:1` means
// no temp file is written to disk.
#[tauri::command]
async fn extract_embedded_subtitle(
    video_path: String,
    stream_index: i64,
) -> Result<String, String> {
    if stream_index < 0 {
        return Err(format!("Invalid stream index: {}", stream_index));
    }

    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    #[cfg(debug_assertions)]
    println!(
        "Extracting embedded subtitle stream {} from: {}",
        stream_index, video_path
    );

    let mut cmd = get_ffmpeg_command();
    cmd.args([
        "-v",
        "error",
        "-i",
        &video_path,
        "-map",
        &format!("0:{}", stream_index),
        "-f",
        "srt",
        "pipe:1",
    ]);

    let output = run_with_timeout(cmd, TIMEOUT, "ffmpeg").await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg failed to extract subtitle: {}", stderr));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[derive(Serialize, Clone)]
struct VideoFile {
    path: String,
    name: String,
    size: u64,
    modified: u64,
    duration: Option<f64>,
    is_cloud_only: bool,
}

#[cfg(target_os = "windows")]
fn is_cloud_only_file(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    let attrs = metadata.file_attributes();
    // Set by OneDrive, Google Drive for Desktop, and Dropbox for files not yet downloaded
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
    const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x40000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x400000;
    attrs
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
}

#[cfg(target_os = "macos")]
fn is_cloud_only_file(metadata: &fs::Metadata) -> bool {
    use std::os::darwin::fs::MetadataExt;
    const SF_DATALESS: u32 = 0x40000000;
    (metadata.st_flags() & SF_DATALESS) != 0
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
fn is_cloud_only_file(_metadata: &fs::Metadata) -> bool {
    false
}

fn is_cloud_only_path(path: &str) -> bool {
    fs::metadata(path)
        .map(|m| is_cloud_only_file(&m))
        .unwrap_or(false)
}

#[cfg(target_os = "windows")]
fn is_windows_hidden(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_HIDDEN: u32 = 0x2;
    metadata.file_attributes() & FILE_ATTRIBUTE_HIDDEN != 0
}

#[cfg(not(target_os = "windows"))]
fn is_windows_hidden(_metadata: &fs::Metadata) -> bool {
    false
}

#[derive(Serialize, Clone)]
struct SubtitleGenerationProgress {
    stage: String,
    progress: f32,
    message: String,
}

#[derive(Serialize, Clone)]
struct SetupStatus {
    ffmpeg_installed: bool,
    models_installed: Vec<String>,
    setup_completed: bool,
}

#[derive(Serialize, Clone)]
struct DownloadProgress {
    downloaded: u64,
    total: u64,
    percentage: f32,
    message: String,
}

#[derive(Serialize, serde::Deserialize, Clone)]
struct WatchProgress {
    path: String,
    current_time: f64,
    duration: f64,
    last_watched: u64,
}

#[derive(Serialize, Clone)]
struct ConversionProgress {
    stage: String,
    progress: f32,
    message: String,
}

#[derive(Serialize, Clone)]
struct VideoInfo {
    format: String,
    size_mb: f64,
}

#[derive(Serialize, Clone)]
struct EmbeddedSubtitleTrack {
    index: i64,
    codec_name: String,
    language: Option<String>,
    title: Option<String>,
}

#[derive(Serialize, Clone)]
struct EmbeddedAudioTrack {
    index: i64,
    codec_name: String,
    language: Option<String>,
    title: Option<String>,
    channels: Option<i64>,
    is_default: bool,
}

#[tauri::command]
async fn get_embedded_audio_tracks(video_path: String) -> Result<Vec<EmbeddedAudioTrack>, String> {
    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

    let mut cmd = get_ffprobe_command();
    cmd.args([
        "-v",
        "error",
        "-select_streams",
        "a",
        "-show_entries",
        "stream=index,codec_name,channels:stream_tags=language,title:stream_disposition=default",
        "-of",
        "json",
        &video_path,
    ]);

    let output = run_with_timeout(cmd, TIMEOUT, "ffprobe").await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("ffprobe failed: {}", stderr));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let parsed: serde_json::Value = match serde_json::from_str(&stdout) {
        Ok(v) => v,
        Err(e) => {
            return Err(format!(
                "Failed to parse ffprobe JSON: {} (stdout: {})",
                e, stdout
            ))
        }
    };

    let streams = match parsed["streams"].as_array() {
        Some(s) => s,
        None => return Err("ffprobe JSON missing 'streams' array".to_string()),
    };

    let mut tracks = Vec::new();
    for stream in streams {
        let Some(index) = stream["index"].as_i64() else {
            continue;
        };
        let codec_name = stream["codec_name"]
            .as_str()
            .unwrap_or("unknown")
            .to_string();
        let is_default = stream["disposition"]["default"].as_i64().unwrap_or(0) == 1;
        tracks.push(EmbeddedAudioTrack {
            index,
            codec_name,
            language: stream["tags"]["language"].as_str().map(|s| s.to_string()),
            title: stream["tags"]["title"].as_str().map(|s| s.to_string()),
            channels: stream["channels"].as_i64(),
            is_default,
        });
    }

    #[cfg(debug_assertions)]
    println!(
        "Found {} embedded audio track(s) in: {}",
        tracks.len(),
        video_path
    );

    Ok(tracks)
}

// Re-mux the video retaining only the selected audio stream (stream copy — no
// re-encoding).  Returns the path to a temp file that the frontend can load.
#[tauri::command]
async fn remux_with_audio_track(
    video_path: String,
    audio_stream_index: i64,
) -> Result<String, String> {
    if audio_stream_index < 0 {
        return Err(format!(
            "Invalid audio stream index: {}",
            audio_stream_index
        ));
    }

    let audio_tracks = get_embedded_audio_tracks(video_path.clone()).await?;
    if !audio_tracks
        .iter()
        .any(|track| track.index == audio_stream_index)
    {
        return Err(format!(
            "Stream index {} is not a valid audio track for this video",
            audio_stream_index
        ));
    }

    let temp_dir = std::env::temp_dir();
    let mut temp_path_opt: Option<std::path::PathBuf> = None;

    for i in 0..100 {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();

        let filename = format!(
            "glucose_audio_{}_{}.mkv",
            std::process::id(),
            timestamp + i as u128
        );
        let candidate_path = temp_dir.join(&filename);

        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate_path)
        {
            Ok(_) => {
                temp_path_opt = Some(candidate_path);
                break;
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(format!("Failed to create temporary file: {}", e)),
        }
    }

    let temp_path = temp_path_opt
        .ok_or_else(|| "Failed to generate a unique temporary file path".to_string())?;
    let temp_path_str = temp_path.to_string_lossy().to_string();
    let out = temp_path_str.clone();

    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(120);

    #[cfg(debug_assertions)]
    println!(
        "Remuxing audio stream {} from: {} -> {}",
        audio_stream_index, video_path, temp_path_str
    );

    let mut cmd = get_ffmpeg_command();
    cmd.args([
        "-v",
        "error",
        "-i",
        &video_path,
        "-map",
        "0:V?",
        "-map",
        &format!("0:{}", audio_stream_index),
        "-c",
        "copy",
        "-y",
        &temp_path_str,
    ]);

    let output_result = run_with_timeout(cmd, TIMEOUT, "ffmpeg").await;

    match output_result {
        Ok(output) => {
            if !output.status.success() {
                let stderr_str = String::from_utf8_lossy(&output.stderr);
                let _ = tokio::fs::remove_file(&temp_path).await;
                return Err(format!("FFmpeg remux failed: {}", stderr_str));
            }
        }
        Err(e) => {
            let _ = tokio::fs::remove_file(&temp_path).await;
            return Err(e);
        }
    }

    Ok(out)
}

// Delete a file that must reside in the system temp directory.
#[tauri::command]
async fn delete_temp_file(path: String) -> Result<(), String> {
    let target = std::path::Path::new(&path);

    let p = match target.canonicalize() {
        Ok(p) => p,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e.to_string()),
    };

    let temp_dir = std::env::temp_dir()
        .canonicalize()
        .map_err(|e| e.to_string())?;

    if !p.starts_with(&temp_dir) {
        return Err("Only files inside the system temp directory may be deleted".to_string());
    }
    let file_name = p
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;
    if !file_name.starts_with("glucose_audio_") || !file_name.ends_with(".mkv") {
        return Err("Only glucose_audio_*.mkv files may be deleted".to_string());
    }
    if let Err(e) = tokio::fs::remove_file(&p).await {
        if e.kind() != std::io::ErrorKind::NotFound {
            return Err(format!("Failed to delete temp file: {}", e));
        }
    }
    Ok(())
}

// Get video duration using FFmpeg
// Note: Caller should verify ffprobe is available before calling this
async fn get_video_duration(video_path: &str) -> Option<f64> {
    const TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);
    let mut cmd = get_ffprobe_command();
    cmd.args([
        "-v",
        "error",
        "-show_entries",
        "format=duration",
        "-of",
        "default=noprint_wrappers=1:nokey=1",
        video_path,
    ]);
    let output = run_with_timeout(cmd, TIMEOUT, "ffprobe").await.ok()?;
    if output.status.success() {
        let duration_str = String::from_utf8_lossy(&output.stdout);
        duration_str.trim().parse::<f64>().ok()
    } else {
        None
    }
}

// Check if FFmpeg is installed
#[tauri::command]
fn check_ffmpeg_installed() -> Result<bool, String> {
    // First check resources folder in AppData\Local
    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
            let ffmpeg_exe = std::path::Path::new(&app_data)
                .join("glucose")
                .join("resources")
                .join("ffmpeg")
                .join("bin")
                .join("ffmpeg.exe");
            let ffmpeg_path = ffmpeg_exe.to_string_lossy();
            println!("[FFmpeg Check] Checking AppData\\Local: {}", ffmpeg_path);
            if ffmpeg_exe.exists() {
                println!(
                    "[FFmpeg Check] ✓ Found FFmpeg in AppData\\Local: {}",
                    ffmpeg_path
                );
                return Ok(true);
            }
            println!("[FFmpeg Check] ✗ FFmpeg not found in AppData\\Local");
        }
    }

    // Fall back to PATH
    println!("[FFmpeg Check] Checking system PATH...");
    let result = create_hidden_command("ffmpeg")
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if result {
        println!("[FFmpeg Check] ✓ Found FFmpeg in system PATH");
    } else {
        println!("[FFmpeg Check] ✗ FFmpeg not found in system PATH");
    }
    Ok(result)
}

// Check which Whisper models are installed
#[tauri::command]
fn check_installed_models() -> Result<Vec<String>, String> {
    let mut models = Vec::new();
    let model_files = vec![
        ("ggml-tiny.bin", "tiny"),
        ("ggml-small.bin", "small"),
        ("ggml-large-v3-turbo-q5_0.bin", "large-v3-turbo"),
    ];

    println!("[Models Check] Starting model check...");

    // Check resources folder in AppData\Local first
    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
            let resources_models_dir = std::path::Path::new(&app_data)
                .join("glucose")
                .join("resources")
                .join("models");
            let resources_path = resources_models_dir.to_string_lossy();
            println!("[Models Check] Checking AppData\\Local: {}", resources_path);
            if resources_models_dir.exists() {
                println!("[Models Check] AppData\\Local models folder exists");
                for (filename, model_name) in &model_files {
                    let model_path = resources_models_dir.join(filename);
                    if model_path.exists() {
                        println!(
                            "[Models Check] ✓ Found {} in AppData\\Local: {}",
                            model_name, filename
                        );
                        models.push(model_name.to_string());
                    } else {
                        println!("[Models Check] ✗ {} not found in AppData\\Local", filename);
                    }
                }
            } else {
                println!("[Models Check] AppData\\Local models folder does not exist");
            }
        }
    }

    // Check home directory
    if let Some(home) = dirs::home_dir() {
        let models_dir = home.join(".whisper").join("models");
        let home_path = models_dir.to_string_lossy();
        println!("[Models Check] Checking home directory: {}", home_path);
        if models_dir.exists() {
            println!("[Models Check] Home models folder exists");
            for (filename, model_name) in &model_files {
                if models_dir.join(filename).exists() && !models.contains(&model_name.to_string()) {
                    println!(
                        "[Models Check] ✓ Found {} in home: {}",
                        model_name, filename
                    );
                    models.push(model_name.to_string());
                } else if !models.contains(&model_name.to_string()) {
                    println!("[Models Check] ✗ {} not found in home", filename);
                }
            }
        } else {
            println!("[Models Check] Home models folder does not exist");
        }
    }

    println!("[Models Check] Complete. Found models: {:?}", models);
    Ok(models)
}

// Get setup status
#[tauri::command]
fn get_setup_status() -> Result<SetupStatus, String> {
    let ffmpeg_installed = check_ffmpeg_installed()?;
    let models_installed = check_installed_models()?;

    // Check if setup was completed (stored in config)
    let setup_completed = load_setup_completed()?;

    Ok(SetupStatus {
        ffmpeg_installed,
        models_installed,
        setup_completed,
    })
}

// Save setup completion status
#[tauri::command]
fn mark_setup_completed() -> Result<(), String> {
    save_setup_completed(true)
}

// Load setup completion status from config
fn load_setup_completed() -> Result<bool, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let config_file = config_dir.join("config.json");

    if !config_file.exists() {
        return Ok(false);
    }

    let content =
        fs::read_to_string(config_file).map_err(|e| format!("Failed to read config: {}", e))?;

    let config: serde_json::Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;

    Ok(config
        .get("setup_completed")
        .and_then(|v| v.as_bool())
        .unwrap_or(false))
}

// Save setup completion status to config
fn save_setup_completed(completed: bool) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let config_file = config_dir.join("config.json");

    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    let _guard = CONFIG_MUTEX.lock().unwrap_or_else(|e| e.into_inner());

    let mut config: serde_json::Value = if config_file.exists() {
        let content = fs::read_to_string(&config_file)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?
    } else {
        serde_json::json!({})
    };

    let config_object = config
        .as_object_mut()
        .ok_or_else(|| "Config root must be a JSON object".to_string())?;
    config_object.insert("setup_completed".to_string(), serde_json::json!(completed));

    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    let temp_file = config_file.with_extension("json.tmp");
    fs::write(&temp_file, &content).map_err(|e| format!("Failed to write temp config: {}", e))?;
    fs::rename(&temp_file, &config_file).map_err(|e| format!("Failed to replace config: {}", e))?;

    Ok(())
}

// Download Whisper model
#[tauri::command]
async fn download_whisper_model(
    app_handle: tauri::AppHandle,
    model_size: String,
) -> Result<String, String> {
    let model_name = match model_size.as_str() {
        "tiny" => "ggml-tiny.bin",
        "small" => "ggml-small.bin",
        "large-v3-turbo" => "ggml-large-v3-turbo-q5_0.bin",
        _ => return Err("Invalid model size".to_string()),
    };

    let url = format!(
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
        model_name
    );

    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let models_dir = home.join(".whisper").join("models");

    // Create models directory
    fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create models directory: {}", e))?;

    let output_path = models_dir.join(model_name);

    // Download file with progress
    download_file_with_progress(
        &app_handle,
        &url,
        &output_path,
        &format!("Downloading {} model", model_size),
    )
    .await?;

    Ok(output_path.to_string_lossy().to_string())
}

// Download file with progress reporting
async fn download_file_with_progress(
    app_handle: &tauri::AppHandle,
    url: &str,
    output_path: &std::path::PathBuf,
    message: &str,
) -> Result<(), String> {
    use futures_util::StreamExt;
    use std::io::Write;

    let client = reqwest::Client::new();
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Failed to start download: {}", e))?;

    // Abort early on non-2xx statuses; avoid writing error pages to disk
    if !response.status().is_success() {
        let status = response.status();
        let ct = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");

        // For textual responses, include a short snippet to help debugging
        let snippet = if ct.contains("text")
            || ct.contains("json")
            || ct.contains("xml")
            || ct.contains("html")
        {
            match response.text().await {
                Ok(t) => t.chars().take(512).collect::<String>(),
                Err(_) => String::new(),
            }
        } else {
            String::new()
        };

        return if snippet.is_empty() {
            Err(format!("HTTP error {} {}", status.as_u16(), status))
        } else {
            Err(format!(
                "HTTP error {} {}: {}",
                status.as_u16(),
                status,
                snippet
            ))
        };
    }

    // Success: only now inspect content length, create the file, and stream bytes
    let total_size = response.content_length().unwrap_or(0);

    let mut file =
        fs::File::create(output_path).map_err(|e| format!("Failed to create file: {}", e))?;

    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| format!("Download error: {}", e))?;
        file.write_all(&chunk)
            .map_err(|e| format!("Failed to write to file: {}", e))?;

        downloaded += chunk.len() as u64;
        let percentage = if total_size > 0 {
            (downloaded as f32 / total_size as f32) * 100.0
        } else {
            0.0
        };

        // Emit progress every 1MB or so
        if downloaded % (1024 * 1024) < chunk.len() as u64 || downloaded == total_size {
            let _ = app_handle.emit(
                "download-progress",
                DownloadProgress {
                    downloaded,
                    total: total_size,
                    percentage,
                    message: message.to_string(),
                },
            );
        }
    }

    Ok(())
}

// Helper function to extract audio from video using FFmpeg
fn extract_audio_from_video(video_path: &str, output_audio_path: &str) -> Result<(), String> {
    #[cfg(debug_assertions)]
    println!("Extracting audio from video: {}", video_path);

    let output = get_ffmpeg_command()
        .args([
            "-i",
            video_path,
            "-vn", // No video
            "-acodec",
            "pcm_s16le", // PCM 16-bit little-endian
            "-ar",
            "16000", // Sample rate 16kHz (Whisper's expected rate)
            "-ac",
            "1",  // Mono channel
            "-y", // Overwrite output file
            output_audio_path,
        ])
        .output()
        .map_err(|e| {
            format!(
                "Failed to execute ffmpeg: {}. Make sure FFmpeg is installed and in PATH.",
                e
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg failed: {}", stderr));
    }

    #[cfg(debug_assertions)]
    println!("Audio extracted successfully to: {}", output_audio_path);
    Ok(())
}

// Helper function to format time for SRT subtitles (HH:MM:SS,mmm)
fn format_srt_time(seconds: f64) -> String {
    // Round to total milliseconds first to avoid cases like millis == 1000
    let total_millis = (seconds * 1000.0).round() as u64;
    let hours = total_millis / 3_600_000;
    let minutes = (total_millis % 3_600_000) / 60_000;
    let secs = (total_millis % 60_000) / 1000;
    let millis = total_millis % 1000;
    format!("{:02}:{:02}:{:02},{:03}", hours, minutes, secs, millis)
}

// Generate SRT subtitle file from Whisper segments
fn generate_srt_from_segments(
    segments: Vec<(f64, f64, String)>,
    output_path: &str,
) -> Result<(), String> {
    let mut srt_content = String::new();

    for (index, (start, end, text)) in segments.iter().enumerate() {
        srt_content.push_str(&format!("{}\n", index + 1));
        srt_content.push_str(&format!(
            "{} --> {}\n",
            format_srt_time(*start),
            format_srt_time(*end)
        ));
        srt_content.push_str(&format!("{}\n\n", text.trim()));
    }

    fs::write(output_path, srt_content).map_err(|e| format!("Failed to write SRT file: {}", e))?;

    #[cfg(debug_assertions)]
    println!("SRT file generated: {}", output_path);
    Ok(())
}

#[tauri::command]
async fn generate_subtitles(
    app_handle: tauri::AppHandle,
    video_path: String,
    model_size: String,
    language: String,
) -> Result<String, String> {
    #[cfg(debug_assertions)]
    {
        println!("Starting subtitle generation for: {}", video_path);
        println!("Model size: {}", model_size);
    }

    // Emit initial progress
    let _ = app_handle.emit(
        "subtitle-generation-progress",
        SubtitleGenerationProgress {
            stage: "initializing".to_string(),
            progress: 0.0,
            message: "Initializing subtitle generation...".to_string(),
        },
    );

    let video_path_obj = Path::new(&video_path);
    let video_dir = video_path_obj
        .parent()
        .ok_or("Could not get video directory")?;
    let video_stem = video_path_obj
        .file_stem()
        .ok_or("Could not get video filename")?;

    // Create temporary audio file path
    let temp_audio_path =
        video_dir.join(format!("{}_temp_audio.wav", video_stem.to_string_lossy()));
    let temp_audio_str = temp_audio_path.to_string_lossy().to_string();

    // Output subtitle path
    let subtitle_path = video_dir.join(format!("{}.srt", video_stem.to_string_lossy()));
    let subtitle_path_str = subtitle_path.to_string_lossy().to_string();

    // Step 1: Extract audio from video
    let _ = app_handle.emit(
        "subtitle-generation-progress",
        SubtitleGenerationProgress {
            stage: "extracting_audio".to_string(),
            progress: 10.0,
            message: "Extracting audio from video...".to_string(),
        },
    );

    extract_audio_from_video(&video_path, &temp_audio_str)?;

    // Step 2: Load Whisper model
    let _ = app_handle.emit(
        "subtitle-generation-progress",
        SubtitleGenerationProgress {
            stage: "loading_model".to_string(),
            progress: 30.0,
            message: format!("Loading Whisper {} model...", model_size),
        },
    );

    // Get model path from user's home directory or use default location
    let model_name = match model_size.as_str() {
        "tiny" => "ggml-tiny.bin",
        "small" => "ggml-small.bin",
        "large-v3-turbo" => "ggml-large-v3-turbo-q5_0.bin",
        _ => "ggml-tiny.bin",
    };

    let model_path = dirs::home_dir().ok_or("Could not find home directory")?;
    let model_path = model_path.join(".whisper").join("models").join(model_name);

    if !model_path.exists() {
        let error_msg = format!(
            "Whisper model not found at: {}\n\nPlease download the model first. You can download it from:\nhttps://huggingface.co/ggerganov/whisper.cpp/tree/main\n\nPlace it in: {}",
            model_path.display(),
            model_path.parent().unwrap().display()
        );

        // Clean up temp audio file
        let _ = fs::remove_file(&temp_audio_str);

        let _ = app_handle.emit(
            "subtitle-generation-progress",
            SubtitleGenerationProgress {
                stage: "error".to_string(),
                progress: 0.0,
                message: error_msg.clone(),
            },
        );

        return Err(error_msg);
    }

    // Step 3: Transcribe audio with Whisper
    let _ = app_handle.emit(
        "subtitle-generation-progress",
        SubtitleGenerationProgress {
            stage: "transcribing".to_string(),
            progress: 50.0,
            message: "Transcribing audio with AI...".to_string(),
        },
    );

    // Run Whisper transcription in a separate thread to avoid blocking
    let model_path_clone = model_path.clone();
    let temp_audio_clone = temp_audio_str.clone();
    let subtitle_path_clone = subtitle_path_str.clone();
    let app_handle_clone = app_handle.clone();
    let language_clone = language.clone();

    tokio::task::spawn_blocking(move || {
        transcribe_audio_with_whisper(
            &model_path_clone.to_string_lossy(),
            &temp_audio_clone,
            &subtitle_path_clone,
            &app_handle_clone,
            &language_clone,
        )
    })
    .await
    .map_err(|e| format!("Transcription task failed: {}", e))??;

    // Clean up temporary audio file
    let _ = fs::remove_file(&temp_audio_str);

    // Step 4: Complete
    let _ = app_handle.emit(
        "subtitle-generation-progress",
        SubtitleGenerationProgress {
            stage: "complete".to_string(),
            progress: 100.0,
            message: "Subtitles generated successfully!".to_string(),
        },
    );

    Ok(subtitle_path_str)
}

// Transcribe audio using whisper-rs
fn transcribe_audio_with_whisper(
    model_path: &str,
    audio_path: &str,
    output_subtitle_path: &str,
    app_handle: &tauri::AppHandle,
    language: &str,
) -> Result<(), String> {
    use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

    #[cfg(debug_assertions)]
    println!("Loading Whisper model from: {}", model_path);

    let ctx = WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
        .map_err(|e| format!("Failed to load Whisper model: {}", e))?;

    #[cfg(debug_assertions)]
    println!("Reading audio file: {}", audio_path);

    // Read audio file as samples
    let audio_data = read_wav_file(audio_path)?;

    #[cfg(debug_assertions)]
    println!("Starting transcription... ({} samples)", audio_data.len());

    // Create transcription parameters
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    // Configure parameters for better subtitle generation
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(true);
    params.set_translate(false); // Don't translate, keep original language
    params.set_language(Some(language)); // Use selected language
    params.set_max_len(0); // Disable max length limit per segment
    params.set_split_on_word(true); // Split on word boundaries

    // Run transcription
    let mut state = ctx
        .create_state()
        .map_err(|e| format!("Failed to create Whisper state: {}", e))?;

    state
        .full(params, &audio_data)
        .map_err(|e| format!("Transcription failed: {}", e))?;

    #[cfg(debug_assertions)]
    println!("Transcription complete, extracting segments...");

    // Extract segments with timestamps
    let num_segments = state.full_n_segments();

    #[cfg(debug_assertions)]
    println!("Found {} segments", num_segments);

    let mut segments = Vec::new();

    for i in 0..num_segments {
        // 1. Fetch the segment object for this loop iteration
        // USING .ok_or_else() INSTEAD OF .map_err() because get_segment returns an Option
        let segment = state
            .get_segment(i)
            .ok_or_else(|| format!("Failed to get segment {}", i))?;

        // 2. Grab the timestamps from the object
        let start_timestamp = segment.start_timestamp();
        let end_timestamp = segment.end_timestamp();

        // 3. Grab the text
        let text = segment
            .to_str()
            .map_err(|e| format!("Failed to parse segment text: {}", e))?
            .to_string();

        // Convert from Whisper's timestamp units (10ms) to seconds
        let start_seconds = start_timestamp as f64 / 100.0;
        let end_seconds = end_timestamp as f64 / 100.0;

        if !text.trim().is_empty() {
            segments.push((start_seconds, end_seconds, text));
        }

        // Emit progress periodically
        if i % 10 == 0 {
            let progress = 50.0 + (i as f32 / num_segments as f32) * 40.0;
            let _ = app_handle.emit(
                "subtitle-generation-progress",
                SubtitleGenerationProgress {
                    stage: "transcribing".to_string(),
                    progress,
                    message: format!("Processing segment {} of {}...", i + 1, num_segments),
                },
            );
        }
    }

    #[cfg(debug_assertions)]
    println!("Generating SRT file with {} segments...", segments.len());

    // Generate SRT file
    generate_srt_from_segments(segments, output_subtitle_path)?;

    Ok(())
}

// Read WAV file and convert to f32 samples for Whisper
fn read_wav_file(path: &str) -> Result<Vec<f32>, String> {
    use std::fs::File;
    use std::io::Read;

    let mut file = File::open(path).map_err(|e| format!("Failed to open audio file: {}", e))?;

    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer)
        .map_err(|e| format!("Failed to read audio file: {}", e))?;

    // Validate buffer has at least 44 bytes (standard WAV header size)
    if buffer.len() < 44 {
        return Err(format!(
            "Invalid or truncated WAV file: expected at least 44 bytes for header, got {} bytes",
            buffer.len()
        ));
    }

    // Skip WAV header (44 bytes for standard WAV)
    let audio_data = &buffer[44..];

    // Validate audio data has at least 2 bytes for one sample
    if audio_data.len() < 2 {
        return Err(format!(
            "Invalid or truncated WAV file: audio data is too short ({} bytes)",
            audio_data.len()
        ));
    }

    // Validate audio data length is even (complete 16-bit samples)
    if audio_data.len() % 2 != 0 {
        return Err(format!(
            "Invalid WAV file: audio data length ({} bytes) is not even, cannot parse complete 16-bit samples",
            audio_data.len()
        ));
    }

    // Convert 16-bit PCM to f32 samples (-1.0 to 1.0)
    let samples: Vec<f32> = audio_data
        .chunks_exact(2)
        .map(|chunk| {
            let sample = i16::from_le_bytes([chunk[0], chunk[1]]);
            sample as f32 / 32768.0
        })
        .collect();

    Ok(samples)
}

// Save watch progress for a video
#[tauri::command]
fn save_watch_progress(video_path: String, current_time: f64, duration: f64) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let progress_file = config_dir.join("watch_progress.json");

    // Create config directory if it doesn't exist
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    // Load existing progress data or create new
    let mut progress_map: std::collections::HashMap<String, WatchProgress> =
        if progress_file.exists() {
            let content = fs::read_to_string(&progress_file)
                .map_err(|e| format!("Failed to read progress file: {}", e))?;
            serde_json::from_str(&content).unwrap_or_else(|_| std::collections::HashMap::new())
        } else {
            std::collections::HashMap::new()
        };

    // Get current time as Unix timestamp
    let last_watched = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Failed to get current time: {}", e))?
        .as_secs();

    // Update or insert progress for this video
    progress_map.insert(
        video_path.clone(),
        WatchProgress {
            path: video_path,
            current_time,
            duration,
            last_watched,
        },
    );

    // Save to file
    let content = serde_json::to_string_pretty(&progress_map)
        .map_err(|e| format!("Failed to serialize progress: {}", e))?;

    fs::write(progress_file, content)
        .map_err(|e| format!("Failed to write progress file: {}", e))?;

    Ok(())
}

// Get watch progress for a video
#[tauri::command]
fn get_watch_progress(video_path: String) -> Result<Option<WatchProgress>, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let progress_file = config_dir.join("watch_progress.json");

    if !progress_file.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(&progress_file)
        .map_err(|e| format!("Failed to read progress file: {}", e))?;

    let progress_map: std::collections::HashMap<String, WatchProgress> =
        serde_json::from_str(&content).unwrap_or_else(|_| std::collections::HashMap::new());

    Ok(progress_map.get(&video_path).cloned())
}

// Get video file info
#[tauri::command]
fn get_video_info(video_path: String) -> Result<VideoInfo, String> {
    let path = Path::new(&video_path);

    // Get file size
    let metadata = fs::metadata(path).map_err(|e| format!("Failed to get file metadata: {}", e))?;
    let size_bytes = metadata.len();
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);

    // Get format from extension
    let format = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("unknown")
        .to_uppercase();

    Ok(VideoInfo { format, size_mb })
}

// Estimate converted file size (rough estimation)
#[allow(dead_code)]
fn estimate_converted_size(original_size_mb: f64, target_format: &str) -> f64 {
    // Simple compression ratio estimates
    match target_format {
        "mp4" => original_size_mb * 0.85,  // H.264 usually ~85% of original
        "webm" => original_size_mb * 0.70, // VP9 usually ~70% of original
        "mkv" => original_size_mb * 0.90,  // MKV container, minimal change
        _ => original_size_mb,
    }
}

// Convert video to different format
#[tauri::command]
async fn convert_video(
    app_handle: tauri::AppHandle,
    video_path: String,
    target_format: String,
) -> Result<String, String> {
    let video_path_obj = Path::new(&video_path);
    let video_dir = video_path_obj
        .parent()
        .ok_or("Could not get video directory")?;
    let video_stem = video_path_obj
        .file_stem()
        .ok_or("Could not get video filename")?;

    // Output path
    let output_path = video_dir.join(format!(
        "{}_converted.{}",
        video_stem.to_string_lossy(),
        target_format
    ));
    let output_path_str = output_path.to_string_lossy().to_string();

    // Emit initial progress
    let _ = app_handle.emit(
        "conversion-progress",
        ConversionProgress {
            stage: "starting".to_string(),
            progress: 0.0,
            message: format!("Starting conversion to {}...", target_format.to_uppercase()),
        },
    );

    // Run conversion in blocking task
    let video_path_clone = video_path.clone();
    let output_path_clone = output_path_str.clone();
    let target_format_clone = target_format.clone();
    let app_handle_clone = app_handle.clone();

    tokio::task::spawn_blocking(move || {
        convert_video_with_ffmpeg(
            &video_path_clone,
            &output_path_clone,
            &target_format_clone,
            &app_handle_clone,
        )
    })
    .await
    .map_err(|e| format!("Conversion task failed: {}", e))??;

    // Emit completion
    let _ = app_handle.emit(
        "conversion-progress",
        ConversionProgress {
            stage: "complete".to_string(),
            progress: 100.0,
            message: "Conversion complete!".to_string(),
        },
    );

    Ok(output_path_str)
}

// Convert video using FFmpeg
fn convert_video_with_ffmpeg(
    input_path: &str,
    output_path: &str,
    target_format: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    let _ = app_handle.emit(
        "conversion-progress",
        ConversionProgress {
            stage: "converting".to_string(),
            progress: 50.0,
            message: format!("Converting to {}...", target_format.to_uppercase()),
        },
    );

    // Build FFmpeg command based on target format
    let mut cmd = get_ffmpeg_command();
    cmd.arg("-i").arg(input_path);

    match target_format {
        "mp4" => {
            cmd.args([
                "-c:v", "libx264", "-preset", "medium", "-crf", "23", "-c:a", "aac", "-b:a", "192k",
            ]);
        }
        "webm" => {
            cmd.args([
                "-c:v",
                "libvpx-vp9",
                "-crf",
                "30",
                "-b:v",
                "0",
                "-c:a",
                "libopus",
            ]);
        }
        "mkv" => {
            cmd.args(["-c:v", "copy", "-c:a", "copy"]); // Just remux, no re-encoding
        }
        _ => return Err(format!("Unsupported format: {}", target_format)),
    }

    cmd.arg("-y").arg(output_path);

    let output = cmd.output().map_err(|e| {
        format!(
            "Failed to execute ffmpeg: {}. Make sure FFmpeg is installed.",
            e
        )
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("FFmpeg conversion failed: {}", stderr));
    }

    Ok(())
}

// Get all watch progress data
#[tauri::command]
fn get_all_watch_progress() -> Result<std::collections::HashMap<String, WatchProgress>, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let progress_file = config_dir.join("watch_progress.json");

    if !progress_file.exists() {
        return Ok(std::collections::HashMap::new());
    }

    let content = fs::read_to_string(&progress_file)
        .map_err(|e| format!("Failed to read progress file: {}", e))?;

    let progress_map: std::collections::HashMap<String, WatchProgress> =
        serde_json::from_str(&content).unwrap_or_else(|_| std::collections::HashMap::new());

    Ok(progress_map)
}

fn scan_dir_for_media(dir: &Path, videos: &mut Vec<VideoFile>, depth: u32) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        // Skip symlinks: avoids loops for directories and duplicates for files
        if file_type.is_symlink() {
            continue;
        }
        let path = entry.path();
        if file_type.is_dir() {
            // Skip Windows hidden directories (e.g. system/app folders)
            #[cfg(target_os = "windows")]
            if entry
                .metadata()
                .map(|m| is_windows_hidden(&m))
                .unwrap_or(false)
            {
                continue;
            }
            scan_dir_for_media(&path, videos, depth - 1);
        } else if file_type.is_file() {
            let ext = match path.extension().and_then(|e| e.to_str()) {
                Some(e) => e.to_lowercase(),
                None => continue,
            };
            if !is_media_extension(&ext) {
                continue;
            }
            // Fetch metadata only after confirming it's a regular file
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if is_windows_hidden(&metadata) {
                continue;
            }
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let Ok(mod_secs) = modified.duration_since(SystemTime::UNIX_EPOCH) else {
                continue;
            };
            videos.push(VideoFile {
                path: path.to_string_lossy().to_string(),
                name: name.to_string_lossy().to_string(),
                size: metadata.len(),
                modified: mod_secs.as_secs(),
                duration: None,
                is_cloud_only: is_cloud_only_file(&metadata),
            });
        }
    }
}

#[tauri::command]
async fn get_recent_videos() -> Result<Vec<VideoFile>, String> {
    let search_dirs: Vec<std::path::PathBuf> = get_gallery_paths()?
        .into_iter()
        .map(std::path::PathBuf::from)
        .collect();

    let handles: Vec<_> = search_dirs
        .into_iter()
        .map(|dir| {
            tokio::task::spawn_blocking(move || {
                let mut videos = Vec::new();
                scan_dir_for_media(&dir, &mut videos, 6);
                videos
            })
        })
        .collect();

    let mut videos = Vec::new();
    for handle in handles {
        videos.append(&mut handle.await.map_err(|e| e.to_string())?);
    }

    let mut seen = std::collections::HashSet::new();
    videos.retain(|v| seen.insert(v.path.clone()));
    videos.sort_by(|a, b| b.modified.cmp(&a.modified));
    videos.truncate(100);

    Ok(videos)
}

#[derive(Serialize, Clone)]
struct VideoDurationUpdate {
    path: String,
    duration: Option<f64>,
}

#[tauri::command]
async fn fetch_video_durations(
    app_handle: tauri::AppHandle,
    paths: Vec<String>,
) -> Result<(), String> {
    tokio::spawn(async move {
        let ffprobe_available = tokio::task::spawn_blocking(|| {
            get_ffprobe_command()
                .arg("-version")
                .output()
                .map(|o| o.status.success())
                .unwrap_or(false)
        })
        .await
        .unwrap_or(false);

        if !ffprobe_available {
            let _ = app_handle.emit("ffprobe-unavailable", ());
            return;
        }

        for path in paths {
            let duration = if is_cloud_only_path(&path) {
                None
            } else {
                get_video_duration(&path).await
            };
            let _ = app_handle.emit(
                "video-duration-ready",
                VideoDurationUpdate { path, duration },
            );
        }
    });
    Ok(())
}

// Function to process video files and emit events
fn process_video_files(app_handle: &tauri::AppHandle, video_files: Vec<String>) {
    if !video_files.is_empty() {
        // Store in global queue
        {
            let mut pending = PENDING_FILES.lock().unwrap();
            for video_file in &video_files {
                pending.push_back(video_file.clone());
            }
        }

        // Spawn background thread for persistent file loading attempts
        let app_handle_clone = app_handle.clone();
        thread::spawn(move || {
            for attempt in 1..=MAX_FILE_LOADING_ATTEMPTS {
                // Check if file has been processed
                {
                    let processed = FILE_PROCESSED.lock().unwrap();
                    if *processed {
                        #[cfg(debug_assertions)]
                        println!("File already processed, stopping attempts");
                        break;
                    }
                }

                // Try to emit event for first video file immediately on first attempt
                if let Some(video_file) = video_files.first() {
                    #[cfg(debug_assertions)]
                    match app_handle_clone.emit("open-file", video_file) {
                        Ok(_) => {
                            println!(
                                "Attempt {}: Successfully emitted open-file for {}",
                                attempt, video_file
                            );
                        }
                        Err(e) => {
                            println!("Attempt {}: Failed to emit event: {:?}", attempt, e);
                        }
                    }
                    #[cfg(not(debug_assertions))]
                    let _ = app_handle_clone.emit("open-file", video_file);
                }

                // Wait before next attempt (no delay before first attempt)
                if attempt < MAX_FILE_LOADING_ATTEMPTS {
                    let delay = if attempt == 1 {
                        // First retry: wait for frontend to be fully ready
                        Duration::from_millis(FRONTEND_READY_WAIT_MS)
                    } else if attempt <= INITIAL_ATTEMPT_COUNT {
                        Duration::from_millis(INITIAL_ATTEMPT_DELAY_MS)
                    } else if attempt <= MIDDLE_ATTEMPT_COUNT {
                        Duration::from_millis(MIDDLE_ATTEMPT_DELAY_MS)
                    } else {
                        Duration::from_millis(FINAL_ATTEMPT_DELAY_MS)
                    };

                    thread::sleep(delay);
                }
            }

            #[cfg(debug_assertions)]
            println!("File loading attempts completed");
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init());

    // Add updater plugin only on Windows
    #[cfg(target_os = "windows")]
    {
        builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    }

    builder
        .setup(|app| {
            // Handle command line arguments for file associations
            let args: Vec<String> = std::env::args().collect();

            #[cfg(debug_assertions)]
            {
                println!("=== GLUCOSE STARTUP ===");
                println!("Command line arguments: {:?}", args);
            }

            // Check if we're being launched via file association
            if args.len() > 1 {
                #[cfg(debug_assertions)]
                println!("*** LAUNCHED WITH ARGUMENTS - POTENTIAL FILE ASSOCIATION ***");

                let mut video_files: Vec<String> = Vec::new();

                for arg in &args[1..] {
                    // Sanitize the path
                    let clean_arg = sanitize_path(arg);
                    #[cfg(debug_assertions)]
                    println!("Processing argument: {} -> {}", arg, clean_arg);

                    let is_media = std::path::Path::new(&clean_arg)
                        .extension()
                        .and_then(|s| s.to_str())
                        .map(|ext| is_media_extension(&ext.to_lowercase()))
                        .unwrap_or(false);
                    if is_media {
                        video_files.push(clean_arg.clone());
                        #[cfg(debug_assertions)]
                        println!("Found video file: {}", clean_arg);
                    }
                }

                if !video_files.is_empty() {
                    #[cfg(debug_assertions)]
                    println!("Queued {} video files", video_files.len());
                    process_video_files(&app.handle(), video_files);
                }
            } else {
                #[cfg(debug_assertions)]
                println!("*** LAUNCHED WITHOUT ARGUMENTS - NORMAL APP LAUNCH ***");
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_file_dialog,
            open_folder_dialog,
            open_subtitle_dialog,
            get_gallery_paths,
            save_gallery_paths,
            convert_file_path,
            get_recent_videos,
            fetch_video_durations,
            get_pending_file,
            mark_file_processed,
            frontend_ready,
            exit_app,
            find_subtitle_for_video,
            get_embedded_subtitle_tracks,
            extract_embedded_subtitle,
            get_embedded_audio_tracks,
            remux_with_audio_track,
            delete_temp_file,
            generate_subtitles,
            check_ffmpeg_installed,
            check_installed_models,
            get_setup_status,
            mark_setup_completed,
            download_whisper_model,
            save_watch_progress,
            get_watch_progress,
            get_all_watch_progress,
            get_video_info,
            convert_video,
            enter_pip_mode,
            exit_pip_mode,
            save_pip_window_layout,
            settle_pip_window
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app_handle, event| {
            // Log events for debugging
            if cfg!(debug_assertions) {
                println!("Received event: {:?}", event);
            }

            #[cfg(any(target_os = "macos", target_os = "ios"))]
            match event {
                // Handle macOS file association events
                RunEvent::Opened { urls } => {
                    #[cfg(debug_assertions)]
                    {
                        println!("*** FILE ASSOCIATION EVENT RECEIVED ***");
                        println!("Received opened event with URLs: {:?}", urls);
                    }

                    let mut video_files: Vec<String> = Vec::new();

                    for url in urls {
                        let url_str = url.to_string();
                        #[cfg(debug_assertions)]
                        println!("Processing URL: {}", url_str);

                        if url_str.starts_with("file://") {
                            let path = url_str.replace("file://", "");
                            let decoded_path = urlencoding::decode(&path).unwrap_or_default();

                            #[cfg(debug_assertions)]
                            println!("Decoded path: {}", decoded_path);

                            let is_media = Path::new(decoded_path.as_ref())
                                .extension()
                                .and_then(|s| s.to_str())
                                .map(|ext| is_media_extension(&ext.to_lowercase()))
                                .unwrap_or(false);
                            if is_media {
                                video_files.push(decoded_path.to_string());
                                #[cfg(debug_assertions)]
                                println!("Found video file from opened event: {}", decoded_path);
                            }
                        }
                    }

                    if !video_files.is_empty() {
                        #[cfg(debug_assertions)]
                        println!(
                            "Processing {} video files from file association event",
                            video_files.len()
                        );
                        process_video_files(&_app_handle, video_files);
                    }
                }
                _ => {}
            }
        });
}
