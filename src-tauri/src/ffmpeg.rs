use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Clone, Serialize)]
pub struct FfmpegPathInfo {
    pub path: Option<String>,
    pub is_custom: bool,
}

pub(crate) fn get_ffmpeg_custom_path_from_config() -> Option<String> {
    let home = dirs::home_dir()?;
    let config_file = home.join(".glucose").join("config.json");
    let content = fs::read_to_string(&config_file).ok()?;
    let config: serde_json::Value = serde_json::from_str(&content).ok()?;
    config
        .get("ffmpeg_custom_path")
        .and_then(|v| v.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
}

fn validates_as_ffmpeg(path: &Path) -> bool {
    path.is_file()
        && crate::create_hidden_command(path.to_string_lossy().as_ref())
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
}

pub(crate) fn validate_ffmpeg_path(path: &str) -> Result<(), String> {
    let candidate = Path::new(path);

    if !candidate.exists() {
        return Err(format!("FFmpeg path does not exist: {}", path));
    }

    if !candidate.is_file() {
        return Err(format!(
            "FFmpeg path must point to an executable file: {}",
            path
        ));
    }

    if !validates_as_ffmpeg(candidate) {
        return Err(format!(
            "Selected file is not a working FFmpeg executable: {}",
            path
        ));
    }

    Ok(())
}

fn executable_candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    let base = dir.join(name);

    #[cfg(target_os = "windows")]
    {
        if base.extension().is_some() {
            return vec![base];
        }

        let pathext =
            std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".to_string());
        pathext
            .split(';')
            .filter(|ext| !ext.trim().is_empty())
            .map(|ext| dir.join(format!("{}{}", name, ext.trim())))
            .collect()
    }

    #[cfg(not(target_os = "windows"))]
    {
        vec![base]
    }
}

fn resolve_from_path(name: &str) -> Option<String> {
    let path_var = std::env::var_os("PATH")?;

    for dir in std::env::split_paths(&path_var) {
        for candidate in executable_candidates(&dir, name) {
            if validates_as_ffmpeg(&candidate) {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    None
}

pub(crate) fn resolve_ffmpeg_path_info() -> FfmpegPathInfo {
    if let Some(custom) = get_ffmpeg_custom_path_from_config() {
        if validate_ffmpeg_path(&custom).is_ok() {
            return FfmpegPathInfo {
                path: Some(custom),
                is_custom: true,
            };
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Ok(app_data) = std::env::var("LOCALAPPDATA") {
            let ffmpeg_exe = std::path::Path::new(&app_data)
                .join("glucose")
                .join("resources")
                .join("ffmpeg")
                .join("bin")
                .join("ffmpeg.exe");
            if validates_as_ffmpeg(&ffmpeg_exe) {
                return FfmpegPathInfo {
                    path: Some(ffmpeg_exe.to_string_lossy().to_string()),
                    is_custom: false,
                };
            }
        }
    }

    if let Some(path) = resolve_from_path("ffmpeg") {
        FfmpegPathInfo {
            path: Some(path),
            is_custom: false,
        }
    } else {
        FfmpegPathInfo {
            path: None,
            is_custom: false,
        }
    }
}

#[tauri::command]
pub fn get_ffmpeg_path() -> Result<FfmpegPathInfo, String> {
    Ok(resolve_ffmpeg_path_info())
}

#[tauri::command]
pub async fn pick_ffmpeg_executable(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let result = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        let builder = app
            .dialog()
            .file()
            .add_filter("FFmpeg Executable", &["exe"]);
        #[cfg(not(target_os = "windows"))]
        let builder = app.dialog().file();
        builder.blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    match result {
        Some(file) => {
            let path_buf = file.into_path().map_err(|e| e.to_string())?;
            Ok(Some(path_buf.to_string_lossy().to_string()))
        }
        None => Ok(None),
    }
}

#[tauri::command]
pub fn save_ffmpeg_custom_path(path: Option<String>) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    let config_dir = home.join(".glucose");
    let config_file = config_dir.join("config.json");

    fs::create_dir_all(&config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    let _guard = crate::CONFIG_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());

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

    match path {
        Some(p) => {
            validate_ffmpeg_path(&p)?;
            config_object.insert("ffmpeg_custom_path".to_string(), serde_json::json!(p));
        }
        None => {
            config_object.remove("ffmpeg_custom_path");
        }
    }

    let content = serde_json::to_string_pretty(&config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    let temp_file = config_file.with_extension("json.tmp");
    fs::write(&temp_file, &content).map_err(|e| format!("Failed to write temp config: {}", e))?;
    fs::rename(&temp_file, &config_file).map_err(|e| format!("Failed to replace config: {}", e))?;

    Ok(())
}
