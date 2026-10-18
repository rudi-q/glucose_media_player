// Smart subtitle position: when a cue starts, look at a small copy of the current frame and
// show the cue at the top instead of the bottom when the bottom has on-screen text or a
// face, or is bright or busy while the top is clearly calmer. Follows the order streaming
// services' subtitling guidelines give: bottom by default, never cover on-screen text, and
// when text and a face cannot both be avoided, cover the face rather than the text.

import { invoke } from "@tauri-apps/api/core";

export type SubtitlePlacement = "bottom" | "top";

// A face found in the frame, as fractions of its width and height (from face_detect.rs).
export interface Face {
  x: number;
  y: number;
  w: number;
  h: number;
  score: number;
}

// Size of the frame copy that is scored. Burned-in text (signs, tickers, hard-coded
// subtitles) mostly blurs away below this, and it is still only a few milliseconds' work.
const SAMPLE_W = 256;
const SAMPLE_H = 144;

// Rows (as fractions of the height) where bottom and top subtitles sit.
const BOTTOM_BAND: [number, number] = [0.7, 0.95];
const TOP_BAND: [number, number] = [0.05, 0.3];

// Above this, the bottom counts as busy enough to consider moving.
const MOVE_UP_BUSY = 0.45;
// The top has to be at least this much calmer before subtitles move up.
const MOVE_UP_MARGIN = 0.2;
// Back to the bottom once it calms down below this, or the top stops being calmer.
const MOVE_DOWN_BUSY = 0.35;
const MOVE_DOWN_MARGIN = 0.05;

export interface BandScore {
  // 0 (dark and plain) to 1 (bright, or with text in it).
  busy: number;
  brightness: number;
  edges: number;
  brightShare: number;
  // How much a line of text stands out: the densest run of three rows of sharp light/dark
  // steps, minus the band's typical row. Text is a thin strip of edges; textures such as
  // carved wood or fabric have edges in every row and score low.
  textRow: number;
  // Faces a subtitle in this band would cover.
  faces: number;
}

// A sharp step between neighbouring pixels, in luma levels.
const SHARP_STEP = 50;
// In testing, text scored 0.09 to 0.33 (subtitles, a title, a news ticker), while textures
// in dark film scenes (carved wood, a sofa) scored 0.01 to 0.04. Very short lines of text
// can score below the cut-off; the face and brightness checks still apply to them.
const TEXT_ROW_PRESENT = 0.06;
const TEXT_ROW_FULL = 0.12;

// How hard white subtitle text would be to read over one band of a frame, from RGBA pixels.
export function scoreBand(
  pixels: Uint8ClampedArray,
  width: number,
  height: number,
  band: [number, number],
): BandScore {
  const y0 = Math.floor(band[0] * height);
  const y1 = Math.min(height, Math.ceil(band[1] * height));
  const luma = (x: number, y: number) => {
    const i = (y * width + x) * 4;
    return 0.299 * pixels[i] + 0.587 * pixels[i + 1] + 0.114 * pixels[i + 2];
  };
  let sum = 0;
  let edgeSum = 0;
  let bright = 0;
  let n = 0;
  const rowSharp: number[] = [];
  for (let y = y0; y < y1; y++) {
    let sharp = 0;
    for (let x = 0; x < width; x++) {
      const l = luma(x, y);
      sum += l;
      if (l > 200) bright++;
      if (x + 1 < width) {
        const step = Math.abs(l - luma(x + 1, y));
        edgeSum += step;
        if (step > SHARP_STEP) sharp++;
      }
      if (y + 1 < y1) edgeSum += Math.abs(l - luma(x, y + 1));
      n++;
    }
    rowSharp.push(sharp / width);
  }
  if (n === 0) return { busy: 0, brightness: 0, edges: 0, brightShare: 0, textRow: 0, faces: 0 };
  let run = 0;
  for (let i = 0; i + 2 < rowSharp.length; i++) {
    run = Math.max(run, Math.min(rowSharp[i], rowSharp[i + 1], rowSharp[i + 2]));
  }
  const typical = [...rowSharp].sort((a, b) => a - b)[Math.floor(rowSharp.length / 2)];
  const textRow = Math.max(0, run - typical);
  const brightness = sum / n / 255;
  // Mean neighbour difference; around 15 levels is already a very detailed area.
  const edges = Math.min(1, edgeSum / n / 15);
  const brightShare = bright / n;
  // Brightness is what makes white subtitles hard to read. Detail counts only a little:
  // film grain and textures make most real scenes "detailed" (0.6 to 0.8 in dark scenes).
  const picture = 0.6 * brightness + 0.1 * edges + 0.3 * brightShare;
  // Text makes a band busy on its own, however dark the picture around it is.
  const busy = Math.max(picture, Math.min(1, textRow / TEXT_ROW_FULL));
  return { busy, brightness, edges, brightShare, textRow, faces: 0 };
}

// Subtitles fill roughly the middle of the width, so faces at the far sides are not covered.
const SUBTITLE_SPAN: [number, number] = [0.1, 0.9];
// A face counts as covered when this much of its height is inside the band.
const FACE_OVERLAP = 0.25;

// How many faces a subtitle in this band would cover.
export function facesInBand(faces: Face[], band: [number, number]): number {
  return faces.filter(f => {
    const overlapY = Math.min(f.y + f.h, band[1]) - Math.max(f.y, band[0]);
    const overlapsX = f.x < SUBTITLE_SPAN[1] && f.x + f.w > SUBTITLE_SPAN[0];
    return overlapsX && overlapY >= FACE_OVERLAP * f.h;
  }).length;
}

// Picks where the next cue goes. On-screen text decides first, then faces: a cue moves off
// either, and never onto one. Otherwise it prefers the bottom and only changes when the
// difference is clear, so subtitles do not jump between cues over small changes in the
// picture.
export function choosePlacement(
  current: SubtitlePlacement,
  bottom: BandScore,
  top: BandScore,
): SubtitlePlacement {
  const bottomText = bottom.textRow >= TEXT_ROW_PRESENT;
  const topText = top.textRow >= TEXT_ROW_PRESENT;
  if (bottomText !== topText) return bottomText ? "top" : "bottom";
  if (bottom.faces > 0 && top.faces === 0) return "top";
  if (top.faces > 0 && bottom.faces === 0) return "bottom";
  if (current === "bottom") {
    return bottom.busy > MOVE_UP_BUSY && top.busy < bottom.busy - MOVE_UP_MARGIN ? "top" : "bottom";
  }
  return bottom.busy < MOVE_DOWN_BUSY || top.busy > bottom.busy - MOVE_DOWN_MARGIN ? "bottom" : "top";
}

// Longest side of the frame copy sent for face detection, keeping the video's shape.
const FACE_SAMPLE_SIDE = 320;

const canvases = new Map<string, CanvasRenderingContext2D | null>();

// RGBA pixels of the current frame scaled to width x height. The browser scales the frame
// down off the window thread; drawing the full-size frame straight onto a canvas took
// 25-50 ms per cue in testing.
async function framePixels(video: HTMLVideoElement, width: number, height: number) {
  const bitmap = await createImageBitmap(video, {
    resizeWidth: width,
    resizeHeight: height,
    resizeQuality: "low",
  });
  try {
    const size = `${width}x${height}`;
    if (!canvases.has(size)) {
      const canvas = document.createElement("canvas");
      canvas.width = width;
      canvas.height = height;
      canvases.set(size, canvas.getContext("2d", { willReadFrequently: true }));
    }
    const ctx = canvases.get(size);
    if (!ctx) return null;
    ctx.drawImage(bitmap, 0, 0);
    return ctx.getImageData(0, 0, width, height).data;
  } finally {
    bitmap.close();
  }
}

// Size of the frame copy sent for face detection.
function faceSampleSize(video: HTMLVideoElement): [number, number] {
  const scale = FACE_SAMPLE_SIDE / Math.max(video.videoWidth, video.videoHeight);
  return [
    Math.max(1, Math.round(video.videoWidth * scale)),
    Math.max(1, Math.round(video.videoHeight * scale)),
  ];
}

// Faces in the current frame. Detection runs in the app's Rust side; no faces when it fails.
async function findFaces(video: HTMLVideoElement): Promise<Face[]> {
  const [width, height] = faceSampleSize(video);
  const pixels = await framePixels(video, width, height);
  if (!pixels) return [];
  try {
    return await invoke<Face[]>("detect_faces", new Uint8Array(pixels.buffer), {
      headers: { "x-width": String(width), "x-height": String(height) },
    });
  } catch (err) {
    if (import.meta.env.DEV) console.warn("Smart subtitle position: face detection failed", err);
    return [];
  }
}

// Prepares face detection for this video's frame size ahead of the first subtitle.
export function preloadFaceDetection(video: HTMLVideoElement) {
  if (!video.videoWidth || !video.videoHeight) return;
  const [width, height] = faceSampleSize(video);
  invoke("preload_face_detection", { width, height }).catch(err => {
    if (import.meta.env.DEV) console.warn("Smart subtitle position: could not load face detection", err);
  });
}

export interface FrameScores {
  bottom: BandScore;
  top: BandScore;
  faces: Face[];
}

// Scores the bottom and top of the frame the video is showing now, or null when there
// is no frame to look at yet (or the browser refuses to read it).
export async function scoreVideoFrame(video: HTMLVideoElement): Promise<FrameScores | null> {
  if (video.readyState < 2 || !video.videoWidth || !video.videoHeight) return null;
  try {
    const [pixels, faces] = await Promise.all([
      framePixels(video, SAMPLE_W, SAMPLE_H),
      findFaces(video),
    ]);
    if (!pixels) return null;
    const bottom = scoreBand(pixels, SAMPLE_W, SAMPLE_H, BOTTOM_BAND);
    const top = scoreBand(pixels, SAMPLE_W, SAMPLE_H, TOP_BAND);
    bottom.faces = facesInBand(faces, BOTTOM_BAND);
    top.faces = facesInBand(faces, TOP_BAND);
    return { bottom, top, faces };
  } catch (err) {
    if (import.meta.env.DEV) console.warn("Smart subtitle position: could not read frame", err);
    return null;
  }
}
