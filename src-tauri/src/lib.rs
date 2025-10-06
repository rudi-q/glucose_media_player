use tauri::Emitter;
#[cfg(any(target_os = "macos", target_os = "ios"))]
use tauri::{Manager, RunEvent};
use std::fs;
use std::time::SystemTime;
use std::collections::VecDeque;
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use serde::Serialize;

// Global state to store pending file paths
static PENDING_FILES: Mutex<VecDeque<String>> = Mutex::new(VecDeque::new());
static FILE_PROCESSED: Mutex<bool> = Mutex::new(false);

// Configuration constants
const MAX_FILE_LOADING_ATTEMPTS: u32 = 30;
const FRONTEND_READY_WAIT_MS: u64 = 500;
const INITIAL_ATTEMPT_DELAY_MS: u64 = 2000;
const MIDDLE_ATTEMPT_DELAY_MS: u64 = 1000;
const FINAL_ATTEMPT_DELAY_MS: u64 = 500;
const INITIAL_ATTEMPT_COUNT: u32 = 5;
const MIDDLE_ATTEMPT_COUNT: u32 = 15;

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
        .add_filter("Image Files", &["jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "avif", "heic", "heif"])
        .add_filter("All Media", &["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv", "jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "avif", "heic", "heif"])
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
    println!("Frontend is ready to receive events");
    
    // Check if there are any pending files and emit them now
    let pending_files: Vec<String> = {
        let mut pending = PENDING_FILES.lock().unwrap();
        pending.drain(..).collect()
    };
    
    if !pending_files.is_empty() {
        println!("Frontend ready - processing {} pending files", pending_files.len());
        process_video_files(&app_handle, pending_files);
    }
    
    Ok(())
}

#[tauri::command]
fn exit_app(app_handle: tauri::AppHandle) {
    println!("Exit app command called");
    app_handle.exit(0);
}

#[derive(Serialize, Clone)]
struct VideoFile {
    path: String,
    name: String,
    size: u64,
    modified: u64,
}

#[tauri::command]
fn get_recent_videos() -> Result<Vec<VideoFile>, String> {
    let video_extensions = vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv"];
    let mut videos = Vec::new();
    
    // Get common video directories
    let mut search_dirs = Vec::new();
    
    if let Some(home) = dirs::home_dir() {
        search_dirs.push(home.join("Videos"));
        search_dirs.push(home.join("Downloads"));
        search_dirs.push(home.join("Desktop"));
        search_dirs.push(home.join("Documents"));
    }
    
    // Scan directories
    for dir in search_dirs {
        if dir.exists() {
            if let Ok(entries) = fs::read_dir(&dir) {
                for entry in entries.flatten() {
                    if let Ok(metadata) = entry.metadata() {
                        if metadata.is_file() {
                            if let Some(ext) = entry.path().extension() {
                                if let Some(ext_str) = ext.to_str() {
                                    if video_extensions.contains(&ext_str.to_lowercase().as_str()) {
                                        if let Ok(modified) = metadata.modified() {
                                            if let Ok(duration) = modified.duration_since(SystemTime::UNIX_EPOCH) {
                                                videos.push(VideoFile {
                                                    path: entry.path().to_string_lossy().to_string(),
                                                    name: entry.file_name().to_string_lossy().to_string(),
                                                    size: metadata.len(),
                                                    modified: duration.as_secs(),
                                                });
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
                        println!("File already processed, stopping attempts");
                        break;
                    }
                }

                // Try to emit event for first video file immediately on first attempt
                if let Some(video_file) = video_files.first() {
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

            println!("File loading attempts completed");
        });
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Handle command line arguments for file associations
            let args: Vec<String> = std::env::args().collect();
            
            println!("=== GLUCOSE STARTUP ===");
            println!("Command line arguments: {:?}", args);
            
            // Check if we're being launched via file association
            if args.len() > 1 {
                println!("*** LAUNCHED WITH ARGUMENTS - POTENTIAL FILE ASSOCIATION ***");
                
                let video_extensions = vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv"];
                let image_extensions = vec!["jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "avif", "heic", "heif"];
                let mut media_files: Vec<String> = Vec::new();

                for arg in &args[1..] {
                    // Sanitize the path
                    let clean_arg = sanitize_path(arg);
                    println!("Processing argument: {} -> {}", arg, clean_arg);

                    let lower = clean_arg.to_lowercase();
                    
                    // Check if it's a video file
                    let mut is_media = false;
                    for ext in &video_extensions {
                        if lower.ends_with(&format!(".{}", ext)) {
                            media_files.push(clean_arg.clone());
                            println!("Found video file: {}", clean_arg);
                            is_media = true;
                            break;
                        }
                    }
                    
                    // Check if it's an image file
                    if !is_media {
                        for ext in &image_extensions {
                            if lower.ends_with(&format!(".{}", ext)) {
                                media_files.push(clean_arg.clone());
                                println!("Found image file: {}", clean_arg);
                                break;
                            }
                        }
                    }
                }

                if !media_files.is_empty() {
                    println!("Queued {} media files", media_files.len());
                    process_video_files(&app.handle(), media_files);
                }
            } else {
                println!("*** LAUNCHED WITHOUT ARGUMENTS - NORMAL APP LAUNCH ***");
            }
            
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            open_file_dialog,
            convert_file_path,
            get_recent_videos,
            get_pending_file,
            mark_file_processed,
            frontend_ready,
            exit_app
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
                    println!("*** FILE ASSOCIATION EVENT RECEIVED ***");
                    println!("Received opened event with URLs: {:?}", urls);
                    
                    let video_extensions = vec!["mp4", "mkv", "avi", "mov", "wmv", "flv", "webm", "m4v", "mpg", "mpeg", "ogv"];
                    let image_extensions = vec!["jpg", "jpeg", "png", "gif", "webp", "bmp", "svg", "avif", "heic", "heif"];
                    let mut media_files: Vec<String> = Vec::new();
                    
                    for url in urls {
                        let url_str = url.to_string();
                        println!("Processing URL: {}", url_str);
                        
                        if url_str.starts_with("file://") {
                            let path = url_str.replace("file://", "");
                            let decoded_path = urlencoding::decode(&path).unwrap_or_default();
                            
                            println!("Decoded path: {}", decoded_path);
                            
                            let lower = decoded_path.to_lowercase();
                            let mut is_media = false;
                            
                            // Check video extensions
                            for ext in &video_extensions {
                                if lower.ends_with(&format!(".{}", ext)) {
                                    media_files.push(decoded_path.to_string());
                                    println!("Found video file from opened event: {}", decoded_path);
                                    is_media = true;
                                    break;
                                }
                            }
                            
                            // Check image extensions
                            if !is_media {
                                for ext in &image_extensions {
                                    if lower.ends_with(&format!(".{}", ext)) {
                                        media_files.push(decoded_path.to_string());
                                        println!("Found image file from opened event: {}", decoded_path);
                                        break;
                                    }
                                }
                            }
                        }
                    }
                    
                    if !media_files.is_empty() {
                        println!("Processing {} media files from file association event", media_files.len());
                        process_video_files(&_app_handle, media_files);
                    }
                }
                _ => {}
            }
        });
}
