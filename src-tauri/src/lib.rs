use tauri::{Manager, Emitter};
use std::fs;
use std::time::SystemTime;
use serde::Serialize;

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
fn convert_file_path(path: String) -> Result<String, String> {
    Ok(format!("https://asset.localhost/{}", path))
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            // Handle file opening via command line arguments
            if let Some(args) = std::env::args().nth(1) {
                let window = app.get_webview_window("main").unwrap();
                window.emit("open-file", args).ok();
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![open_file_dialog, convert_file_path, get_recent_videos])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
