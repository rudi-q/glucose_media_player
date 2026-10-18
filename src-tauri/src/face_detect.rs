// Face detection for smart subtitle placement: finds faces in a small copy of the current
// video frame so subtitles can avoid covering them. Uses the YuNet model (MIT, see
// models/face_detection/LICENSE), built into the app, run with tract (pure Rust).

use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};

use serde::Serialize;
use tauri::ipc::{InvokeBody, Request};
use tract_onnx::prelude::*;

const MODEL: &[u8] = include_bytes!("../models/face_detection/face_detection_yunet_2023mar.onnx");

// Frames are scaled so their longest side is SIZE, then padded with black up to a multiple
// of STRIDE (the model's coarsest grid), so a 16:9 frame runs at 320x192.
const SIZE: usize = 320;
const STRIDE: usize = 32;
// Largest frame side accepted; the page sends frames already scaled to fit SIZE.
const MAX_SIDE: usize = 1024;
const MIN_SCORE: f32 = 0.6;
const NMS_IOU: f32 = 0.3;

type Model = TypedRunnableModel<TypedModel>;

// The parsed model, and one optimised copy per input size (usually one per video shape).
// Each size has its own once-cell, so concurrent first calls for a size wait for a single
// preparation instead of each optimising the model.
type ModelCell = Arc<OnceLock<Result<Arc<Model>, String>>>;
static PARSED: OnceLock<Result<InferenceModel, String>> = OnceLock::new();
static MODELS: OnceLock<Mutex<HashMap<(usize, usize), ModelCell>>> = OnceLock::new();

fn parsed() -> Result<&'static InferenceModel, String> {
    PARSED
        .get_or_init(|| parse().map_err(|e| format!("Could not load the face detection model: {e}")))
        .as_ref()
        .map_err(Clone::clone)
}

fn parse() -> TractResult<InferenceModel> {
    let onnx = tract_onnx::onnx().with_ignore_output_shapes(true);
    let mut proto = onnx.proto_model_for_read(&mut std::io::Cursor::new(MODEL))?;
    // The export records 640x640 shapes for every intermediate value; drop them so the
    // model can run at other sizes.
    if let Some(graph) = proto.graph.as_mut() {
        graph.value_info.clear();
    }
    onnx.model_for_proto_model(&proto)
}

fn model(width: usize, height: usize) -> Result<Arc<Model>, String> {
    let cell = MODELS
        .get_or_init(Default::default)
        .lock()
        .map_err(|e| e.to_string())?
        .entry((width, height))
        .or_default()
        .clone();
    cell.get_or_init(|| {
        parsed()?
            .clone()
            .with_input_fact(0, f32::fact([1, 3, height, width]).into())
            .and_then(|m| m.into_optimized())
            .and_then(|m| m.into_runnable())
            .map(Arc::new)
            .map_err(|e| format!("Could not prepare face detection: {e}"))
    })
    .clone()
}

// Input size for a frame: longest side SIZE, both sides rounded up to a multiple of STRIDE.
fn input_size(width: usize, height: usize) -> (f32, usize, usize, usize, usize) {
    let scale = SIZE as f32 / width.max(height) as f32;
    let sw = ((width as f32 * scale) as usize).clamp(1, SIZE);
    let sh = ((height as f32 * scale) as usize).clamp(1, SIZE);
    (scale, sw, sh, sw.div_ceil(STRIDE) * STRIDE, sh.div_ceil(STRIDE) * STRIDE)
}

/// A face, as fractions of the frame's width and height.
#[derive(Debug, Clone, Copy, Serialize, PartialEq)]
pub struct Face {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
    pub score: f32,
}

/// Finds faces in an RGBA frame. The body is the raw pixels; the `x-width` and `x-height`
/// headers give its size. Loads the model on first use.
#[tauri::command]
pub async fn detect_faces(request: Request<'_>) -> Result<Vec<Face>, String> {
    let header = |name: &str| -> Result<usize, String> {
        request
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .ok_or_else(|| format!("Missing or invalid {name} header"))
    };
    let (width, height) = (header("x-width")?, header("x-height")?);
    let InvokeBody::Raw(pixels) = request.body() else {
        return Err("Expected raw frame pixels".into());
    };
    if width == 0 || height == 0 || width > MAX_SIDE || height > MAX_SIDE || pixels.len() != width * height * 4 {
        return Err("Frame size does not match its pixels".into());
    }
    let pixels = pixels.clone();
    tauri::async_runtime::spawn_blocking(move || detect(&pixels, width, height))
        .await
        .map_err(|e| e.to_string())?
}

/// Prepares the model for a video's frame size ahead of the first subtitle, so that one is
/// not held back.
#[tauri::command]
pub async fn preload_face_detection(width: usize, height: usize) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Ok(());
    }
    tauri::async_runtime::spawn_blocking(move || {
        let (_, _, _, iw, ih) = input_size(width, height);
        model(iw, ih).map(|_| ())
    })
    .await
    .map_err(|e| e.to_string())?
}

/// For the hardware check: milliseconds one detection takes on a 16:9 frame, the median
/// of five runs after the model is prepared.
pub(crate) fn time_detection() -> Result<f64, String> {
    let (w, h) = (320, 180);
    let frame = vec![40u8; w * h * 4];
    detect(&frame, w, h)?;
    let mut times = Vec::new();
    for _ in 0..5 {
        let started = std::time::Instant::now();
        detect(&frame, w, h)?;
        times.push(started.elapsed().as_secs_f64() * 1000.0);
    }
    times.sort_by(f64::total_cmp);
    Ok(times[times.len() / 2])
}

fn detect(rgba: &[u8], width: usize, height: usize) -> Result<Vec<Face>, String> {
    // Scale (nearest pixel; the page already sends a small frame) and pad.
    let (scale, sw, sh, iw, ih) = input_size(width, height);
    let model = model(iw, ih)?;
    // BGR, 0-255, NCHW, as the model expects.
    let plane = iw * ih;
    let mut input = vec![0f32; 3 * plane];
    for y in 0..sh {
        let src_y = ((y as f32 / scale) as usize).min(height - 1);
        for x in 0..sw {
            let src_x = ((x as f32 / scale) as usize).min(width - 1);
            let i = (src_y * width + src_x) * 4;
            let o = y * iw + x;
            input[o] = rgba[i + 2] as f32;
            input[plane + o] = rgba[i + 1] as f32;
            input[2 * plane + o] = rgba[i] as f32;
        }
    }
    let input = Tensor::from_shape(&[1, 3, ih, iw], &input).map_err(|e| e.to_string())?;
    let outputs = model
        .run(tvec!(input.into()))
        .map_err(|e| format!("Face detection failed: {e}"))?;
    let mut faces = decode(&outputs, iw, scale).map_err(|e| format!("Face detection failed: {e}"))?;
    for f in &mut faces {
        f.x /= width as f32;
        f.w /= width as f32;
        f.y /= height as f32;
        f.h /= height as f32;
    }
    Ok(faces)
}

// Turns the model's per-grid-cell outputs into face boxes in frame pixels.
// Outputs: cls_8, cls_16, cls_32, obj_8, obj_16, obj_32, bbox_8, bbox_16, bbox_32, kps_*.
fn decode(outputs: &[TValue], input_width: usize, scale: f32) -> TractResult<Vec<Face>> {
    let mut faces = Vec::new();
    for (i, stride) in [8usize, 16, 32].into_iter().enumerate() {
        let cls = outputs[i].as_slice::<f32>()?;
        let obj = outputs[3 + i].as_slice::<f32>()?;
        let bbox = outputs[6 + i].as_slice::<f32>()?;
        let cols = input_width / stride;
        let s = stride as f32;
        for idx in 0..cls.len().min(obj.len()).min(bbox.len() / 4) {
            let score = (cls[idx].clamp(0.0, 1.0) * obj[idx].clamp(0.0, 1.0)).sqrt();
            if score < MIN_SCORE {
                continue;
            }
            let (row, col) = ((idx / cols) as f32, (idx % cols) as f32);
            let cx = (col + bbox[idx * 4]) * s;
            let cy = (row + bbox[idx * 4 + 1]) * s;
            let w = bbox[idx * 4 + 2].exp() * s;
            let h = bbox[idx * 4 + 3].exp() * s;
            faces.push(Face {
                x: (cx - w / 2.0) / scale,
                y: (cy - h / 2.0) / scale,
                w: w / scale,
                h: h / scale,
                score,
            });
        }
    }
    Ok(non_max_suppression(faces))
}

// Keeps the best box where several overlap the same face.
fn non_max_suppression(mut faces: Vec<Face>) -> Vec<Face> {
    faces.sort_by(|a, b| b.score.total_cmp(&a.score));
    let mut kept: Vec<Face> = Vec::new();
    for f in faces {
        if kept.iter().all(|k| iou(k, &f) < NMS_IOU) {
            kept.push(f);
        }
    }
    kept
}

fn iou(a: &Face, b: &Face) -> f32 {
    let x1 = a.x.max(b.x);
    let y1 = a.y.max(b.y);
    let x2 = (a.x + a.w).min(b.x + b.w);
    let y2 = (a.y + a.h).min(b.y + b.h);
    let inter = (x2 - x1).max(0.0) * (y2 - y1).max(0.0);
    let union = a.w * a.h + b.w * b.h - inter;
    if union <= 0.0 {
        0.0
    } else {
        inter / union
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn model_loads_and_finds_no_face_in_a_blank_frame() {
        let (w, h) = (320, 180);
        let frame = vec![30u8; w * h * 4];
        assert_eq!(detect(&frame, w, h).unwrap(), Vec::<Face>::new());
    }

    #[test]
    fn overlapping_boxes_keep_the_best() {
        let face = |x: f32, score: f32| Face { x, y: 0.0, w: 10.0, h: 10.0, score };
        let kept = non_max_suppression(vec![face(0.0, 0.7), face(1.0, 0.9), face(50.0, 0.8)]);
        assert_eq!(kept, vec![face(1.0, 0.9), face(50.0, 0.8)]);
    }
}
