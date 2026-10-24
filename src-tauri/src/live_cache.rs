// Persists live subtitle results per video, audio track and language, so a later live
// session resumes where an earlier one stopped instead of transcribing everything again.
// See "Saving and the live cache" in docs/spikes/live-subtitles.md.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

// Bumped when cached cues would differ from fresh ones (2: Netflix-style cue formatting,
// 3: cues built from word timestamps).
const FORMAT_VERSION: u32 = 3;
// Total size the cache folder is trimmed back to after each write.
const MAX_CACHE_BYTES: u64 = 50 * 1024 * 1024;
// A temporary file older than this was left behind by a crash.
const STALE_TMP_AGE: std::time::Duration = std::time::Duration::from_secs(10 * 60);

#[derive(Clone, Serialize, Deserialize)]
pub struct CachedCue {
    start: f64,
    end: f64,
    text: String,
}

// Identifies one transcription target. A changed size or modification time means the
// file was replaced or re-encoded, so its old cues no longer apply.
#[derive(Clone, PartialEq, Serialize, Deserialize)]
struct CacheKey {
    video_path: String,
    size: u64,
    modified: u64,
    audio_stream_index: Option<i64>,
    language: String,
    // Target language of translated cues. Left out of the file (and the hash) when not
    // translating, so caches written before translation existed still match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    translate_to: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    key: CacheKey,
    model: String,
    ranges: Vec<[f64; 2]>,
    cues: Vec<CachedCue>,
    // Untranslated cues of a translated cache, for dual subtitles. Missing in files
    // written before dual subtitles existed; those parts then show the translation only.
    #[serde(default)]
    original_cues: Vec<CachedCue>,
}

#[derive(Serialize)]
pub struct LiveCacheData {
    model: String,
    ranges: Vec<[f64; 2]>,
    cues: Vec<CachedCue>,
    original_cues: Vec<CachedCue>,
}

fn cache_dir() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir().ok_or("Could not find the local app data directory")?;
    Ok(base.join("glucose").join("live-cache"))
}

fn make_key(
    video_path: &str,
    audio_stream_index: Option<i64>,
    language: &str,
    translate_to: Option<String>,
) -> Result<CacheKey, String> {
    let meta = fs::metadata(video_path).map_err(|e| format!("Failed to read video file: {}", e))?;
    let modified = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Ok(CacheKey {
        video_path: video_path.to_string(),
        size: meta.len(),
        modified,
        audio_stream_index,
        language: language.to_string(),
        translate_to,
    })
}

// FNV-1a, chosen over std's DefaultHasher because its output must stay the same across
// Rust releases for existing cache files to be found.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf29ce484222325;
    for b in bytes {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

fn cache_path(key: &CacheKey) -> Result<PathBuf, String> {
    let id = serde_json::to_vec(key).map_err(|e| e.to_string())?;
    Ok(cache_dir()?.join(format!("{:016x}.json", fnv1a(&id))))
}

#[tauri::command]
pub async fn load_live_cache(
    video_path: String,
    audio_stream_index: Option<i64>,
    language: String,
    translate_to: Option<String>,
) -> Result<Option<LiveCacheData>, String> {
    crate::blocking(move || {
        let key = make_key(&video_path, audio_stream_index, &language, translate_to)?;
        let Ok(raw) = fs::read(cache_path(&key)?) else {
            return Ok(None);
        };
        // An unreadable or outdated file is treated as a miss; the next save replaces it.
        let Ok(file) = serde_json::from_slice::<CacheFile>(&raw) else {
            return Ok(None);
        };
        if file.version != FORMAT_VERSION || file.key != key {
            return Ok(None);
        }
        // Results from a weaker model than the one live mode would use now are not reused,
        // so the better model's output replaces them.
        if let (Some(cached), Some(current)) = (
            super::live_subtitles::model_rank(&file.model),
            super::live_subtitles::preferred_model().and_then(super::live_subtitles::model_rank),
        ) {
            if cached > current {
                return Ok(None);
            }
        }
        Ok(Some(LiveCacheData {
            model: file.model,
            ranges: file.ranges,
            cues: file.cues,
            original_cues: file.original_cues,
        }))
    })
    .await
}

#[tauri::command]
pub async fn save_live_cache(
    video_path: String,
    audio_stream_index: Option<i64>,
    language: String,
    translate_to: Option<String>,
    model: String,
    ranges: Vec<[f64; 2]>,
    cues: Vec<CachedCue>,
    original_cues: Vec<CachedCue>,
) -> Result<(), String> {
    crate::blocking(move || {
        if ranges.is_empty() {
            return Ok(());
        }
        // Saves can now overlap; one at a time, so they never share the temporary file.
        static SAVING: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _saving = SAVING.lock().unwrap_or_else(|e| e.into_inner());
        let key = make_key(&video_path, audio_stream_index, &language, translate_to)?;
        let path = cache_path(&key)?;
        let dir = cache_dir()?;
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create cache folder: {}", e))?;

        let json = serde_json::to_vec(&CacheFile {
            version: FORMAT_VERSION,
            key,
            model,
            ranges,
            cues,
            original_cues,
        })
        .map_err(|e| e.to_string())?;

        // Write then rename, so a crash mid-write never leaves a truncated cache file.
        let tmp = path.with_extension("json.tmp");
        fs::write(&tmp, json).map_err(|e| format!("Failed to write cache: {}", e))?;
        fs::rename(&tmp, &path).map_err(|e| {
            let _ = fs::remove_file(&tmp);
            format!("Failed to write cache: {}", e)
        })?;

        evict(&dir, &path);
        Ok(())
    })
    .await
}

// Removes the least recently written cache files until the folder fits the size cap,
// never `keep` (the file just written, even if it alone is over the cap). Also removes
// temporary files left by a crash between writing and renaming; only old ones, since
// another Glucose window may be writing one right now.
fn evict(dir: &PathBuf, keep: &std::path::Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = Vec::new();
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        let Ok(modified) = meta.modified() else {
            continue;
        };
        if path.to_string_lossy().ends_with(".json.tmp") {
            if modified.elapsed().is_ok_and(|age| age > STALE_TMP_AGE) {
                let _ = fs::remove_file(&path);
            }
        } else if path.extension().is_some_and(|ext| ext == "json") && path != keep {
            files.push((modified, meta.len(), path));
        }
    }
    let keep_len = fs::metadata(keep).map(|m| m.len()).unwrap_or(0);
    let mut total: u64 = keep_len + files.iter().map(|(_, len, _)| len).sum::<u64>();
    if total <= MAX_CACHE_BYTES {
        return;
    }
    files.sort_by_key(|(modified, _, _)| *modified);
    for (_, len, path) in files {
        if total <= MAX_CACHE_BYTES {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            total -= len;
        }
    }
}
