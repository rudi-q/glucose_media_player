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

export function getFadeDurationMs(raw?: string | null): number {
  return FADE_MODE_MS[getFadeMode(raw)];
}
