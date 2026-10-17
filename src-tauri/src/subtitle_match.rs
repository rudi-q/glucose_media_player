// Finds subtitle files that belong to a video even when the names differ, as with
// "Some.Movie.2021.1080p.WEBRip.x264.mp4" and "Some.Movie.2022.WEB-DL.HI-English.srt": both
// reduce to the title "some movie". Looks next to the video and in Subs/Subtitles folders.

use std::fs;
use std::path::{Path, PathBuf};

use serde::Serialize;

const SUBTITLE_EXTS: [&str; 5] = ["srt", "vtt", "ass", "ssa", "sub"];
// Folders release groups commonly put subtitles in.
const SUBTITLE_DIRS: [&str; 3] = ["subs", "subtitles", "sub"];
// Other subtitle files in the folder are only offered when there are this few, so a
// season folder full of episode subtitles does not flood the menu.
const MAX_OTHER: usize = 5;

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MatchKind {
    // Same file name as the video.
    Exact,
    // Same title (and episode), different release details.
    Title,
    // Some other subtitle file nearby.
    Other,
}

#[derive(Debug, Clone, Serialize)]
pub struct NearbySubtitle {
    pub path: String,
    pub name: String,
    pub kind: MatchKind,
}

// Words that start the release details after a title.
const RELEASE_WORDS: [&str; 40] = [
    "720p", "1080p", "1080i", "2160p", "4k", "uhd", "hdr", "hdr10", "dv", "webrip", "web",
    "webdl", "bluray", "bdrip", "brrip", "dvdrip", "hdtv", "hdrip", "remux", "x264", "x265",
    "h264", "h265", "hevc", "avc", "aac", "aac5", "ac3", "dd5", "ddp5", "dts", "atmos",
    "proper", "repack", "extended", "unrated", "remastered", "internal", "limited", "imax",
];

// Language and accessibility tags subtitle files add after the title. Only removed from
// the end, so "The.English.Patient" keeps its title.
const TAG_WORDS: [&str; 24] = [
    "en", "eng", "english", "hi", "sdh", "cc", "forced", "full", "subs", "subtitles", "de",
    "ger", "german", "fr", "fre", "french", "es", "spa", "spanish", "it", "ita", "italian",
    "nl", "dutch",
];

fn words(stem: &str) -> Vec<String> {
    stem.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect()
}

fn is_year(w: &str) -> bool {
    w.len() == 4 && (w.starts_with("19") || w.starts_with("20")) && w.chars().all(|c| c.is_ascii_digit())
}

// "s01e02", "1x02" style episode markers.
fn episode(w: &str) -> Option<(u32, u32)> {
    let w = w.trim_start_matches('s');
    let (season, ep) = w.split_once('e').or_else(|| w.split_once('x'))?;
    Some((season.parse().ok()?, ep.parse().ok()?))
}

/// The title (plus episode, for TV) a file name is about: everything before the first
/// year or release detail. None when there is no usable title.
pub fn title_key(stem: &str) -> Option<String> {
    let mut title = Vec::new();
    let mut ep = None;
    for w in words(stem) {
        if let Some(e) = episode(&w) {
            ep = Some(e);
            break;
        }
        // A year right at the start is the title (e.g. "1917").
        if (is_year(&w) && !title.is_empty()) || RELEASE_WORDS.contains(&w.as_str()) {
            break;
        }
        title.push(w);
    }
    while title.len() > 1 && title.last().is_some_and(|w| TAG_WORDS.contains(&w.as_str())) {
        title.pop();
    }
    if title.is_empty() {
        return None;
    }
    let mut key = title.join(" ");
    if let Some((s, e)) = ep {
        key.push_str(&format!(" s{s:02}e{e:02}"));
    }
    Some(key)
}

fn subtitle_files(dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| SUBTITLE_EXTS.contains(&e.to_lowercase().as_str()))
        })
        .collect()
}

/// Subtitle files near a video, best matches first.
pub fn nearby_subtitles(video_path: &Path) -> Vec<NearbySubtitle> {
    let Some(dir) = video_path.parent() else {
        return Vec::new();
    };
    let video_stem = video_path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
    let video_key = title_key(&video_stem);

    let mut files = subtitle_files(dir);
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let is_sub_dir = path.is_dir()
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| SUBTITLE_DIRS.contains(&n.to_lowercase().as_str()));
            if is_sub_dir {
                files.extend(subtitle_files(&path));
            }
        }
    }

    let mut found: Vec<NearbySubtitle> = files
        .into_iter()
        .map(|path| {
            let stem = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()).unwrap_or_default();
            let kind = if stem == video_stem {
                MatchKind::Exact
            } else if video_key.is_some() && title_key(&stem) == video_key {
                MatchKind::Title
            } else {
                MatchKind::Other
            };
            NearbySubtitle {
                name: path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default(),
                path: path.to_string_lossy().to_string(),
                kind,
            }
        })
        .collect();
    let others = found.iter().filter(|s| s.kind == MatchKind::Other).count();
    if others > MAX_OTHER {
        found.retain(|s| s.kind != MatchKind::Other);
    }
    let rank = |k: &MatchKind| match k {
        MatchKind::Exact => 0,
        MatchKind::Title => 1,
        MatchKind::Other => 2,
    };
    found.sort_by(|a, b| rank(&a.kind).cmp(&rank(&b.kind)).then_with(|| a.name.cmp(&b.name)));
    found
}

#[tauri::command]
pub async fn find_nearby_subtitles(video_path: String) -> Result<Vec<NearbySubtitle>, String> {
    tauri::async_runtime::spawn_blocking(move || nearby_subtitles(Path::new(&video_path)))
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn different_releases_of_a_film_share_a_title() {
        let video = title_key("All.My.Friends.Hate.Me.2021.1080p.WEBRip.x264.AAC5.1-[YTS.MX]");
        let subs = title_key("All.My.Friends.Hate.Me.2022.1080p.WEB-DL.DD5.1.H.264-EVO-HI-English");
        assert_eq!(video.as_deref(), Some("all my friends hate me"));
        assert_eq!(video, subs);
    }

    #[test]
    fn episodes_stay_apart() {
        let e2 = title_key("Some.Show.S01E02.1080p.WEB.h264");
        assert_eq!(e2.as_deref(), Some("some show s01e02"));
        assert_ne!(e2, title_key("Some.Show.S01E03.720p.HDTV"));
        assert_eq!(e2, title_key("some show 1x02 english"));
    }

    #[test]
    fn language_tags_are_dropped_only_at_the_end() {
        assert_eq!(title_key("Some.Movie.English.SDH").as_deref(), Some("some movie"));
        assert_eq!(title_key("The.English.Patient.eng").as_deref(), Some("the english patient"));
    }

    #[test]
    fn a_year_can_be_the_title() {
        assert_eq!(title_key("1917.2019.1080p.BluRay").as_deref(), Some("1917"));
    }

    #[test]
    fn finds_title_matches_and_subtitle_folders() {
        let dir = std::env::temp_dir().join(format!("glucose_subs_{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("Subs")).unwrap();
        let video = dir.join("Some.Movie.2021.1080p.WEBRip.x264.mp4");
        for f in [
            "Some.Movie.2021.1080p.WEBRip.x264.srt",
            "Some.Movie.2022.WEB-DL.HI-English.srt",
            "Unrelated.Film.2019.srt",
            "Subs/Some.Movie.English.srt",
        ] {
            fs::write(dir.join(f), "").unwrap();
        }
        let found = nearby_subtitles(&video);
        let _ = fs::remove_dir_all(&dir);
        let summary: Vec<(String, MatchKind)> = found.into_iter().map(|s| (s.name, s.kind)).collect();
        assert_eq!(
            summary,
            vec![
                ("Some.Movie.2021.1080p.WEBRip.x264.srt".into(), MatchKind::Exact),
                ("Some.Movie.2022.WEB-DL.HI-English.srt".into(), MatchKind::Title),
                ("Some.Movie.English.srt".into(), MatchKind::Title),
                ("Unrelated.Film.2019.srt".into(), MatchKind::Other),
            ]
        );
    }
}
