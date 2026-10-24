// Audio formats, which open in the audio player; keep in sync with AUDIO_EXTENSIONS and
// EXTRA_PLAYABLE_EXTENSIONS in src-tauri/src/lib.rs.
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

export function isAudio(path: string | null | undefined): boolean {
  if (!path) return false;
  return AUDIO_EXTENSIONS.has(path.split(".").pop()?.toLowerCase() ?? "");
}
