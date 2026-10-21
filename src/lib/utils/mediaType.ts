export const AUDIO_EXTENSIONS = new Set([
  "mp3",
  "flac",
  "wav",
  "aac",
  "ogg",
  "opus",
  "m4a",
  "aiff",
  "wma",
  "m4b",
  "mka",
  "wv",
  "ape",
]);

// Everything the player opens; keep in sync with is_playable_extension in src-tauri/src/lib.rs.
const VIDEO_EXTENSIONS = new Set([
  "mp4",
  "mkv",
  "avi",
  "mov",
  "wmv",
  "flv",
  "webm",
  "m4v",
  "mpg",
  "mpeg",
  "ogv",
  "ts",
  "m2ts",
  "mts",
  "3gp",
  "3g2",
]);

function extension(path: string): string {
  return path.split(".").pop()?.toLowerCase() ?? "";
}

export function isAudio(path: string | null | undefined): boolean {
  if (!path) return false;
  return AUDIO_EXTENSIONS.has(extension(path));
}

export function isPlayable(path: string): boolean {
  const ext = extension(path);
  return VIDEO_EXTENSIONS.has(ext) || AUDIO_EXTENSIONS.has(ext);
}
