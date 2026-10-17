// Smart subtitle position: when a cue starts, look at a tiny copy of the current frame and
// show the cue at the top instead of the bottom if the bottom of the picture is bright or
// busy (on-screen text, a news ticker, a bright floor) and the top is clearly calmer.

export type SubtitlePlacement = "bottom" | "top";

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
  // 0 (dark and flat) to 1 (bright, detailed, or with text in it).
  busy: number;
  brightness: number;
  edges: number;
  brightShare: number;
  // Share of the densest row made of sharp light/dark steps, as in lines of text.
  textRow: number;
}

// A sharp step between neighbouring pixels, in luma levels.
const SHARP_STEP = 50;
// A row this full of sharp steps reads as text: a burned-in subtitle scored about 0.14
// and a news ticker 0.38 in testing, while a busy test pattern stayed near 0.02.
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
  let textRow = 0;
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
    textRow = Math.max(textRow, sharp / width);
  }
  if (n === 0) return { busy: 0, brightness: 0, edges: 0, brightShare: 0, textRow: 0 };
  const brightness = sum / n / 255;
  // Mean neighbour difference; around 15 levels is already a very detailed area.
  const edges = Math.min(1, edgeSum / n / 15);
  const brightShare = bright / n;
  const picture = 0.5 * brightness + 0.3 * edges + 0.2 * brightShare;
  // Text makes a band busy on its own, however dark the picture around it is.
  const busy = Math.max(picture, Math.min(1, textRow / TEXT_ROW_FULL));
  return { busy, brightness, edges, brightShare, textRow };
}

// Picks where the next cue goes. Prefers the bottom, and only changes when the difference
// is clear, so subtitles do not jump between cues over small changes in the picture.
export function choosePlacement(
  current: SubtitlePlacement,
  bottom: BandScore,
  top: BandScore,
): SubtitlePlacement {
  if (current === "bottom") {
    return bottom.busy > MOVE_UP_BUSY && top.busy < bottom.busy - MOVE_UP_MARGIN ? "top" : "bottom";
  }
  return bottom.busy < MOVE_DOWN_BUSY || top.busy > bottom.busy - MOVE_DOWN_MARGIN ? "bottom" : "top";
}

let canvas: HTMLCanvasElement | null = null;
let ctx: CanvasRenderingContext2D | null = null;

// Scores the bottom and top of the frame the video is showing now, or null when there
// is no frame to look at yet (or the browser refuses to read it).
export async function scoreVideoFrame(
  video: HTMLVideoElement,
): Promise<{ bottom: BandScore; top: BandScore } | null> {
  if (video.readyState < 2 || !video.videoWidth || !video.videoHeight) return null;
  try {
    // The browser scales the frame down off the window thread; drawing the full-size
    // frame straight onto a canvas took 25-50 ms per cue in testing.
    const bitmap = await createImageBitmap(video, {
      resizeWidth: SAMPLE_W,
      resizeHeight: SAMPLE_H,
      resizeQuality: "low",
    });
    if (!canvas) {
      canvas = document.createElement("canvas");
      canvas.width = SAMPLE_W;
      canvas.height = SAMPLE_H;
      ctx = canvas.getContext("2d", { willReadFrequently: true });
    }
    if (!ctx) {
      bitmap.close();
      return null;
    }
    ctx.drawImage(bitmap, 0, 0);
    bitmap.close();
    const pixels = ctx.getImageData(0, 0, SAMPLE_W, SAMPLE_H).data;
    return {
      bottom: scoreBand(pixels, SAMPLE_W, SAMPLE_H, BOTTOM_BAND),
      top: scoreBand(pixels, SAMPLE_W, SAMPLE_H, TOP_BAND),
    };
  } catch (err) {
    if (import.meta.env.DEV) console.warn("Smart subtitle position: could not read frame", err);
    return null;
  }
}
