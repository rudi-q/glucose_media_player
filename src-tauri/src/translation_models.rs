// Downloading and removing the translation models from Settings. The models are
// CTranslate2 int8 conversions of the compact OPUS-MT models (Apache-2.0); the originals
// are PyTorch models that the app cannot load. They are hosted as release files of the
// glucose-translation-models repository, named "<model folder>__<file>", with the same
// files the app reads (translate.rs). Downloads land next to the Whisper models.

use std::fs;
use std::io::Write;
use std::path::PathBuf;

use futures_util::StreamExt;
use serde::Serialize;
use tauri::Emitter;

use crate::translate;

// Release holding the model files. A new release (v2, ...) is only needed when models
// are added or replaced; older app versions keep downloading from the release they know.
const MODEL_RELEASE: &str = "https://github.com/rudi-q/glucose-translation-models/releases/download/v1";
// Files in each folder, all required.
const MODEL_FILES: &[&str] = &["config.json", "model.bin", "shared_vocabulary.json", "source.spm", "target.spm"];
// Only the compact models are offered for download.
const DOWNLOADABLE_PREFIX: &str = "opus-mt_tiny_";
// Download size of one compact model, for the Settings list before anything is fetched.
const APPROX_SIZE_MB: u32 = 20;

#[derive(Debug, Clone, Serialize)]
pub struct TranslationModel {
    pub source: String,
    pub target: String,
    pub installed: bool,
    pub size_mb: u32,
}

#[derive(Clone, Serialize)]
struct DownloadProgress {
    source: String,
    target: String,
    downloaded: u64,
    total: u64,
}

fn downloadable() -> impl Iterator<Item = &'static (&'static str, &'static str, &'static str)> {
    translate::MODELS.iter().filter(|(_, _, folder)| folder.starts_with(DOWNLOADABLE_PREFIX))
}

fn folder_for(source: &str, target: &str) -> Result<&'static str, String> {
    downloadable()
        .find(|(s, t, _)| *s == source && *t == target)
        .map(|(_, _, folder)| *folder)
        .ok_or_else(|| {
            format!(
                "There is no downloadable model from {} to {}",
                translate::language_name(source),
                translate::language_name(target)
            )
        })
}

// Where downloads go: the same folder as the Whisper downloads.
fn download_root() -> Result<PathBuf, String> {
    let home = dirs::home_dir().ok_or("Could not find the home folder")?;
    Ok(home.join(".whisper").join("models"))
}

/// The language pairs Settings offers, and which of them are installed.
#[tauri::command]
pub async fn translation_models() -> Vec<TranslationModel> {
    downloadable()
        .map(|(source, target, folder)| TranslationModel {
            source: source.to_string(),
            target: target.to_string(),
            installed: translate::installed_dir(folder).is_some(),
            size_mb: APPROX_SIZE_MB,
        })
        .collect()
}

/// Downloads one language pair. Files go into a temporary folder that only replaces the
/// model folder once every file has arrived, so an interrupted download never leaves a
/// half model behind. Progress is reported as `translation-model-progress` events.
#[tauri::command]
pub async fn download_translation_model(
    app: tauri::AppHandle,
    source: String,
    target: String,
) -> Result<(), String> {
    let folder = folder_for(&source, &target)?;
    let root = download_root()?;
    fs::create_dir_all(&root).map_err(|e| format!("Could not create the models folder: {e}"))?;
    let partial = root.join(format!("{folder}.download"));
    let _ = fs::remove_dir_all(&partial);
    fs::create_dir_all(&partial).map_err(|e| format!("Could not create the download folder: {e}"))?;

    let result = fetch_all(folder, &partial, |downloaded, total| {
        let _ = app.emit(
            "translation-model-progress",
            DownloadProgress { source: source.clone(), target: target.clone(), downloaded, total },
        );
    })
    .await;
    if let Err(e) = result {
        let _ = fs::remove_dir_all(&partial);
        return Err(e);
    }

    let final_dir = root.join(folder);
    let _ = fs::remove_dir_all(&final_dir);
    fs::rename(&partial, &final_dir).map_err(|e| {
        let _ = fs::remove_dir_all(&partial);
        format!("Could not finish installing the model: {e}")
    })
}

// Downloads a model's files into `into`, reporting (downloaded, total) bytes.
async fn fetch_all(
    folder: &str,
    into: &std::path::Path,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), String> {
    let client = reqwest::Client::new();
    let url = |file: &str| format!("{MODEL_RELEASE}/{folder}__{file}");

    // Total size first, so the progress bar covers the whole model.
    let mut total = 0;
    for file in MODEL_FILES {
        let response = client
            .head(url(file))
            .send()
            .await
            .map_err(|e| format!("Could not reach the model server: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("The model server answered {} for {file}", response.status()));
        }
        total += response.content_length().unwrap_or(0);
    }

    let mut downloaded = 0;
    let mut last_reported = 0;
    for file in MODEL_FILES {
        let response = client
            .get(url(file))
            .send()
            .await
            .map_err(|e| format!("Could not download {file}: {e}"))?;
        if !response.status().is_success() {
            return Err(format!("The model server answered {} for {file}", response.status()));
        }
        let mut out = fs::File::create(into.join(file)).map_err(|e| format!("Could not save {file}: {e}"))?;
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download of {file} failed: {e}"))?;
            out.write_all(&chunk).map_err(|e| format!("Could not save {file}: {e}"))?;
            downloaded += chunk.len() as u64;
            // About every 512 KB, and at the end.
            if downloaded - last_reported >= 512 * 1024 || downloaded == total {
                last_reported = downloaded;
                on_progress(downloaded, total);
            }
        }
        out.flush().map_err(|e| format!("Could not save {file}: {e}"))?;
    }
    Ok(())
}

/// Removes an installed language pair.
#[tauri::command]
pub async fn delete_translation_model(source: String, target: String) -> Result<(), String> {
    let folder = folder_for(&source, &target)?;
    let dir = translate::installed_dir(folder).ok_or("This model is not installed")?;
    fs::remove_dir_all(&dir).map_err(|e| format!("Could not remove the model: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn offers_only_the_compact_models() {
        let pairs: Vec<_> = downloadable().map(|(s, t, _)| format!("{s}-{t}")).collect();
        assert_eq!(pairs.len(), 14);
        assert!(downloadable().all(|(_, _, f)| f.starts_with(DOWNLOADABLE_PREFIX)));
        assert!(folder_for("en", "de").is_ok());
        assert!(folder_for("en", "xx").is_err());
    }

    // Downloads English -> German from the release and compares it with the installed copy;
    // needs the network and that model installed. Run with
    // `cargo test --lib downloads_a_model -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn downloads_a_model() {
        let folder = folder_for("en", "de").unwrap();
        let installed = translate::installed_dir(folder).expect("en-de model installed");
        let into = std::env::temp_dir().join(format!("glucose_tm_test_{}", std::process::id()));
        let _ = fs::remove_dir_all(&into);
        fs::create_dir_all(&into).unwrap();
        let mut last = (0, 0);
        tauri::async_runtime::block_on(fetch_all(folder, &into, |d, t| last = (d, t))).unwrap();
        println!("downloaded {} of {} bytes", last.0, last.1);
        assert!(last.1 > 0 && last.0 == last.1);
        for file in MODEL_FILES {
            assert_eq!(fs::read(into.join(file)).unwrap(), fs::read(installed.join(file)).unwrap(), "{file}");
        }
        let _ = fs::remove_dir_all(&into);
    }
}
