// Persists live subtitle results per video, audio track and language, so a later live
// session resumes where an earlier one stopped instead of transcribing everything again.
// See "Saving and the live cache" in docs/spikes/live-subtitles.md.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

// Bumped when cached cues would differ from fresh ones (2: Netflix-style cue formatting).
const FORMAT_VERSION: u32 = 2;
// Total size the cache folder is trimmed back to after each write.
const MAX_CACHE_BYTES: u64 = 50 * 1024 * 1024;

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
}

#[derive(Serialize, Deserialize)]
struct CacheFile {
    version: u32,
    key: CacheKey,
    model: String,
    ranges: Vec<[f64; 2]>,
    cues: Vec<CachedCue>,
}

#[derive(Serialize)]
pub struct LiveCacheData {
    model: String,
    ranges: Vec<[f64; 2]>,
    cues: Vec<CachedCue>,
}

fn cache_dir() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir().ok_or("Could not find the local app data directory")?;
    Ok(base.join("glucose").join("live-cache"))
}

fn make_key(
    video_path: &str,
    audio_stream_index: Option<i64>,
    language: &str,
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
pub fn load_live_cache(
    video_path: String,
    audio_stream_index: Option<i64>,
    language: String,
) -> Result<Option<LiveCacheData>, String> {
    let key = make_key(&video_path, audio_stream_index, &language)?;
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
    }))
}

#[tauri::command]
pub fn save_live_cache(
    video_path: String,
    audio_stream_index: Option<i64>,
    language: String,
    model: String,
    ranges: Vec<[f64; 2]>,
    cues: Vec<CachedCue>,
) -> Result<(), String> {
    if ranges.is_empty() {
        return Ok(());
    }
    let key = make_key(&video_path, audio_stream_index, &language)?;
    let path = cache_path(&key)?;
    let dir = cache_dir()?;
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create cache folder: {}", e))?;

    let json = serde_json::to_vec(&CacheFile {
        version: FORMAT_VERSION,
        key,
        model,
        ranges,
        cues,
    })
    .map_err(|e| e.to_string())?;

    // Write then rename, so a crash mid-write never leaves a truncated cache file.
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| format!("Failed to write cache: {}", e))?;
    fs::rename(&tmp, &path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("Failed to write cache: {}", e)
    })?;

    evict(&dir);
    Ok(())
}

// Removes the least recently written cache files until the folder fits the size cap.
fn evict(dir: &PathBuf) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "json"))
        .filter_map(|e| {
            let meta = e.metadata().ok()?;
            Some((meta.modified().ok()?, meta.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|(_, len, _)| len).sum();
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
