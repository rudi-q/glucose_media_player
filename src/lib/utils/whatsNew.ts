// "What's new" screen: shown once, in the library, the first time a new feature version
// (major or minor, not a patch) opens on a machine that had an older Glucose. Fresh
// installs see onboarding instead. Highlights are written per version here; the summary
// line is the first line of RELEASE_NOTES.md, which is also the release's GitHub text.

import releaseNotes from "../../../RELEASE_NOTES.md?raw";

export const LAST_SEEN_VERSION_KEY = "glucose_last_seen_version";

// What a highlight's button does; handled by the layout.
export type WhatsNewAction = "live" | "translation" | "smart" | "features";

export interface WhatsNewHighlight {
  icon: "captions" | "languages" | "scan-face" | "activity" | "mouse-pointer" | "sparkles";
  title: string;
  text: string;
  // A button for the highlight, for features with a setting to open or turn on. Features
  // used from the player have none: the text says where to find them.
  action?: WhatsNewAction;
  actionLabel?: string;
}

// Keyed by "major.minor". A feature version without an entry shows no screen.
const HIGHLIGHTS: Record<string, WhatsNewHighlight[]> = {
  "4.1": [
    {
      icon: "captions",
      title: "Two subtitles at once",
      text: "Show a second, smaller line under the first: live subtitles with a subtitle file, two languages, or a translation with its original. Pick both lines in the subtitles menu.",
    },
    {
      icon: "mouse-pointer",
      title: "Fix a line where you see it",
      text: "Rest the mouse on a subtitle line to swap it, shift its timing or turn it off.",
    },
    {
      icon: "languages",
      title: "Subtitles named by language",
      text: "Subtitle tracks and files show their language, read from the track or the file name.",
    },
    {
      icon: "sparkles",
      title: "Keep watching while subtitles generate",
      text: "Generating subtitles for a whole video no longer covers the player. Progress and Cancel are in the subtitles menu.",
    },
  ],
  "4.0": [
    {
      icon: "captions",
      title: "Live subtitles",
      text: "Subtitles appear as you watch, for videos that have none. Generated on your PC, nothing is uploaded.",
      action: "live",
      actionLabel: "Open settings",
    },
    {
      icon: "languages",
      title: "Live translation",
      text: "Translate live subtitles between English and German, French, Spanish, Italian, Dutch, Russian or Turkish.",
      action: "translation",
      actionLabel: "Get a language",
    },
    {
      icon: "scan-face",
      title: "Smart subtitle position",
      text: "Subtitles move out of the way of faces and on-screen text, then come back down.",
      action: "smart",
      actionLabel: "Turn on Smart",
    },
    {
      icon: "activity",
      title: "Features that fit your PC",
      text: "Glucose checks your PC and tells you which of these will run well on it.",
      action: "features",
      actionLabel: "Check my PC",
    },
  ],
};

// "4.0" from "4.0.2".
function featureVersion(version: string): string {
  return version.split(".").slice(0, 2).join(".");
}

// NaN when either side is not a version: `NaN || x` would otherwise fall through to the
// minor numbers and call a stored "abc" equal to 4.0.
function compareFeatureVersions(a: string, b: string): number {
  const [aMajor, aMinor = 0] = a.split(".").map(Number);
  const [bMajor, bMinor = 0] = b.split(".").map(Number);
  if ([aMajor, aMinor, bMajor, bMinor].some(Number.isNaN)) return NaN;
  return aMajor - bMajor || aMinor - bMinor;
}

export function whatsNewHighlights(version: string): WhatsNewHighlight[] | null {
  return HIGHLIGHTS[featureVersion(version)] ?? null;
}

// Whether to show the screen for `current`, given the version last seen on this machine.
// `existingUser` tells an update from an older Glucose (which never stored a version)
// apart from a fresh install.
export function shouldShowWhatsNew(current: string, lastSeen: string | null, existingUser: boolean): boolean {
  if (!whatsNewHighlights(current)) return false;
  if (lastSeen === null) return existingUser;
  const newer = compareFeatureVersions(featureVersion(current), featureVersion(lastSeen));
  // A stored value that is not a version (NaN) counts as unknown: the screen shows once,
  // and closing it stores the right version.
  return Number.isNaN(newer) || newer > 0;
}

// The release notes' first line: a one-sentence summary of the release. A heading's "#"
// marks are dropped, should the notes start with one.
export const RELEASE_SUMMARY =
  releaseNotes
    .split(/\r?\n/)
    .find((line) => line.trim().length > 0)
    ?.trim()
    .replace(/^#+\s*/, "") ?? "";
