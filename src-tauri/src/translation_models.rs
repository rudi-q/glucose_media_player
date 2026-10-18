// Downloading and removing the translation models from Settings. The models are
// CTranslate2 int8 conversions of the compact OPUS-MT models (Apache-2.0); the originals
// are PyTorch models that the app cannot load. They are hosted as release files of the
// glucose-translation-models repository, named "<model folder>__<file>", with the same
// files the app reads (translate.rs). Downloads land next to the Whisper models.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Serialize;
use sha2::{Digest, Sha256};
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
// A download gives up when connecting, or waiting for the next piece of data, takes longer
// than this; there is no limit on the whole download, so slow connections still finish.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(20);
const READ_TIMEOUT: Duration = Duration::from_secs(30);

// SHA-256 of every release file ("<model folder>__<file>"), matching SHA256SUMS.txt in the
// release. A file that does not match is rejected, so a broken download or a replaced
// release file is never installed. Adding a model means adding its five lines here.
const SHA256: &[(&str, &str)] = &[
    ("opus-mt_tiny_deu-eng__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_deu-eng__model.bin", "aa5058a6bca0316437ef6c9726d47d701610b4a1f69fb398af67fdfd85c01284"),
    ("opus-mt_tiny_deu-eng__shared_vocabulary.json", "6464d4dbd7fb96c868d4c1d83d9de6cf48610d1879a2c4741eeea0dcd5e34f04"),
    ("opus-mt_tiny_deu-eng__source.spm", "b0c0d8de2d7f020afbfe2c9bd86c60cafc340b3d0cf45b969ab13313c462e0fb"),
    ("opus-mt_tiny_deu-eng__target.spm", "b0c0d8de2d7f020afbfe2c9bd86c60cafc340b3d0cf45b969ab13313c462e0fb"),
    ("opus-mt_tiny_eng-deu__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_eng-deu__model.bin", "16b17fa3a418394602901dc20cc46068e57bb86ce2b255d0b9de770872c601b0"),
    ("opus-mt_tiny_eng-deu__shared_vocabulary.json", "25d30acc3f9a999eaeb2bf4a4bb4f8e8de1dc860a3231aaff459640f4c50cd0f"),
    ("opus-mt_tiny_eng-deu__source.spm", "84ce38d6b0dca4b60e3d3e098e68995220837e4c92a661dca0afb065fc8f732b"),
    ("opus-mt_tiny_eng-deu__target.spm", "84ce38d6b0dca4b60e3d3e098e68995220837e4c92a661dca0afb065fc8f732b"),
    ("opus-mt_tiny_eng-fra__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_eng-fra__model.bin", "e79267bdefa2e1326ea1c531a6a6569934260aff63beb9111814daeb5055c64b"),
    ("opus-mt_tiny_eng-fra__shared_vocabulary.json", "b616b397c01b3bd8be2e5f4ed825baaf2245efdd3cd592cee6e5ce561c2478c4"),
    ("opus-mt_tiny_eng-fra__source.spm", "fe73acaf706d8f09cc32f9de0096ca59ab6bfe2ac4597705d1781233c51193fe"),
    ("opus-mt_tiny_eng-fra__target.spm", "fe73acaf706d8f09cc32f9de0096ca59ab6bfe2ac4597705d1781233c51193fe"),
    ("opus-mt_tiny_eng-ita__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_eng-ita__model.bin", "91ea7c6d62d2f6ad0a77aa9df0709c180d002470e89f200105c98438a8f2ea53"),
    ("opus-mt_tiny_eng-ita__shared_vocabulary.json", "cdabb4c71a04a99164e2af1426a4bb1f2af78190f8a982856be07e91d4bb8295"),
    ("opus-mt_tiny_eng-ita__source.spm", "f56349a46f0f0ebcc0721d6bf966fa2ba130fe748ecf382172665544d6bd33e2"),
    ("opus-mt_tiny_eng-ita__target.spm", "f56349a46f0f0ebcc0721d6bf966fa2ba130fe748ecf382172665544d6bd33e2"),
    ("opus-mt_tiny_eng-nld__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_eng-nld__model.bin", "4143dceecb55bd00e24228cd49993c4bbbd0db2119c4febb86ef98e1008b745e"),
    ("opus-mt_tiny_eng-nld__shared_vocabulary.json", "c08b06e4f28c511f459bf2886064a7f179911ac64e334e8c186c1674a3271eaa"),
    ("opus-mt_tiny_eng-nld__source.spm", "e51e6327c73f871fe120025595215a03a68b58cde757ea510da9558acf79e07f"),
    ("opus-mt_tiny_eng-nld__target.spm", "e51e6327c73f871fe120025595215a03a68b58cde757ea510da9558acf79e07f"),
    ("opus-mt_tiny_eng-rus__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_eng-rus__model.bin", "8f18ba5920985d28eb099595f350e2c2f608a24b1181a6d46114770ad2f74bca"),
    ("opus-mt_tiny_eng-rus__shared_vocabulary.json", "a1e1c12e43ec042eb5f18029c33985a0bb1bda9f1a4a1b5ff0456e0af6258e1f"),
    ("opus-mt_tiny_eng-rus__source.spm", "8282335e85cc36ac5ec1cba7ce18399ce948bf72f3c755b1aced54a13b373c02"),
    ("opus-mt_tiny_eng-rus__target.spm", "8282335e85cc36ac5ec1cba7ce18399ce948bf72f3c755b1aced54a13b373c02"),
    ("opus-mt_tiny_eng-spa__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_eng-spa__model.bin", "b1c24fbceb5211130510b04ead066a8771abcde6a5e73be4ab5b8d93ce40753c"),
    ("opus-mt_tiny_eng-spa__shared_vocabulary.json", "5f47610a7c51abf39ca871833fad1563eb2641276533ecad044b260df89e1472"),
    ("opus-mt_tiny_eng-spa__source.spm", "a8aac80a036107c9c0b1e670f24045d869e39cec21c15936c4670d30676810d6"),
    ("opus-mt_tiny_eng-spa__target.spm", "a8aac80a036107c9c0b1e670f24045d869e39cec21c15936c4670d30676810d6"),
    ("opus-mt_tiny_eng-tur__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_eng-tur__model.bin", "3ef5eb8186d45514224bc654cc3831bb2ddba972a14db978dc2d38e79f7db7f2"),
    ("opus-mt_tiny_eng-tur__shared_vocabulary.json", "fa5af0068db48fee562eb2e68c41ce7c4dab29fcd360b8e2d46025447e52e5d4"),
    ("opus-mt_tiny_eng-tur__source.spm", "fe55e84b94d17b424eafba19d1e4815a90b23cd52138b9cdb1072d1b65c9795c"),
    ("opus-mt_tiny_eng-tur__target.spm", "fe55e84b94d17b424eafba19d1e4815a90b23cd52138b9cdb1072d1b65c9795c"),
    ("opus-mt_tiny_fra-eng__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_fra-eng__model.bin", "d743ec94f8faf7edf7c7be99d9135258aee83dade21e0b631be88e0f4805ca13"),
    ("opus-mt_tiny_fra-eng__shared_vocabulary.json", "f1030bff4ddbc36be8a5812a4950c16af4a65bb5c0844c6b4352d584dc8693fd"),
    ("opus-mt_tiny_fra-eng__source.spm", "878ba36f64bf4025f296526ce725521839008fb5b69566b10e859e8617601421"),
    ("opus-mt_tiny_fra-eng__target.spm", "878ba36f64bf4025f296526ce725521839008fb5b69566b10e859e8617601421"),
    ("opus-mt_tiny_ita-eng__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_ita-eng__model.bin", "7417e60fa978c3135cf5e710debb8912baf00dd14b1c753a7a1d5075ac9d17d1"),
    ("opus-mt_tiny_ita-eng__shared_vocabulary.json", "51c27f9a43095897dd3653b06e73a3e371d6a752ff5b84f54c7a0a794e39a9bf"),
    ("opus-mt_tiny_ita-eng__source.spm", "41026f577bfdf1b448d1383910a7878eee9b202202c972f8f13d7a8c4d1352d7"),
    ("opus-mt_tiny_ita-eng__target.spm", "41026f577bfdf1b448d1383910a7878eee9b202202c972f8f13d7a8c4d1352d7"),
    ("opus-mt_tiny_nld-eng__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_nld-eng__model.bin", "0272cd16f8b7947ec454a8ca72a8a73a9ccc1ca6efa9187ddb1e05e9baa9acdf"),
    ("opus-mt_tiny_nld-eng__shared_vocabulary.json", "7bed4cef91bae2332501b3412910b017b806c80eafb66245aa11b2dc2a33b37f"),
    ("opus-mt_tiny_nld-eng__source.spm", "9d0fb485b5d5fa554d8c30192a6a94ad729ed4bbfa3ff820b55eeec4e8ca8855"),
    ("opus-mt_tiny_nld-eng__target.spm", "9d0fb485b5d5fa554d8c30192a6a94ad729ed4bbfa3ff820b55eeec4e8ca8855"),
    ("opus-mt_tiny_rus-eng__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_rus-eng__model.bin", "4e69a2c5603c429764db602d010ad487d601dc62d82fbfcdd71932569884da8e"),
    ("opus-mt_tiny_rus-eng__shared_vocabulary.json", "16c8e00da2891a861bf0bb814a00f26a0e86251ad4c896c5c6dfe857a3b62e09"),
    ("opus-mt_tiny_rus-eng__source.spm", "e684e7edf0c2a134665ac393e09e342175c7f909549e7aa0cbf0ad27390729f1"),
    ("opus-mt_tiny_rus-eng__target.spm", "e684e7edf0c2a134665ac393e09e342175c7f909549e7aa0cbf0ad27390729f1"),
    ("opus-mt_tiny_spa-eng__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_spa-eng__model.bin", "f8a5a564a66405859b85b28388d08f0c569c4e948307ddc81a4bc9061d8441e4"),
    ("opus-mt_tiny_spa-eng__shared_vocabulary.json", "bf56cb5e31ba1057ff3434bda6b5ce7697318afcf081cc345914f135ccf06d40"),
    ("opus-mt_tiny_spa-eng__source.spm", "a40ad94d67f431248d5dec9c7f0ded19efce671568ff07c959ee58ca50590265"),
    ("opus-mt_tiny_spa-eng__target.spm", "a40ad94d67f431248d5dec9c7f0ded19efce671568ff07c959ee58ca50590265"),
    ("opus-mt_tiny_tur-eng__config.json", "72901fbd8abd89fb5cf4a388f26fc681f5c4c58a1e1a88b30b879f107270e7ee"),
    ("opus-mt_tiny_tur-eng__model.bin", "6b397fe7137f94f48da12c045772f0a1b65daed57c1ccc63966db00caaa2e2f9"),
    ("opus-mt_tiny_tur-eng__shared_vocabulary.json", "e3489a396011897afc2447fa038121707e417925bd297a9538e763636519e185"),
    ("opus-mt_tiny_tur-eng__source.spm", "514637f7fbe60aaf50758d3b57dae3a57ed683bd30b6996c719a7c1b8c813641"),
    ("opus-mt_tiny_tur-eng__target.spm", "514637f7fbe60aaf50758d3b57dae3a57ed683bd30b6996c719a7c1b8c813641"),
];

fn expected_sha256(asset: &str) -> Option<&'static str> {
    SHA256.iter().find(|(name, _)| *name == asset).map(|(_, digest)| *digest)
}

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
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .build()
        .map_err(|e| format!("Could not set up the download: {e}"))?;
    let asset = |file: &str| format!("{folder}__{file}");
    let url = |file: &str| format!("{MODEL_RELEASE}/{}", asset(file));
    for file in MODEL_FILES {
        if expected_sha256(&asset(file)).is_none() {
            return Err(format!("No checksum is known for {}", asset(file)));
        }
    }

    // Total size first, so the progress bar covers the whole model. Read from the header
    // itself: for a HEAD request the body is empty, so the client's own length can be 0.
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
        total += response
            .headers()
            .get(reqwest::header::CONTENT_LENGTH)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(0);
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
        let mut hasher = Sha256::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("Download of {file} failed: {e}"))?;
            hasher.update(&chunk);
            out.write_all(&chunk).map_err(|e| format!("Could not save {file}: {e}"))?;
            downloaded += chunk.len() as u64;
            // About every 512 KB, and at the end.
            if downloaded - last_reported >= 512 * 1024 || downloaded == total {
                last_reported = downloaded;
                on_progress(downloaded, total);
            }
        }
        out.flush().map_err(|e| format!("Could not save {file}: {e}"))?;
        let digest: String = hasher.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if Some(digest.as_str()) != expected_sha256(&asset(file)) {
            return Err(format!("{file} did not download correctly (checksum mismatch). Please try again."));
        }
    }
    Ok(())
}

/// Removes an installed language pair from every models folder that has a copy, so it no
/// longer counts as installed.
#[tauri::command]
pub async fn delete_translation_model(source: String, target: String) -> Result<(), String> {
    let folder = folder_for(&source, &target)?;
    let copies: Vec<PathBuf> = crate::model_candidate_dirs()
        .into_iter()
        .map(|dir| dir.join(folder))
        .filter(|dir| dir.is_dir())
        .collect();
    if copies.is_empty() {
        return Err("This model is not installed".into());
    }
    let failed: Vec<String> = copies
        .iter()
        .filter_map(|dir| fs::remove_dir_all(dir).err().map(|e| format!("{}: {e}", dir.display())))
        .collect();
    if failed.is_empty() {
        Ok(())
    } else {
        Err(format!("Could not remove the model: {}", failed.join("; ")))
    }
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

    #[test]
    fn every_downloadable_file_has_a_checksum() {
        for (_, _, folder) in downloadable() {
            for file in MODEL_FILES {
                let digest = expected_sha256(&format!("{folder}__{file}"));
                assert!(digest.is_some_and(|d| d.len() == 64), "{folder}__{file}");
            }
        }
        assert_eq!(SHA256.len(), 14 * MODEL_FILES.len());
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
