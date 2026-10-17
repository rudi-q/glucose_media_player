// Turns the hardware check (src-tauri/src/hardware_check.rs) into plain advice per heavy
// feature, so Glucose does not promise more than the machine can deliver.

export interface HardwareReport {
  cpu_name: string | null;
  threads: number;
  ram_gb: number | null;
  gpu_name: string | null;
  live_model: string | null;
  // How many times faster than real time Whisper transcribed 20 s of speech.
  live_speed_x: number | null;
  face_ms: number | null;
  problems: string[];
}

export type Verdict = "good" | "slow" | "poor" | "unknown";
export type FeatureId = "live" | "translation" | "smart" | "generation";

export interface FeatureAdvice {
  id: FeatureId;
  name: string;
  verdict: Verdict;
  // One sentence on why, in plain words.
  reason: string;
}

export const VERDICT_LABELS: Record<Verdict, string> = {
  good: "Works well",
  slow: "May be slow",
  poor: "Not recommended",
  unknown: "Not tested",
};

// Live subtitles need Whisper to stay ahead of playback. At 3x it keeps a comfortable
// lead; between 1.5x and 3x it can fall behind in dense dialogue; below 1.5x it cannot
// keep up once the start-up lead is used.
const LIVE_GOOD_X = 3;
const LIVE_SLOW_X = 1.5;
// Smart placement holds a new subtitle back while the frame is checked, for at most
// 200 ms (SubtitleOverlay.svelte).
const FACE_GOOD_MS = 60;
const FACE_SLOW_MS = 150;
// The large model needs about 2 GB of memory on top of the app and the video.
const GENERATION_GOOD_GB = 8;
const GENERATION_SLOW_GB = 4;

function speed(x: number): string {
  return `${x.toFixed(1)}x`;
}

export function adviseFeatures(r: HardwareReport): FeatureAdvice[] {
  const x = r.live_speed_x;
  const live: FeatureAdvice =
    x === null
      ? {
          id: "live",
          name: "Live subtitles",
          verdict: "unknown",
          reason: "Download a speech model in AI Settings, then check again to test this.",
        }
      : x >= LIVE_GOOD_X
        ? {
            id: "live",
            name: "Live subtitles",
            verdict: "good",
            reason: `Speech recognition runs ${speed(x)} faster than playback, enough to stay well ahead.`,
          }
        : x >= LIVE_SLOW_X
          ? {
              id: "live",
              name: "Live subtitles",
              verdict: "slow",
              reason: `Speech recognition runs ${speed(x)} faster than playback, so subtitles can fall behind in busy scenes. "Pause until ready" helps.`,
            }
          : {
              id: "live",
              name: "Live subtitles",
              verdict: "poor",
              reason: `Speech recognition runs at ${speed(x)} playback speed, too slow to keep up with the video.`,
            };

  // Translation itself is tiny (about 0.1 s per 20 s of audio); it lives or dies with
  // live subtitles.
  const translation: FeatureAdvice = {
    id: "translation",
    name: "Live translation",
    verdict: live.verdict,
    reason:
      live.verdict === "unknown"
        ? "Depends on live subtitles, which have not been tested yet."
        : live.verdict === "good"
          ? "Translating adds only a fraction of a second per minute of video."
          : "Translating is quick, but it needs live subtitles, which may struggle here.",
  };

  const ms = r.face_ms;
  const smart: FeatureAdvice =
    ms === null
      ? {
          id: "smart",
          name: "Smart subtitle position",
          verdict: "unknown",
          reason: "Face detection could not run. Smart position still avoids on-screen text and bright areas.",
        }
      : ms <= FACE_GOOD_MS
        ? {
            id: "smart",
            name: "Smart subtitle position",
            verdict: "good",
            reason: `Checking a frame for faces takes ${Math.round(ms)} ms, too short to notice.`,
          }
        : ms <= FACE_SLOW_MS
          ? {
              id: "smart",
              name: "Smart subtitle position",
              verdict: "slow",
              reason: `Checking a frame for faces takes ${Math.round(ms)} ms, so subtitles can appear a moment late.`,
            }
          : {
              id: "smart",
              name: "Smart subtitle position",
              verdict: "poor",
              reason: `Checking a frame for faces takes ${Math.round(ms)} ms, often too long to wait for, so faces would mostly be missed.`,
            };

  // Generating a whole subtitle file is not real-time, so slowness only means waiting
  // longer; memory is what can make the large model fail.
  const gb = r.ram_gb;
  const generation: FeatureAdvice =
    gb === null
      ? {
          id: "generation",
          name: "AI subtitles (Large model)",
          verdict: "unknown",
          reason: "The amount of memory could not be read.",
        }
      : gb >= GENERATION_GOOD_GB && (x === null || x >= LIVE_GOOD_X)
        ? {
            id: "generation",
            name: "AI subtitles (Large model)",
            verdict: "good",
            reason: `This PC has ${Math.round(gb)} GB of memory, plenty for the large model.`,
          }
        : gb >= GENERATION_SLOW_GB
          ? {
              id: "generation",
              name: "AI subtitles (Large model)",
              verdict: "slow",
              reason: "It will work, but generating a film's subtitles can take a long time. Tiny or Small are faster.",
            }
          : {
              id: "generation",
              name: "AI subtitles (Large model)",
              verdict: "poor",
              reason: `With ${Math.round(gb)} GB of memory the large model may not fit. Use Tiny or Small instead.`,
            };

  return [live, translation, smart, generation];
}

// What the last check found, kept between launches.
export const HARDWARE_CHECK_KEY = "glucose_hardware_check";
export const HARDWARE_CHECK_VERSION = 1;

export interface StoredHardwareCheck {
  version: number;
  checkedAt: string;
  report: HardwareReport;
  // Features the first automatic check turned off, to explain it in the Features tab.
  turnedOff: FeatureId[];
}

export function loadHardwareCheck(raw: string | null): StoredHardwareCheck | null {
  if (!raw) return null;
  try {
    const parsed = JSON.parse(raw) as StoredHardwareCheck;
    return parsed?.version === HARDWARE_CHECK_VERSION && parsed.report ? parsed : null;
  } catch {
    return null;
  }
}
