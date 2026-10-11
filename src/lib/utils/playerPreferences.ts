export type DefaultPlayMode = "cinematic" | "fullscreen" | "pip";
export type EndBehavior = "nothing" | "loop" | "next";
export type FadeMode = "off" | "short" | "default" | "long";

export const FADE_MODE_MS: Record<FadeMode, number> = {
  off: 0,
  short: 300,
  default: 800,
  long: 1500,
};

export function getDefaultPlayMode(raw?: string | null): DefaultPlayMode {
  return raw === "fullscreen" || raw === "pip" ? raw : "cinematic";
}

export function getEndBehavior(raw?: string | null): EndBehavior {
  return raw === "loop" || raw === "next" ? raw : "nothing";
}

export function getFadeMode(raw?: string | null): FadeMode {
  return raw === "off" || raw === "short" || raw === "long" ? raw : "default";
}

// Whether the player starts live subtitles on its own when a video has none.
export const AUTO_LIVE_SUBTITLES_KEY = "glucose_auto_live_subtitles";

export function getAutoLiveSubtitles(raw?: string | null): boolean {
  return raw === "true";
}

// What the player does when playback reaches a point live subtitles are not ready for.
export type LiveSubtitleWait = "play" | "pause";
export const LIVE_SUBTITLE_WAIT_KEY = "glucose_live_subtitle_wait";

export function getLiveSubtitleWait(raw?: string | null): LiveSubtitleWait {
  return raw === "pause" ? "pause" : "play";
}

// Language live subtitles are translated into; null shows the original. Every target
// except English translates English speech; English translates German, French, Spanish
// or Hindi speech. Must match the models in src-tauri/src/translate.rs.
export type LiveTranslate = "de" | "fr" | "es" | "ru" | "tr" | "it" | "nl" | "en";
export const LIVE_TRANSLATE_KEY = "glucose_live_translate";
export const LIVE_TRANSLATE_NAMES: Record<LiveTranslate, string> = {
  de: "German",
  fr: "French",
  es: "Spanish",
  ru: "Russian",
  tr: "Turkish",
  it: "Italian",
  nl: "Dutch",
  en: "English",
};

export function getLiveTranslate(raw?: string | null): LiveTranslate | null {
  return raw && raw in LIVE_TRANSLATE_NAMES ? (raw as LiveTranslate) : null;
}

export function getFadeDurationMs(raw?: string | null): number {
  return FADE_MODE_MS[getFadeMode(raw)];
}
