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

pub(crate) fn handle(request: Request<Vec<u8>>, responder: tauri::UriSchemeResponder) {
    tauri::async_runtime::spawn_blocking(move || {
        let origin = request
            .headers()
            .get(ORIGIN)
            .and_then(|o| o.to_str().ok())
            .unwrap_or("*")
            .to_string();
        let started = std::time::Instant::now();
        let response = serve(&request, &origin).unwrap_or_else(|e| {
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .header(ACCESS_CONTROL_ALLOW_ORIGIN, &origin)
                .body(e.to_string().into_bytes())
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

fn serve(request: &Request<Vec<u8>>, origin: &str) -> Served {
    let path = request_path(request);
    let resp = Response::builder().header(ACCESS_CONTROL_ALLOW_ORIGIN, origin);
    if tauri::path::SafePathBuf::new(path.clone().into()).is_err() {
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
    let len = file.metadata()?.len();

    let mut magic = Vec::new();
    (&mut file).take(len.min(8192)).read_to_end(&mut magic)?;
    let mime = tauri::utils::mime_type::MimeType::parse(&magic, &path);
    let resp = resp
        .header(CONTENT_TYPE, mime)
        .header(ACCEPT_RANGES, "bytes")
        .header(ACCESS_CONTROL_EXPOSE_HEADERS, "content-range");

    if request.method() == Method::HEAD {
        return Ok(resp.header(CONTENT_LENGTH, len).body(Vec::new())?);
    }

    let range = request.headers().get(RANGE).and_then(|r| r.to_str().ok());
    if range.is_none() && len <= MAX_LEN {
        file.rewind()?;
        let mut buf = Vec::with_capacity(len as usize);
        file.read_to_end(&mut buf)?;
        return Ok(resp.header(CONTENT_LENGTH, len).body(buf)?);
    }
    // A large file without a range is sent in parts too, never read whole into memory.
    let Some((start, end)) = range.map_or(len.checked_sub(1).map(|e| (0, e)), |r| parse_range(r, len))
    else {
        return Ok(Response::builder()
            .status(StatusCode::RANGE_NOT_SATISFIABLE)
            .header(ACCESS_CONTROL_ALLOW_ORIGIN, origin)
            .header(CONTENT_RANGE, format!("bytes */{len}"))
            .body(Vec::new())?);
    };
    let end = end.min(start + MAX_LEN - 1);
    let nbytes = end + 1 - start;
    let mut buf = Vec::with_capacity(nbytes as usize);
    file.seek(SeekFrom::Start(start))?;
    file.take(nbytes).read_to_end(&mut buf)?;
    Ok(resp
        .status(StatusCode::PARTIAL_CONTENT)
        .header(CONTENT_RANGE, format!("bytes {start}-{end}/{len}"))
        .header(CONTENT_LENGTH, nbytes)
        .body(buf)?)
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
    use super::parse_range;

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
}
