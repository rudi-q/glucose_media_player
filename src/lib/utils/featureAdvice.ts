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

// Live subtitles keep up as long as Whisper is faster than playback (above 1x); anything
// beyond that builds a lead. Just above 1x they keep up but take long to catch up after a
// seek (20 s of audio per chunk, so about 16 s at 1.2x). These limits are deliberately
// generous: the test runs whatever else the PC is doing at the time, and a 1.2x result
// from a busy machine was perfectly usable in practice, translation included.
const LIVE_GOOD_X = 1.5;
const LIVE_SLOW_X = 1.0;
// Smart placement holds a new subtitle back while the frame is checked, for at most
// 200 ms (SubtitleOverlay.svelte); under about 100 ms the delay is not noticeable.
const FACE_GOOD_MS = 100;
const FACE_SLOW_MS = 200;
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
            reason: `Speech recognition runs ${speed(x)} faster than playback, enough to stay ahead.`,
          }
        : x >= LIVE_SLOW_X
          ? {
              id: "live",
              name: "Live subtitles",
              verdict: "slow",
              reason: `Speech recognition runs ${speed(x)} faster than playback: it keeps up, but after a seek the first subtitles take a while. "Pause until ready" helps.`,
            }
          : {
              id: "live",
              name: "Live subtitles",
              verdict: "poor",
              reason: `Speech recognition runs at ${speed(x)} playback speed, slower than the video, so subtitles would keep falling behind.`,
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
          : "Translating is quick; how well it works depends on live subtitles above.",
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
              reason: `Checking a frame for faces takes ${Math.round(ms)} ms, longer than subtitles wait, so faces would often be missed.`,
            };

  // Generating a whole subtitle file is not real-time, so a slower PC only means waiting
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
      : gb >= GENERATION_GOOD_GB
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
              reason: `With ${Math.round(gb)} GB of memory the large model fits, but leaves little room for other programs. Tiny or Small are lighter.`,
            }
          : {
              id: "generation",
              name: "AI subtitles (Large model)",
              verdict: "poor",
              reason: `With ${Math.round(gb)} GB of memory the large model may not fit. Use Tiny or Small instead.`,
            };

  return [live, translation, smart, generation];
}

// Hardware names without trademark marks and clock speeds, for display:
// "13th Gen Intel(R) Core(TM) i9-13900H" becomes "13th Gen Intel Core i9-13900H".
export function displayHardwareName(name: string): string {
  return name
    .replace(/\((?:R|TM|C)\)/gi, "")
    .replace(/\s+CPU\b/i, "")
    .replace(/\s*@\s*[\d.]+\s*GHz/i, "")
    .replace(/\s+/g, " ")
    .trim();
}

// A short warning to show next to a feature's own setting, or null when it runs fine
// (or was not tested).
export function featureWarning(verdict: Verdict | undefined): string | null {
  if (verdict === "poor") return "Not recommended on this PC. See Settings > Features.";
  if (verdict === "slow") return "May be slow on this PC. See Settings > Features.";
  return null;
}

// Verdict per feature from a stored check; empty when the PC was not checked yet.
export function featureVerdicts(check: StoredHardwareCheck | null): Partial<Record<FeatureId, Verdict>> {
  return check ? Object.fromEntries(adviseFeatures(check.report).map((a) => [a.id, a.verdict])) : {};
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
