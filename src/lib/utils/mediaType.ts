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
]);

export function isAudio(path: string | null | undefined): boolean {
  if (!path) return false;
  return AUDIO_EXTENSIONS.has(path.split(".").pop()?.toLowerCase() ?? "");
}
