// Serves local media files to the webview (asset://, what convertFileSrc points at).
// Replaces Tauri's built-in asset protocol, which reads the file on the window thread:
// a file that is slow to read, like a OneDrive file that is still downloading, then froze
// the whole window ("Not responding") until the read finished. Here every request is read
// on a background thread and the window keeps running.

use crate::live_log;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use tauri::http::{header::*, Method, Request, Response, StatusCode};

// The most bytes sent for one range request, as in Tauri's own handler.
const MAX_LEN: u64 = 1000 * 1024;
// Reads slower than this are written to the development log.
const SLOW_MS: u64 = 1000;

type Served = Result<Response<Vec<u8>>, Box<dyn std::error::Error>>;

// Pages allowed to read media through this protocol with CORS (the player's <video> asks
// with crossorigin, for face detection): the app's own origin on Windows and macOS, and
// the dev server in debug builds. Other origins get no Access-Control-Allow-Origin.
fn allowed_origin(origin: &str) -> bool {
    matches!(origin, "http://tauri.localhost" | "https://tauri.localhost" | "tauri://localhost")
        || (cfg!(debug_assertions) && origin == "http://localhost:2060")
}

// A response builder with the CORS headers for `origin` (empty when not allowed).
fn builder(origin: &str) -> tauri::http::response::Builder {
    let builder = Response::builder().header(VARY, "Origin");
    if origin.is_empty() {
        builder
    } else {
        builder.header(ACCESS_CONTROL_ALLOW_ORIGIN, origin)
    }
}

pub(crate) fn handle(request: Request<Vec<u8>>, responder: tauri::UriSchemeResponder) {
    tauri::async_runtime::spawn_blocking(move || {
        let origin = request
            .headers()
            .get(ORIGIN)
            .and_then(|o| o.to_str().ok())
            .filter(|o| allowed_origin(o))
            .unwrap_or("")
            .to_string();
        let started = std::time::Instant::now();
        // The error text can hold paths and system messages; the page gets none of it.
        let response = serve(&request, &origin).unwrap_or_else(|e| {
            if cfg!(debug_assertions) {
                eprintln!("[media protocol] {e}");
            }
            builder(&origin)
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(b"Could not read the file".to_vec())
                .unwrap()
        });
        let took = live_log::ms(started);
        if took > SLOW_MS {
            live_log::log(
                "media_read_slow",
                serde_json::json!({
                    "file": live_log::file_name(&request_path(&request)),
                    "ms": took,
                    "status": response.status().as_u16(),
                }),
            );
        }
        responder.respond(response);
    });
}

// The file path from asset://localhost/<percent-encoded path>.
fn request_path(request: &Request<Vec<u8>>) -> String {
    let encoded = request.uri().path().trim_start_matches('/');
    String::from_utf8_lossy(&urlencoding::decode_binary(encoded.as_bytes())).into_owned()
}

// Only video and audio files the player opens are served: everything the app loads this
// way is media (the player, the audio player, gallery previews and thumbnails, and the
// temporary .mkv made when switching audio tracks). Anything else, such as keys or
// documents, is refused, so a script that got into the webview could not read other files
// through this protocol.
// Paths in the Windows device namespace (\\.\, \\?\GLOBALROOT) and reserved device names
// such as CON or NUL, even with an extension, are refused too: reading one can block a
// thread forever. Network shares (\\server\share) stay allowed for videos on a NAS.
fn is_servable(path: &str) -> bool {
    let p = std::path::Path::new(path);
    let media = p
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| crate::is_playable_extension(&e.to_lowercase()));
    // Device paths and names are a Windows matter; on other systems "aux.mp3" is a file.
    if !cfg!(windows) {
        return media;
    }
    let normalized = path.replace('/', "\\").to_uppercase();
    let device_path = normalized.starts_with("\\\\.\\") || normalized.starts_with("\\\\?\\GLOBALROOT");
    let device_name = p
        .file_stem()
        .and_then(|s| s.to_str())
        .map(|s| s.trim_end_matches([' ', '.']).to_uppercase())
        .is_some_and(|s| {
            matches!(s.as_str(), "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$")
                || (s.len() == 4
                    && (s.starts_with("COM") || s.starts_with("LPT"))
                    && s.ends_with(|c: char| c.is_ascii_digit()))
        });
    media && !device_path && !device_name
}

// Content type by extension; every servable extension is listed, so files are not read
// on each range request to sniff it.
fn mime_type(path: &str) -> &'static str {
    let ext = std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_lowercase)
        .unwrap_or_default();
    match ext.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "mpg" | "mpeg" => "video/mpeg",
        "ogv" => "video/ogg",
        "ts" | "m2ts" | "mts" => "video/mp2t",
        "3gp" => "video/3gpp",
        "3g2" => "video/3gpp2",
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "aac" => "audio/aac",
        "ogg" | "opus" => "audio/ogg",
        "m4a" | "m4b" => "audio/mp4",
        "aiff" => "audio/aiff",
        "wma" => "audio/x-ms-wma",
        "mka" => "audio/x-matroska",
        "wv" => "audio/wavpack",
        "ape" => "audio/ape",
        _ => "application/octet-stream",
    }
}

fn serve(request: &Request<Vec<u8>>, origin: &str) -> Served {
    let path = request_path(request);
    let resp = builder(origin);
    // Only absolute paths: a relative one would be read from the working directory.
    if !std::path::Path::new(&path).is_absolute()
        || tauri::path::SafePathBuf::new(path.clone().into()).is_err()
        || !is_servable(&path)
    {
        return Ok(resp.status(StatusCode::FORBIDDEN).body(Vec::new())?);
    }
    let mut file = match File::open(&path) {
        Ok(file) => file,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(resp.status(StatusCode::NOT_FOUND).body(Vec::new())?);
        }
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            return Ok(resp.status(StatusCode::FORBIDDEN).body(Vec::new())?);
        }
        Err(e) => return Err(e.into()),
    };
    let metadata = file.metadata()?;
    // A folder named like a video, or a device, is not a file to serve.
    if !metadata.is_file() {
        return Ok(resp.status(StatusCode::FORBIDDEN).body(Vec::new())?);
    }
    // A .ts file that is TypeScript source rather than a video is not served.
    if path.to_lowercase().ends_with(".ts") && !crate::starts_like_transport_stream(&mut file) {
        return Ok(resp.status(StatusCode::FORBIDDEN).body(Vec::new())?);
    }
    let len = metadata.len();

    let resp = resp
        .header(CONTENT_TYPE, mime_type(&path))
        .header(ACCEPT_RANGES, "bytes")
        .header(ACCESS_CONTROL_EXPOSE_HEADERS, "content-range");

    if request.method() == Method::HEAD {
        return Ok(resp.header(CONTENT_LENGTH, len).body(Vec::new())?);
    }

    // A range whose end comes before its start is invalid and ignored (RFC 9110), as if
    // the request had none.
    let range = request
        .headers()
        .get(RANGE)
        .and_then(|r| r.to_str().ok())
        .filter(|r| !is_reversed_range(r));
    let Some(range) = range else {
        if len <= MAX_LEN {
            file.rewind()?;
            let mut buf = Vec::with_capacity(len as usize);
            file.read_to_end(&mut buf)?;
            return Ok(resp.header(CONTENT_LENGTH, len).body(buf)?);
        }
        // Every caller is a <video> or <audio> element (player, previews, thumbnails, the
        // audio player), and media elements ask for byte ranges. A larger file asked for
        // whole cannot be answered: the body is not streamed and a multi-gigabyte video is
        // never read into memory. A partial 206 would be invalid without a Range header
        // and look like the whole file to a plain fetch, so the answer is 416, giving the
        // size so the caller can ask for ranges instead.
        return Ok(builder(origin)
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(ACCEPT_RANGES, "bytes")
            .header(CONTENT_RANGE, format!("bytes */{len}"))
            .body(Vec::new())?);
    };
    let Some((start, end)) = parse_range(range, len) else {
        return Ok(builder(origin)
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(CONTENT_RANGE, format!("bytes */{len}"))
            .body(Vec::new())?);
    };
    let end = end.min(start + MAX_LEN - 1);
    let nbytes = end + 1 - start;
    let mut buf = Vec::with_capacity(nbytes as usize);
    file.seek(SeekFrom::Start(start))?;
    file.take(nbytes).read_to_end(&mut buf)?;
    // The file can shrink after its size was read (a temporary audio-track file still being
    // written): the headers describe the bytes actually read.
    if buf.is_empty() {
        return Ok(builder(origin)
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(CONTENT_RANGE, format!("bytes */{len}"))
            .body(Vec::new())?);
    }
    let end = start + buf.len() as u64 - 1;
    Ok(resp
        .status(StatusCode::PARTIAL_CONTENT)
        .header(CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
        .header(CONTENT_LENGTH, buf.len())
        .body(buf)?)
}

// "bytes=500-100": both ends given, the end before the start.
fn is_reversed_range(header: &str) -> bool {
    let spec = header.trim().strip_prefix("bytes=").and_then(|s| s.split(',').next()).unwrap_or("");
    match spec.trim().split_once('-') {
        Some((from, to)) => matches!((from.parse::<u64>(), to.parse::<u64>()), (Ok(f), Ok(t)) if t < f),
        None => false,
    }
}

// The first range of a "bytes=" header as inclusive (start, end), clamped to the file.
// Media elements only ask for one range at a time.
fn parse_range(header: &str, len: u64) -> Option<(u64, u64)> {
    let last = len.checked_sub(1)?;
    let spec = header.trim().strip_prefix("bytes=")?.split(',').next()?.trim();
    let (from, to) = spec.split_once('-')?;
    let (start, end) = if from.is_empty() {
        let suffix: u64 = to.parse().ok()?;
        if suffix == 0 {
            return None;
        }
        (len.saturating_sub(suffix), last)
    } else {
        let start: u64 = from.parse().ok()?;
        let end = if to.is_empty() { last } else { to.parse::<u64>().ok()?.min(last) };
        (start, end)
    };
    (start <= end).then_some((start, end))
}

#[cfg(test)]
mod tests {
    use super::{allowed_origin, is_reversed_range, is_servable, mime_type, parse_range, MAX_LEN};
    use tauri::http::{header::*, StatusCode};

    #[test]
    fn serves_only_media_files() {
        assert!(is_servable(r"C:\Films\A Film.MKV"));
        assert!(is_servable("/home/someone/music/song.flac"));
        assert!(is_servable(r"C:\Users\x\AppData\Local\Temp\glucose_audio_1_2.mkv"));
        assert!(is_servable(r"D:\Recordings\match.M2TS"));
        assert!(is_servable("/home/someone/books/story.m4b"));
        assert!(!is_servable(r"C:\Users\x\.ssh\id_rsa"));
        assert!(!is_servable(r"C:\Users\x\Documents\taxes.pdf"));
        assert!(!is_servable(r"C:\Users\x\notes"));
    }

    #[test]
    #[cfg(windows)]
    fn refuses_windows_devices_but_not_network_shares() {
        assert!(!is_servable(r"C:\x\CON.mp4"));
        assert!(!is_servable(r"C:\x\nul.mkv"));
        assert!(!is_servable(r"C:\x\com1.mp3"));
        assert!(!is_servable(r"\\.\PhysicalDrive0\x.mp4"));
        assert!(!is_servable(r"\\?\GLOBALROOT\Device\x.mp4"));
        assert!(is_servable(r"\\nas\films\Film.mkv"));
        assert!(is_servable(r"C:\x\Conference.mp4"));
        assert!(is_servable(r"C:\x\COM10.mp4"));
    }

    #[test]
    fn typescript_is_not_a_transport_stream() {
        use crate::starts_like_transport_stream as ts;
        assert!(ts(&mut &[0x47u8, 0x40, 0x11, 0x10, 0x00][..]));
        assert!(ts(&mut &[0x00u8, 0x00, 0x00, 0x00, 0x47][..]));
        assert!(!ts(&mut &b"import { x } from './y';"[..]));
        assert!(!ts(&mut &[0x47u8][..]));
    }

    #[test]
    fn reversed_ranges_are_ignored_and_origins_are_checked() {
        assert!(is_reversed_range("bytes=500-100"));
        assert!(!is_reversed_range("bytes=100-500"));
        assert!(!is_reversed_range("bytes=-30"));
        assert!(!is_reversed_range("bytes=10-"));
        assert!(allowed_origin("http://tauri.localhost"));
        assert!(!allowed_origin("https://example.com"));
    }

    #[test]
    fn content_type_comes_from_the_extension() {
        assert_eq!(mime_type(r"C:\a\b.MKV"), "video/x-matroska");
        assert_eq!(mime_type("/a/b.m4b"), "audio/mp4");
        assert_eq!(mime_type("/a/b.m2ts"), "video/mp2t");
    }

    #[test]
    fn parses_ranges() {
        assert_eq!(parse_range("bytes=0-", 100), Some((0, 99)));
        assert_eq!(parse_range("bytes=10-19", 100), Some((10, 19)));
        assert_eq!(parse_range("bytes=90-500", 100), Some((90, 99)));
        assert_eq!(parse_range("bytes=-30", 100), Some((70, 99)));
        assert_eq!(parse_range("bytes=0-9, 20-29", 100), Some((0, 9)));
        assert_eq!(parse_range("bytes=100-", 100), None);
        assert_eq!(parse_range("bytes=0-", 0), None);
        assert_eq!(parse_range("items=0-9", 100), None);
    }

    // Serves `path` for a GET with an optional Range header.
    fn get(path: &std::path::Path, range: Option<&str>) -> tauri::http::Response<Vec<u8>> {
        let uri = format!("asset://localhost/{}", urlencoding::encode(&path.to_string_lossy()));
        let mut request = tauri::http::Request::builder().uri(uri);
        if let Some(range) = range {
            request = request.header(RANGE, range);
        }
        super::serve(&request.body(Vec::new()).unwrap(), "").unwrap()
    }

    fn header<'a>(response: &'a tauri::http::Response<Vec<u8>>, name: HeaderName) -> &'a str {
        response.headers().get(name).and_then(|v| v.to_str().ok()).unwrap_or("")
    }

    #[test]
    fn whole_file_only_when_it_fits_in_one_response() {
        let dir = std::env::temp_dir().join(format!("glucose_media_{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let small = dir.join("small.mp4");
        let large = dir.join("large.mp4");
        std::fs::write(&small, vec![1u8; 1000]).unwrap();
        let large_len = MAX_LEN + 5000;
        std::fs::write(&large, vec![2u8; large_len as usize]).unwrap();

        // No Range, small file: the whole file, 200.
        let r = get(&small, None);
        assert_eq!((r.status(), r.body().len()), (StatusCode::OK, 1000));

        // No Range, large file: 416 with the size, never a partial body.
        let r = get(&large, None);
        assert_eq!(r.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        assert_eq!(header(&r, CONTENT_RANGE), format!("bytes */{large_len}"));
        assert!(r.body().is_empty());

        // A reversed range is ignored, so it is answered like no Range at all.
        let r = get(&large, Some("bytes=500-100"));
        assert_eq!(r.status(), StatusCode::RANGE_NOT_SATISFIABLE);
        let r = get(&small, Some("bytes=500-100"));
        assert_eq!((r.status(), r.body().len()), (StatusCode::OK, 1000));

        // What media elements send: "bytes=0-" gets the first part, 206.
        let r = get(&large, Some("bytes=0-"));
        assert_eq!(r.status(), StatusCode::PARTIAL_CONTENT);
        assert_eq!(r.body().len() as u64, MAX_LEN);
        assert_eq!(header(&r, CONTENT_RANGE), format!("bytes 0-{}/{large_len}", MAX_LEN - 1));
        let r = get(&large, Some(&format!("bytes={}-", large_len - 10)));
        assert_eq!((r.status(), r.body().len()), (StatusCode::PARTIAL_CONTENT, 10));

        // A range past the end is still unsatisfiable.
        let r = get(&large, Some(&format!("bytes={large_len}-")));
        assert_eq!(r.status(), StatusCode::RANGE_NOT_SATISFIABLE);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
