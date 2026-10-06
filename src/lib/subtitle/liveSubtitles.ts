import type { VttCue } from '$lib/subtitle/vttParser';

// Event payloads emitted by src-tauri/src/live_subtitles.rs.
export interface LiveSubtitleChunk {
  session_id: number;
  covered_start: number;
  covered_end: number;
  cues: VttCue[];
}

export interface LiveSubtitleStatus {
  session_id: number;
  state: 'loading' | 'running' | 'paused' | 'done' | 'error';
  message: string;
}

export interface LiveSessionInfo {
  session_id: number;
  model: string;
}

// A processed span of the timeline, [start, end) in seconds.
export type CoveredRange = [number, number];

// Identifies a live cache file: one per video, audio track and language.
export interface LiveCacheKey {
  videoPath: string;
  audioStreamIndex: number | null;
  language: string;
}

// Returned by load_live_cache in src-tauri/src/live_cache.rs.
export interface LiveCacheData {
  model: string;
  ranges: CoveredRange[];
  cues: VttCue[];
}

// Gaps up to this size between ranges are treated as contiguous, so float rounding
// at chunk boundaries does not leave hairline holes.
const MERGE_EPSILON = 0.05;

export function addRange(ranges: CoveredRange[], start: number, end: number): CoveredRange[] {
  const sorted = [...ranges, [start, end] as CoveredRange].sort((a, b) => a[0] - b[0]);
  const merged: CoveredRange[] = [];
  for (const [s, e] of sorted) {
    const last = merged[merged.length - 1];
    if (last && s <= last[1] + MERGE_EPSILON) {
      last[1] = Math.max(last[1], e);
    } else {
      merged.push([s, e]);
    }
  }
  return merged;
}

export function findRange(ranges: CoveredRange[], time: number): CoveredRange | undefined {
  return ranges.find(([s, e]) => time >= s && time < e);
}

// Replaces any cues inside [start, end) with the new ones, keeping the list sorted.
export function mergeCues(cues: VttCue[], incoming: VttCue[], start: number, end: number): VttCue[] {
  const kept = cues.filter((c) => c.start < start || c.start >= end);
  return [...kept, ...incoming].sort((a, b) => a.start - b.start);
}
