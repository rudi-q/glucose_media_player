use tauri::{Emitter, Manager};
#[cfg(any(target_os = "macos", target_os = "ios"))]
use tauri::RunEvent;
use std::fs;
use std::time::SystemTime;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use serde::{Serialize, Deserialize};
use std::path::Path;
use std::process::Command;
use tauri::{PhysicalPosition, PhysicalSize};
use anyhow::anyhow;

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

// Global state to store pending file paths
static PENDING_FILES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static FILE_PROCESSED: Mutex<bool> = Mutex::new(false);

// PiP window state storage
#[derive(Clone, Debug)]
struct WindowState {
    size: PhysicalSize<u32>,
    position: PhysicalPosition<i32>,
    decorations_enabled: bool,
    pip_active: bool,
}

static WINDOW_STATE: Mutex<Option<WindowState>> = Mutex::new(None);

// Configuration constants
const MAX_FILE_LOADING_ATTEMPTS: u32 = 30;
const FRONTEND_READY_WAIT_MS: u64 = 500;
const INITIAL_ATTEMPT_DELAY_MS: u64 = 2000;
const MIDDLE_ATTEMPT_DELAY_MS: u64 = 1000;
const FINAL_ATTEMPT_DELAY_MS: u64 = 500;
const INITIAL_ATTEMPT_COUNT: u32 = 5;
const MIDDLE_ATTEMPT_COUNT: u32 = 15;

// PiP window configuration from constants.json
#[derive(Deserialize)]
struct PipWindowConfig {
    width: u32,
    height: u32,
    padding: i32,
}

#[derive(Deserialize)]
struct AppConstants {
    #[serde(rename = "pipWindow")]
    pip_window: PipWindowConfig,
}

fn get_pip_constants() -> Result<PipWindowConfig, anyhow::Error> {
    const CONSTANTS_JSON: &str = include_str!("../../constants.json");
    let constants: AppConstants = serde_json::from_str(CONSTANTS_JSON)
        .map_err(|e| anyhow!("Failed to parse constants.json: {}", e))?;
    Ok(constants.pip_window)
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
    
    let file_path = app.dialog()
        .file()
        .add_filter("Video Files", &["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv"])
        .blocking_pick_file();
    
    match file_path {
        Some(file) => {
            let path_buf = file.into_path().map_err(|e| e.to_string())?;
            let path = path_buf.to_string_lossy().to_string();
            Ok(Some(path))
        },
        None => Ok(None)
    }
}

#[tauri::command]
async fn open_subtitle_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    
    let file_path = app.dialog()
        .file()
        .add_filter("Subtitle Files", &["srt", "vtt", "ass", "ssa", "sub"])
        .blocking_pick_file();
    
    match file_path {
        Some(file) => {
            let path_buf = file.into_path().map_err(|e| e.to_string())?;
            let path = path_buf.to_string_lossy().to_string();
            Ok(Some(path))
        },
        None => Ok(None)
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
        println!("Frontend ready - processing {} pending files", pending_files.len());
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
fn enter_pip_mode(app_handle: tauri::AppHandle) -> Result<(), String> {
    let window = app_handle.get_webview_window("main")
        .ok_or("Failed to get main window")?;
    
    // Check if already in PiP mode (idempotency)
    {
        let state = WINDOW_STATE.lock().unwrap();
        if let Some(saved_state) = state.as_ref() {
            if saved_state.pip_active {
                #[cfg(debug_assertions)]
                println!("Already in PiP mode, skipping");
                return Ok(());
            }
        }
    }
    
    // Get PiP configuration from constants.json
    let pip_config = get_pip_constants()
        .map_err(|e| format!("Failed to load PiP configuration: {}", e))?;
    
    // Save current window state
    let current_size = window.outer_size()
        .map_err(|e| format!("Failed to get window size: {}", e))?;
    let current_position = window.outer_position()
        .map_err(|e| format!("Failed to get window position: {}", e))?;
    let current_decorations = window.is_decorated()
        .map_err(|e| format!("Failed to get decoration state: {}", e))?;
    
    let mut state = WINDOW_STATE.lock().unwrap();
    *state = Some(WindowState {
        size: current_size,
        position: current_position,
        decorations_enabled: current_decorations,
        pip_active: true,
    });
    
    #[cfg(debug_assertions)]
    println!("Saved window state: {:?}", state);
    
    // Enable decorations for PiP mode so window can be dragged
    window.set_decorations(true)
        .map_err(|e| format!("Failed to enable decorations: {}", e))?;
    
    // Set PiP window properties
    window.set_size(PhysicalSize::new(pip_config.width, pip_config.height))
        .map_err(|e| format!("Failed to set window size: {}", e))?;
    
    // Position at bottom-right corner with some padding
    let position = if let Ok(monitor) = window.current_monitor() {
        if let Some(monitor) = monitor {
            let monitor_size = monitor.size();
            let x = (monitor_size.width as i32) - (pip_config.width as i32) - pip_config.padding;
            let y = (monitor_size.height as i32) - (pip_config.height as i32) - pip_config.padding;
            PhysicalPosition::new(x, y)
        } else {
            // Fallback when monitor info not available
            let default_width = 1920i32;
            let default_height = 1080i32;
            let x = default_width - (pip_config.width as i32) - pip_config.padding;
            let y = default_height - (pip_config.height as i32) - pip_config.padding;
            PhysicalPosition::new(x, y)
        }
    } else {
        // Fallback when monitor detection fails
        let default_width = 1920i32;
        let default_height = 1080i32;
        let x = default_width - (pip_config.width as i32) - pip_config.padding;
        let y = default_height - (pip_config.height as i32) - pip_config.padding;
        PhysicalPosition::new(x, y)
    };
    
    window.set_position(position)
        .map_err(|e| format!("Failed to set window position: {}", e))?;
    
    // Set window to always on top
    window.set_always_on_top(true)
        .map_err(|e| format!("Failed to set always on top: {}", e))?;
    
    #[cfg(debug_assertions)]
    println!("Entered PiP mode");
    
    Ok(())
}

#[tauri::command]
fn exit_pip_mode(app_handle: tauri::AppHandle) -> Result<(), String> {
    let window = app_handle.get_webview_window("main")
        .ok_or("Failed to get main window")?;
    
    // Validate that we are in PiP mode and have saved state
    let mut state = WINDOW_STATE.lock().unwrap();
    let saved_state = state.take()
        .ok_or("Cannot exit PiP mode: no saved window state found")?;
    
    if !saved_state.pip_active {
        return Err("Cannot exit PiP mode: PiP mode is not currently active".to_string());
    }
    
    #[cfg(debug_assertions)]
    println!("Restoring window state: {:?}", saved_state);
    
    // Restore size and position
    window.set_size(saved_state.size)
        .map_err(|e| format!("Failed to restore window size: {}", e))?;
    window.set_position(saved_state.position)
        .map_err(|e| format!("Failed to restore window position: {}", e))?;
    
    // Restore original decoration state
    window.set_decorations(saved_state.decorations_enabled)
        .map_err(|e| format!("Failed to restore decoration state: {}", e))?;
    
    // Remove always on top
    window.set_always_on_top(false)
        .map_err(|e| format!("Failed to remove always on top: {}", e))?;
    
    #[cfg(debug_assertions)]
    println!("Exited PiP mode");
    
    Ok(())
}

#[tauri::command]
fn find_subtitle_for_video(video_path: String) -> Result<Option<String>, String> {
    use std::path::Path;
    
    let video_path_obj = Path::new(&video_path);
    let video_dir = video_path_obj.parent().ok_or("Could not get video directory")?;
    let video_stem = video_path_obj.file_stem().ok_or("Could not get video filename")?;
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
                entry_path.extension().and_then(|s| s.to_str())
            ) {
                let file_stem_lower = file_stem.to_lowercase();
                let file_ext_lower = file_ext.to_lowercase();
                
                // Check if stem matches (case-insensitive) and extension is a subtitle format
                if file_stem_lower == video_stem_lower && subtitle_exts.contains(&file_ext_lower.as_str()) {
                    #[cfg(debug_assertions)]
                    println!("Found subtitle file (case-insensitive match): {:?}", entry_path);
                    return Ok(Some(entry_path.to_string_lossy().to_string()));
                }
            }
        }
    }
    
    #[cfg(debug_assertions)]
    println!("No subtitle file found for video: {}", video_path);
    Ok(None)
}

#[derive(Serialize, Clone)]
struct VideoFile {
    path: String,
    name: String,
    size: u64,
    modified: u64,
    duration: Option<f64>,
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

// Get video duration using FFmpeg
// Note: Caller should verify ffprobe is available before calling this
fn get_video_duration(video_path: &str) -> Option<f64> {
    let output = create_hidden_command("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            video_path
        ])
        .output()
        .ok()?;
    
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
    create_hidden_command("ffmpeg")
        .arg("-version")
        .output()
        .map(|output| output.status.success())
        .or(Ok(false))
}

// Check which Whisper models are installed
#[tauri::command]
fn check_installed_models() -> Result<Vec<String>, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let models_dir = home.join(".whisper").join("models");
    
    if !models_dir.exists() {
        return Ok(Vec::new());
    }
    
    let mut models = Vec::new();
    let model_files = vec![
        ("ggml-tiny.bin", "tiny"),
        ("ggml-small.bin", "small"),
        ("ggml-large-v3-turbo-q5_0.bin", "large-v3-turbo"),
    ];
    
    for (filename, model_name) in model_files {
        if models_dir.join(filename).exists() {
            models.push(model_name.to_string());
        }
    }
    
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
    
    let content = fs::read_to_string(config_file)
        .map_err(|e| format!("Failed to read config: {}", e))?;
    
    let config: serde_json::Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse config: {}", e))?;
    
    Ok(config.get("setup_completed").and_then(|v| v.as_bool()).unwrap_or(false))
}

// Save setup completion status to config
fn save_setup_completed(completed: bool) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let config_file = config_dir.join("config.json");
    
    // Create config directory if it doesn't exist
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;
    
    // Load existing config or create new
    let mut config: serde_json::Value = if config_file.exists() {
        let content = fs::read_to_string(&config_file)
            .map_err(|e| format!("Failed to read config: {}", e))?;
        serde_json::from_str(&content)
            .map_err(|e| format!("Failed to parse config: {}", e))?
    } else {
        serde_json::json!({})
    };
    
    // Update setup_completed field
    config["setup_completed"] = serde_json::json!(completed);
    
    // Save config
    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    
    fs::write(config_file, content)
        .map_err(|e| format!("Failed to write config: {}", e))?;
    
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
    
    let url = format!("https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}", model_name);
    
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let models_dir = home.join(".whisper").join("models");
    
    // Create models directory
    fs::create_dir_all(&models_dir)
        .map_err(|e| format!("Failed to create models directory: {}", e))?;
    
    let output_path = models_dir.join(model_name);
    
    // Download file with progress
    download_file_with_progress(&app_handle, &url, &output_path, &format!("Downloading {} model", model_size)).await?;
    
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
    let response = client.get(url).send().await
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
        let snippet = if ct.contains("text") || ct.contains("json") || ct.contains("xml") || ct.contains("html") {
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
            Err(format!("HTTP error {} {}: {}", status.as_u16(), status, snippet))
        };
    }

    // Success: only now inspect content length, create the file, and stream bytes
    let total_size = response.content_length().unwrap_or(0);

    let mut file = fs::File::create(output_path)
        .map_err(|e| format!("Failed to create file: {}", e))?;

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
            let _ = app_handle.emit("download-progress", DownloadProgress {
                downloaded,
                total: total_size,
                percentage,
                message: message.to_string(),
            });
        }
    }

    Ok(())
}

// Helper function to extract audio from video using FFmpeg
fn extract_audio_from_video(video_path: &str, output_audio_path: &str) -> Result<(), String> {
    #[cfg(debug_assertions)]
    println!("Extracting audio from video: {}", video_path);
    
    let output = create_hidden_command("ffmpeg")
        .args([
            "-i", video_path,
            "-vn",  // No video
            "-acodec", "pcm_s16le",  // PCM 16-bit little-endian
            "-ar", "16000",  // Sample rate 16kHz (Whisper's expected rate)
            "-ac", "1",  // Mono channel
            "-y",  // Overwrite output file
            output_audio_path
        ])
        .output()
        .map_err(|e| format!("Failed to execute ffmpeg: {}. Make sure FFmpeg is installed and in PATH.", e))?;
    
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
fn generate_srt_from_segments(segments: Vec<(f64, f64, String)>, output_path: &str) -> Result<(), String> {
    let mut srt_content = String::new();
    
    for (index, (start, end, text)) in segments.iter().enumerate() {
        srt_content.push_str(&format!("{}\n", index + 1));
        srt_content.push_str(&format!("{} --> {}\n", format_srt_time(*start), format_srt_time(*end)));
        srt_content.push_str(&format!("{}\n\n", text.trim()));
    }
    
    fs::write(output_path, srt_content)
        .map_err(|e| format!("Failed to write SRT file: {}", e))?;
    
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
    #[cfg(debug_assertions)] {
        println!("Starting subtitle generation for: {}", video_path);
        println!("Model size: {}", model_size);
    }
    
    // Emit initial progress
    let _ = app_handle.emit("subtitle-generation-progress", SubtitleGenerationProgress {
        stage: "initializing".to_string(),
        progress: 0.0,
        message: "Initializing subtitle generation...".to_string(),
    });
    
    let video_path_obj = Path::new(&video_path);
    let video_dir = video_path_obj.parent()
        .ok_or("Could not get video directory")?;
    let video_stem = video_path_obj.file_stem()
        .ok_or("Could not get video filename")?;
    
    // Create temporary audio file path
    let temp_audio_path = video_dir.join(format!("{}_temp_audio.wav", video_stem.to_string_lossy()));
    let temp_audio_str = temp_audio_path.to_string_lossy().to_string();
    
    // Output subtitle path
    let subtitle_path = video_dir.join(format!("{}.srt", video_stem.to_string_lossy()));
    let subtitle_path_str = subtitle_path.to_string_lossy().to_string();
    
    // Step 1: Extract audio from video
    let _ = app_handle.emit("subtitle-generation-progress", SubtitleGenerationProgress {
        stage: "extracting_audio".to_string(),
        progress: 10.0,
        message: "Extracting audio from video...".to_string(),
    });
    
    extract_audio_from_video(&video_path, &temp_audio_str)?;
    
    // Step 2: Load Whisper model
    let _ = app_handle.emit("subtitle-generation-progress", SubtitleGenerationProgress {
        stage: "loading_model".to_string(),
        progress: 30.0,
        message: format!("Loading Whisper {} model...", model_size),
    });
    
    // Get model path from user's home directory or use default location
    let model_name = match model_size.as_str() {
        "tiny" => "ggml-tiny.bin",
        "small" => "ggml-small.bin",
        "large-v3-turbo" => "ggml-large-v3-turbo-q5_0.bin",
        _ => "ggml-tiny.bin",
    };
    
    let model_path = dirs::home_dir()
        .ok_or("Could not find home directory")?;
    let model_path = model_path.join(".whisper").join("models").join(model_name);
    
    if !model_path.exists() {
        let error_msg = format!(
            "Whisper model not found at: {}\n\nPlease download the model first. You can download it from:\nhttps://huggingface.co/ggerganov/whisper.cpp/tree/main\n\nPlace it in: {}",
            model_path.display(),
            model_path.parent().unwrap().display()
        );
        
        // Clean up temp audio file
        let _ = fs::remove_file(&temp_audio_str);
        
        let _ = app_handle.emit("subtitle-generation-progress", SubtitleGenerationProgress {
            stage: "error".to_string(),
            progress: 0.0,
            message: error_msg.clone(),
        });
        
        return Err(error_msg);
    }
    
    // Step 3: Transcribe audio with Whisper
    let _ = app_handle.emit("subtitle-generation-progress", SubtitleGenerationProgress {
        stage: "transcribing".to_string(),
        progress: 50.0,
        message: "Transcribing audio with AI...".to_string(),
    });
    
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
    }).await
    .map_err(|e| format!("Transcription task failed: {}", e))??;
    
    // Clean up temporary audio file
    let _ = fs::remove_file(&temp_audio_str);
    
    // Step 4: Complete
    let _ = app_handle.emit("subtitle-generation-progress", SubtitleGenerationProgress {
        stage: "complete".to_string(),
        progress: 100.0,
        message: "Subtitles generated successfully!".to_string(),
    });
    
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
    use whisper_rs::{WhisperContext, WhisperContextParameters, FullParams, SamplingStrategy};
    
    #[cfg(debug_assertions)]
    println!("Loading Whisper model from: {}", model_path);
    
    let ctx = WhisperContext::new_with_params(
        model_path,
        WhisperContextParameters::default(),
    ).map_err(|e| format!("Failed to load Whisper model: {}", e))?;
    
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
    params.set_translate(false);  // Don't translate, keep original language
    params.set_language(Some(language));  // Use selected language
    params.set_max_len(0);  // Disable max length limit per segment
    params.set_split_on_word(true);  // Split on word boundaries
    
    // Run transcription
    let mut state = ctx.create_state()
        .map_err(|e| format!("Failed to create Whisper state: {}", e))?;
    
    state.full(params, &audio_data)
        .map_err(|e| format!("Transcription failed: {}", e))?;
    
    #[cfg(debug_assertions)]
    println!("Transcription complete, extracting segments...");
    
    // Extract segments with timestamps
    let num_segments = state.full_n_segments()
        .map_err(|e| format!("Failed to get segment count: {}", e))?;
    
    #[cfg(debug_assertions)]
    println!("Found {} segments", num_segments);
    
    let mut segments = Vec::new();
    
    for i in 0..num_segments {
        let start_timestamp = state.full_get_segment_t0(i)
            .map_err(|e| format!("Failed to get start timestamp: {}", e))?;
        let end_timestamp = state.full_get_segment_t1(i)
            .map_err(|e| format!("Failed to get end timestamp: {}", e))?;
        let text = state.full_get_segment_text(i)
            .map_err(|e| format!("Failed to get segment text: {}", e))?;
        
        // Convert from Whisper's timestamp units (10ms) to seconds
        let start_seconds = start_timestamp as f64 / 100.0;
        let end_seconds = end_timestamp as f64 / 100.0;
        
        if !text.trim().is_empty() {
            segments.push((start_seconds, end_seconds, text));
        }
        
        // Emit progress periodically
        if i % 10 == 0 {
            let progress = 50.0 + (i as f32 / num_segments as f32) * 40.0;
            let _ = app_handle.emit("subtitle-generation-progress", SubtitleGenerationProgress {
                stage: "transcribing".to_string(),
                progress,
                message: format!("Processing segment {} of {}...", i + 1, num_segments),
            });
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
    
    let mut file = File::open(path)
        .map_err(|e| format!("Failed to open audio file: {}", e))?;
    
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
fn save_watch_progress(
    video_path: String,
    current_time: f64,
    duration: f64,
) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let progress_file = config_dir.join("watch_progress.json");
    
    // Create config directory if it doesn't exist
    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;
    
    // Load existing progress data or create new
    let mut progress_map: std::collections::HashMap<String, WatchProgress> = if progress_file.exists() {
        let content = fs::read_to_string(&progress_file)
            .map_err(|e| format!("Failed to read progress file: {}", e))?;
        serde_json::from_str(&content)
            .unwrap_or_else(|_| std::collections::HashMap::new())
    } else {
        std::collections::HashMap::new()
    };
    
    // Get current time as Unix timestamp
    let last_watched = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|e| format!("Failed to get current time: {}", e))?
        .as_secs();
    
    // Update or insert progress for this video
    progress_map.insert(video_path.clone(), WatchProgress {
        path: video_path,
        current_time,
        duration,
        last_watched,
    });
    
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
    
    let progress_map: std::collections::HashMap<String, WatchProgress> = serde_json::from_str(&content)
        .unwrap_or_else(|_| std::collections::HashMap::new());
    
    Ok(progress_map.get(&video_path).cloned())
}

// Get video file info
#[tauri::command]
fn get_video_info(video_path: String) -> Result<VideoInfo, String> {
    let path = Path::new(&video_path);
    
    // Get file size
    let metadata = fs::metadata(path)
        .map_err(|e| format!("Failed to get file metadata: {}", e))?;
    let size_bytes = metadata.len();
    let size_mb = size_bytes as f64 / (1024.0 * 1024.0);
    
    // Get format from extension
    let format = path.extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("unknown")
        .to_uppercase();
    
    Ok(VideoInfo {
        format,
        size_mb,
    })
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
    let video_dir = video_path_obj.parent()
        .ok_or("Could not get video directory")?;
    let video_stem = video_path_obj.file_stem()
        .ok_or("Could not get video filename")?;
    
    // Output path
    let output_path = video_dir.join(format!("{}_converted.{}", video_stem.to_string_lossy(), target_format));
    let output_path_str = output_path.to_string_lossy().to_string();
    
    // Emit initial progress
    let _ = app_handle.emit("conversion-progress", ConversionProgress {
        stage: "starting".to_string(),
        progress: 0.0,
        message: format!("Starting conversion to {}...", target_format.to_uppercase()),
    });
    
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
    }).await
    .map_err(|e| format!("Conversion task failed: {}", e))??;
    
    // Emit completion
    let _ = app_handle.emit("conversion-progress", ConversionProgress {
        stage: "complete".to_string(),
        progress: 100.0,
        message: "Conversion complete!".to_string(),
    });
    
    Ok(output_path_str)
}

// Convert video using FFmpeg
fn convert_video_with_ffmpeg(
    input_path: &str,
    output_path: &str,
    target_format: &str,
    app_handle: &tauri::AppHandle,
) -> Result<(), String> {
    let _ = app_handle.emit("conversion-progress", ConversionProgress {
        stage: "converting".to_string(),
        progress: 50.0,
        message: format!("Converting to {}...", target_format.to_uppercase()),
    });
    
    // Build FFmpeg command based on target format
    let mut cmd = create_hidden_command("ffmpeg");
    cmd.arg("-i").arg(input_path);
    
    match target_format {
        "mp4" => {
            cmd.args(["-c:v", "libx264", "-preset", "medium", "-crf", "23", "-c:a", "aac", "-b:a", "192k"]);
        }
        "webm" => {
            cmd.args(["-c:v", "libvpx-vp9", "-crf", "30", "-b:v", "0", "-c:a", "libopus"]);
        }
        "mkv" => {
            cmd.args(["-c:v", "copy", "-c:a", "copy"]); // Just remux, no re-encoding
        }
        _ => return Err(format!("Unsupported format: {}", target_format)),
    }
    
    cmd.arg("-y").arg(output_path);
    
    let output = cmd.output()
        .map_err(|e| format!("Failed to execute ffmpeg: {}. Make sure FFmpeg is installed.", e))?;
    
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
    
    let progress_map: std::collections::HashMap<String, WatchProgress> = serde_json::from_str(&content)
        .unwrap_or_else(|_| std::collections::HashMap::new());
    
    Ok(progress_map)
}

#[tauri::command]
fn get_recent_videos() -> Result<Vec<VideoFile>, String> {
    let video_extensions = vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv"];
    let mut videos = Vec::new();
    
    // Check if ffprobe is available once at the start
    let ffprobe_available = create_hidden_command("ffprobe")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false);
    
    if !ffprobe_available {
        eprintln!("Warning: ffprobe not found in PATH. Video durations will not be extracted.");
    }
    
    // Get common video directories
    let mut search_dirs = Vec::new();
    
    if let Some(home) = dirs::home_dir() {
        #[cfg(debug_assertions)]
        println!("Home directory: {:?}", home);
        search_dirs.push(home.join("Videos"));
        search_dirs.push(home.join("Downloads"));
        search_dirs.push(home.join("Desktop"));
        search_dirs.push(home.join("Documents"));
    }
    
    // Scan directories
    for dir in &search_dirs {
        #[cfg(debug_assertions)]
        println!("Checking directory: {:?} (exists: {})", dir, dir.exists());
        if dir.exists() {
            #[cfg(debug_assertions)]
            let mut dir_video_count = 0;
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_file() {
                            if let Some(ext) = entry.path().extension() {
                                if let Some(ext_str) = ext.to_str() {
                                        if video_extensions.contains(&ext_str.to_lowercase().as_str()) {
                                            if let Ok(modified) = metadata.modified() {
                                                if let Ok(duration) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                                                    let video_path = entry.path().to_string_lossy().to_string();
                                                    // Only try to get duration if ffprobe is available
                                                    let video_duration = if ffprobe_available {
                                                        get_video_duration(&video_path)
                                                    } else {
                                                        None
                                                    };
                                                    
                                                    videos.push(VideoFile {
                                                        path: video_path,
                                                        name: entry.file_name().to_string_lossy().to_string(),
                                                        size: metadata.len(),
                                                        modified: duration.as_secs(),
                                                        duration: video_duration,
                                                    });
                                                    #[cfg(debug_assertions)]
                                                    {
                                                        dir_video_count += 1;
                                                    }
                                                }
                                            }
                                        }
                                }
                            }
                        }
                    }
                }
            }
            #[cfg(debug_assertions)]
            println!("Found {} videos in {:?}", dir_video_count, dir);
        }
    }
    
    // Sort by modified time (most recent first)
    videos.sort_by(|a, b| b.modified.cmp(&a.modified));
    
    // Return top 20
    videos.truncate(20);
    
    Ok(videos)
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
                            println!(
                                "Attempt {}: Failed to emit event: {:?}",
                                attempt, e
                            );
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
            
            #[cfg(debug_assertions)] {
                println!("=== GLUCOSE STARTUP ===");
                println!("Command line arguments: {:?}", args);
            }
            
            // Check if we're being launched via file association
            if args.len() > 1 {
                #[cfg(debug_assertions)]
                println!("*** LAUNCHED WITH ARGUMENTS - POTENTIAL FILE ASSOCIATION ***");
                
                let video_extensions = vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv"];
                let mut video_files: Vec<String> = Vec::new();

                for arg in &args[1..] {
                    // Sanitize the path
                    let clean_arg = sanitize_path(arg);
                    #[cfg(debug_assertions)]
                    println!("Processing argument: {} -> {}", arg, clean_arg);

                    let lower = clean_arg.to_lowercase();
                    for ext in &video_extensions {
                        if lower.ends_with(&format!(".{}", ext)) {
                            video_files.push(clean_arg.clone());
                            #[cfg(debug_assertions)]
                            println!("Found video file: {}", clean_arg);
                            break;
                        }
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
            open_subtitle_dialog,
            convert_file_path,
            get_recent_videos,
            get_pending_file,
            mark_file_processed,
            frontend_ready,
            exit_app,
            find_subtitle_for_video,
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
            exit_pip_mode
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
                    #[cfg(debug_assertions)] {
                        println!("*** FILE ASSOCIATION EVENT RECEIVED ***");
                        println!("Received opened event with URLs: {:?}", urls);
                    }
                    
                    let video_extensions = vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv"];
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
                            
                            let lower = decoded_path.to_lowercase();
                            for ext in &video_extensions {
                                if lower.ends_with(&format!(".{}", ext)) {
                                    video_files.push(decoded_path.to_string());
                                    #[cfg(debug_assertions)]
                                    println!("Found video file from opened event: {}", decoded_path);
                                    break;
                                }
                            }
                        }
                    }
                    
                    if !video_files.is_empty() {
                        #[cfg(debug_assertions)]
                        println!("Processing {} video files from file association event", video_files.len());
                        process_video_files(&_app_handle, video_files);
                    }
                }
                _ => {}
            }
        });
}
