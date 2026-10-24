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
// Sent on window when the setting changes, so an open player picks it up.
export const LIVE_SUBTITLE_WAIT_EVENT = "glucose-live-subtitle-wait-changed";

export function getLiveSubtitleWait(raw?: string | null): LiveSubtitleWait {
  return raw === "pause" ? "pause" : "play";
}

// Language live subtitles are translated into; null shows the original. Every target
// except English translates English speech; English translates speech in the languages
// src-tauri/src/translate.rs has models for.
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
  // Own keys only: `in` would also accept "constructor" or "toString".
  return raw && Object.prototype.hasOwnProperty.call(LIVE_TRANSLATE_NAMES, raw) ? (raw as LiveTranslate) : null;
}

// Dual subtitles: with translation on, also show the original line, smaller, under the
// translation. Settings dispatches the event on window so an open player updates at once.
export const LIVE_DUAL_SUBTITLES_KEY = "glucose_live_dual_subtitles";
export const LIVE_DUAL_SUBTITLES_EVENT = "glucose-live-dual-subtitles-changed";

export function getLiveDualSubtitles(raw?: string | null): boolean {
  return raw === "true";
}

// Where subtitles sit: fixed where the subtitle style's Position slider puts them, or
// "smart": at the bottom, moved to the top to keep off on-screen text, faces and bright
// areas. Settings dispatches the event so an open player updates.
export type SubtitlePosition = "fixed" | "smart";
export const SUBTITLE_POSITION_KEY = "glucose_subtitle_position";
export const SUBTITLE_POSITION_EVENT = "glucose-subtitle-position-changed";

export function getSubtitlePosition(raw?: string | null): SubtitlePosition {
  return raw === "smart" ? "smart" : "fixed";
}

export function getFadeDurationMs(raw?: string | null): number {
  return FADE_MODE_MS[getFadeMode(raw)];
}
