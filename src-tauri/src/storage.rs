use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use tauri::{AppHandle, Manager};

#[cfg(target_os = "macos")]
use base64::Engine;
#[cfg(target_os = "macos")]
use base64::engine::general_purpose::STANDARD as BASE64_ENGINE;
#[cfg(target_os = "macos")]
use objc2::rc::{Retained, autoreleasepool};
#[cfg(target_os = "macos")]
use objc2::runtime::Bool;
#[cfg(target_os = "macos")]
use objc2_foundation::{
    NSData, NSError, NSString, NSURL, NSURLBookmarkCreationOptions,
    NSURLBookmarkResolutionOptions,
};

static APP_HANDLE: OnceLock<AppHandle> = OnceLock::new();

// Holds resolved NSURLs for the lifetime of the app so security-scoped access
// (started via startAccessingSecurityScopedResource) remains valid for as long
// as the app is running. Dropping the Retained<NSURL> revokes the kernel
// grant, which causes the asset:// protocol used by <video> to get EACCES on
// every file under a user-picked gallery folder.
#[cfg(target_os = "macos")]
static ACTIVE_GALLERY_BOOKMARKS: OnceLock<Mutex<HashMap<String, Retained<NSURL>>>> =
    OnceLock::new();

#[cfg(target_os = "macos")]
const GALLERY_BOOKMARKS_KEY: &str = "gallery_path_bookmarks";

pub(crate) fn set_app_handle(app_handle: AppHandle) {
    let _ = APP_HANDLE.set(app_handle);
}

fn app_handle() -> Result<&'static AppHandle, String> {
    APP_HANDLE
        .get()
        .ok_or_else(|| "App handle not initialized".to_string())
}

pub(crate) fn default_gallery_paths() -> Vec<String> {
    // macOS sandbox requires explicit user-selected access; no implicit defaults
    #[cfg(target_os = "macos")]
    return Vec::new();

    #[cfg(not(target_os = "macos"))]
    {
        let mut paths = Vec::new();
        for dir in [
            dirs::video_dir(),
            dirs::download_dir(),
            dirs::desktop_dir(),
            dirs::document_dir(),
        ] {
            if let Some(path) = dir {
                let path = path.to_string_lossy().to_string();
                if !paths.contains(&path) {
                    paths.push(path);
                }
            }
        }
        paths
    }
}

pub(crate) fn config_file_path() -> Result<PathBuf, String> {
    let path = config_dir()?.join("config.json");
    #[cfg(target_os = "macos")]
    migrate_legacy_file(&legacy_config_file_path()?, &path)?;
    Ok(path)
}

pub(crate) fn watch_progress_file_path() -> Result<PathBuf, String> {
    let path = watch_progress_dir()?.join("watch_progress.json");
    #[cfg(target_os = "macos")]
    migrate_legacy_file(&legacy_watch_progress_file_path()?, &path)?;
    Ok(path)
}

#[allow(dead_code)] // Referenced only by the AI subtitle code, disabled for the Mac App Store build.
pub(crate) fn whisper_models_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        let dir = app_handle()?
            .path()
            .app_data_dir()
            .map_err(|e| format!("Failed to resolve app data directory: {}", e))?
            .join("models");
        migrate_legacy_directory(&legacy_models_dir()?, &dir)?;
        Ok(dir)
    }

    #[cfg(not(target_os = "macos"))]
    {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        Ok(home.join(".whisper").join("models"))
    }
}

pub(crate) fn load_gallery_paths() -> Result<Vec<String>, String> {
    let config_file = config_file_path()?;
    if !config_file.exists() {
        return Ok(default_gallery_paths());
    }

    let _guard = crate::CONFIG_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let content =
        fs::read_to_string(&config_file).map_err(|e| format!("Failed to read config: {}", e))?;
    let config: Value =
        serde_json::from_str(&content).map_err(|e| format!("Failed to parse config: {}", e))?;

    Ok(extract_gallery_paths(&config))
}

pub(crate) fn save_gallery_paths(paths: &[String]) -> Result<(), String> {
    let config_file = config_file_path()?;
    let config_dir = config_file
        .parent()
        .ok_or_else(|| "Could not get config directory".to_string())?;

    fs::create_dir_all(config_dir)
        .map_err(|e| format!("Failed to create config directory: {}", e))?;

    let _guard = crate::CONFIG_MUTEX
        .lock()
        .unwrap_or_else(|e| e.into_inner());

    let mut config: Value = if config_file.exists() {
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

    #[cfg(target_os = "macos")]
    sync_gallery_bookmarks(config_object, paths)?;

    write_config_file(&config_file, &config)
}

pub(crate) fn load_gallery_scan_dirs() -> Result<Vec<PathBuf>, String> {
    let gallery_paths = load_gallery_paths()?;

    #[cfg(target_os = "macos")]
    {
        let config_file = config_file_path()?;
        let bookmarks = if config_file.exists() {
            let _guard = crate::CONFIG_MUTEX
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            let content = fs::read_to_string(&config_file)
                .map_err(|e| format!("Failed to read config: {}", e))?;
            let config: Value = serde_json::from_str(&content)
                .map_err(|e| format!("Failed to parse config: {}", e))?;
            extract_gallery_bookmarks(&config)
        } else {
            HashMap::new()
        };

        let mut resolved = Vec::new();
        for gallery_path in gallery_paths {
            if let Some(bookmark) = bookmarks.get(&gallery_path) {
                match resolve_security_scoped_bookmark(bookmark) {
                    Ok(path) => {
                        resolved.push(path);
                        continue;
                    }
                    Err(err) => {
                        #[cfg(debug_assertions)]
                        eprintln!(
                            "Failed to resolve bookmark for {}: {}",
                            gallery_path, err
                        );
                    }
                }
            }

            resolved.push(PathBuf::from(gallery_path));
        }

        Ok(resolved)
    }

    #[cfg(not(target_os = "macos"))]
    {
        Ok(gallery_paths.into_iter().map(PathBuf::from).collect())
    }
}

fn extract_gallery_paths(config: &Value) -> Vec<String> {
    if let Some(paths) = config.get("gallery_paths").and_then(|value| value.as_array()) {
        let extracted: Vec<String> = paths
            .iter()
            .filter_map(|value| value.as_str().map(String::from))
            .collect();
        if !extracted.is_empty() {
            return extracted;
        }
    }

    default_gallery_paths()
}

#[cfg(target_os = "macos")]
fn sync_gallery_bookmarks(
    config_object: &mut serde_json::Map<String, Value>,
    paths: &[String],
) -> Result<(), String> {
    let existing = config_object
        .get(GALLERY_BOOKMARKS_KEY)
        .and_then(|value| value.as_object())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|(path, bookmark)| {
                    bookmark
                        .as_str()
                        .map(|bookmark| (path.clone(), bookmark.to_string()))
                })
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let mut next = serde_json::Map::new();

    for path in paths {
        if is_default_gallery_path(path) {
            continue;
        }

        if let Some(bookmark) = existing.get(path) {
            next.insert(path.clone(), Value::String(bookmark.clone()));
            continue;
        }

        let bookmark = create_security_scoped_bookmark(Path::new(path)).map_err(|err| {
            format!(
                "Failed to preserve sandbox access for '{}': {}",
                path, err
            )
        })?;
        next.insert(path.clone(), Value::String(bookmark));
    }

    if next.is_empty() {
        config_object.remove(GALLERY_BOOKMARKS_KEY);
    } else {
        config_object.insert(GALLERY_BOOKMARKS_KEY.to_string(), Value::Object(next));
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn is_default_gallery_path(path: &str) -> bool {
    let normalized = normalize_path(path);
    default_gallery_paths()
        .into_iter()
        .any(|default| normalize_path(&default) == normalized)
}

#[cfg(target_os = "macos")]
fn normalize_path(path: &str) -> String {
    path.trim_end_matches('/').to_string()
}

#[cfg(target_os = "macos")]
fn extract_gallery_bookmarks(config: &Value) -> HashMap<String, String> {
    config
        .get(GALLERY_BOOKMARKS_KEY)
        .and_then(|value| value.as_object())
        .map(|entries| {
            entries
                .iter()
                .filter_map(|(path, bookmark)| {
                    bookmark
                        .as_str()
                        .map(|bookmark| (path.clone(), bookmark.to_string()))
                })
                .collect()
        })
        .unwrap_or_default()
}

fn write_config_file(config_file: &Path, config: &Value) -> Result<(), String> {
    let content = serde_json::to_string_pretty(config)
        .map_err(|e| format!("Failed to serialize config: {}", e))?;
    let temp_file = config_file.with_extension("json.tmp");
    fs::write(&temp_file, &content).map_err(|e| format!("Failed to write temp config: {}", e))?;
    fs::rename(&temp_file, config_file).map_err(|e| format!("Failed to replace config: {}", e))?;
    Ok(())
}

fn config_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        app_handle()?
            .path()
            .app_config_dir()
            .map_err(|e| format!("Failed to resolve app config directory: {}", e))
    }

    #[cfg(not(target_os = "macos"))]
    {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        Ok(home.join(".glucose"))
    }
}

fn watch_progress_dir() -> Result<PathBuf, String> {
    #[cfg(target_os = "macos")]
    {
        config_dir()
    }

    #[cfg(not(target_os = "macos"))]
    {
        let home = dirs::home_dir().ok_or("Could not find home directory")?;
        Ok(home.join(".glucose"))
    }
}

#[cfg(target_os = "macos")]
fn legacy_config_file_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    Ok(home.join(".glucose").join("config.json"))
}

#[cfg(target_os = "macos")]
fn legacy_watch_progress_file_path() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    Ok(home.join(".glucose").join("watch_progress.json"))
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Referenced only by the AI subtitle code, disabled for the Mac App Store build.
fn legacy_models_dir() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find home directory")?;
    Ok(home.join(".whisper").join("models"))
}

#[cfg(target_os = "macos")]
fn migrate_legacy_file(from: &Path, to: &Path) -> Result<(), String> {
    if to.exists() || !from.exists() {
        return Ok(());
    }

    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create destination directory: {}", e))?;
    }

    fs::copy(from, to).map_err(|e| {
        format!(
            "Failed to migrate {} to {}: {}",
            from.display(),
            to.display(),
            e
        )
    })?;

    Ok(())
}

#[cfg(target_os = "macos")]
#[allow(dead_code)] // Referenced only by the AI subtitle code, disabled for the Mac App Store build.
fn migrate_legacy_directory(from: &Path, to: &Path) -> Result<(), String> {
    if !from.exists() {
        return Ok(());
    }

    fs::create_dir_all(to)
        .map_err(|e| format!("Failed to create destination directory: {}", e))?;

    let entries = fs::read_dir(from).map_err(|e| {
        format!(
            "Failed to read legacy models directory {}: {}",
            from.display(),
            e
        )
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
        let source = entry.path();
        let destination = to.join(entry.file_name());

        if source.is_dir() {
            migrate_legacy_directory(&source, &destination)?;
        } else if !destination.exists() {
            fs::copy(&source, &destination).map_err(|e| {
                format!(
                    "Failed to migrate {} to {}: {}",
                    source.display(),
                    destination.display(),
                    e
                )
            })?;
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn active_gallery_bookmarks() -> &'static Mutex<HashMap<String, Retained<NSURL>>> {
    ACTIVE_GALLERY_BOOKMARKS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[cfg(target_os = "macos")]
fn create_security_scoped_bookmark(path: &Path) -> Result<String, String> {
    autoreleasepool(|_| {
        let ns_path = NSString::from_str(path.to_string_lossy().as_ref());
        let url = NSURL::fileURLWithPath_isDirectory(&ns_path, path.is_dir());
        let bookmark = url
            .bookmarkDataWithOptions_includingResourceValuesForKeys_relativeToURL_error(
                NSURLBookmarkCreationOptions::WithSecurityScope
                    | NSURLBookmarkCreationOptions::SecurityScopeAllowOnlyReadAccess,
                None,
                None,
            )
            .map_err(ns_error_message)?;
        let bytes = ns_data_to_vec(&bookmark);
        Ok(BASE64_ENGINE.encode(bytes))
    })
}

#[cfg(target_os = "macos")]
fn resolve_security_scoped_bookmark(bookmark: &str) -> Result<PathBuf, String> {
    let bytes = BASE64_ENGINE
        .decode(bookmark)
        .map_err(|e| format!("Failed to decode bookmark: {}", e))?;

    // Fast path: if this bookmark has already been resolved this session, the
    // retained NSURL is still keeping the security scope alive — reuse it.
    {
        let active = active_gallery_bookmarks()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(url) = active.get(bookmark) {
            return autoreleasepool(|_| {
                let resolved = url.path().ok_or_else(|| {
                    "Resolved bookmark did not contain a filesystem path".to_string()
                })?;
                Ok(PathBuf::from(resolved.to_string()))
            });
        }
    }

    autoreleasepool(|_| {
        let data = unsafe { NSData::dataWithBytes_length(bytes.as_ptr().cast(), bytes.len()) };
        let mut is_stale = Bool::NO;
        let url = unsafe {
            NSURL::URLByResolvingBookmarkData_options_relativeToURL_bookmarkDataIsStale_error(
                &data,
                NSURLBookmarkResolutionOptions::WithSecurityScope
                    | NSURLBookmarkResolutionOptions::WithoutUI,
                None,
                &mut is_stale,
            )
        }
        .map_err(ns_error_message)?;

        let started = unsafe { url.startAccessingSecurityScopedResource() };
        if !started {
            return Err("macOS refused security-scoped access for this folder".to_string());
        }

        if is_stale.as_bool() {
            #[cfg(debug_assertions)]
            eprintln!("Resolved a stale gallery bookmark");
        }

        let resolved = url
            .path()
            .ok_or_else(|| "Resolved bookmark did not contain a filesystem path".to_string())?;
        let path = PathBuf::from(resolved.to_string());

        // Retain the NSURL for the app's lifetime so the security scope stays
        // open. Without this, the asset:// protocol used by <video> via
        // convertFileSrc gets EACCES on every file under this folder.
        let mut active = active_gallery_bookmarks()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        active.insert(bookmark.to_string(), url);

        Ok(path)
    })
}

#[cfg(target_os = "macos")]
fn ns_data_to_vec(data: &NSData) -> Vec<u8> {
    let len = data.length();
    let mut bytes = vec![0_u8; len];
    if !bytes.is_empty() {
        unsafe {
            data.getBytes_length(
                std::ptr::NonNull::new(bytes.as_mut_ptr().cast())
                    .expect("Vec pointer should never be null"),
                len,
            );
        }
    }
    bytes
}

#[cfg(target_os = "macos")]
fn ns_error_message(error: objc2::rc::Retained<NSError>) -> String {
    error.localizedDescription().to_string()
}
