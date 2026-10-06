// Live subtitles spike, step 1: measure Whisper's real-time factor on chunked audio.
//
// Usage:
//   cargo run --release --example live_bench -- <media_path> [options]
//
// Options (comma-separated lists):
//   --models  ggml-base.bin,ggml-small.bin   model files in ~/.whisper/models
//   --threads 4,8                            Whisper thread counts to try
//   --windows 5,20                           window sizes in seconds
//   --start   60                             offset into the media, in seconds
//   --count   3                              windows transcribed per configuration
//   --lang    en                             language code ("auto" to detect)
//   --vad     ggml-silero-v5.1.2.bin         VAD model to sanity-check ("none" to skip)
//
// RTF here is audio seconds / wall seconds, so 2.0 means twice as fast as real time.

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Instant;
use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperVadContext,
    WhisperVadContextParams, WhisperVadParams,
};

const SAMPLE_RATE: usize = 16_000;

struct Args {
    media: String,
    models: Vec<String>,
    threads: Vec<i32>,
    windows: Vec<usize>,
    start: f64,
    count: usize,
    lang: String,
    vad: Option<String>,
}

fn parse_list<T: std::str::FromStr>(s: &str) -> Vec<T> {
    s.split(',').filter_map(|v| v.trim().parse().ok()).collect()
}

fn parse_args() -> Result<Args, String> {
    let mut it = std::env::args().skip(1);
    let media = it.next().ok_or("missing <media_path>")?;
    let mut args = Args {
        media,
        models: vec![
            "ggml-tiny.bin".into(),
            "ggml-base.bin".into(),
            "ggml-small-q5_1.bin".into(),
            "ggml-small.bin".into(),
        ],
        threads: vec![4, 8],
        windows: vec![5, 20],
        start: 60.0,
        count: 3,
        lang: "en".into(),
        vad: Some("ggml-silero-v5.1.2.bin".into()),
    };
    while let Some(flag) = it.next() {
        let value = it.next().ok_or(format!("missing value for {}", flag))?;
        match flag.as_str() {
            "--models" => args.models = parse_list(&value),
            "--threads" => args.threads = parse_list(&value),
            "--windows" => args.windows = parse_list(&value),
            "--start" => args.start = value.parse().map_err(|_| "bad --start")?,
            "--count" => args.count = value.parse().map_err(|_| "bad --count")?,
            "--lang" => args.lang = value,
            "--vad" => args.vad = (value != "none").then_some(value),
            other => return Err(format!("unknown flag {}", other)),
        }
    }
    Ok(args)
}

fn models_dir() -> PathBuf {
    let home = std::env::var("USERPROFILE")
        .or_else(|_| std::env::var("HOME"))
        .expect("no home directory");
    PathBuf::from(home).join(".whisper").join("models")
}

// Decodes the media's default audio track to 16 kHz mono f32 PCM via an FFmpeg pipe,
// the same way the live worker will.
fn decode_audio(path: &str, start: f64, duration: f64) -> Result<Vec<f32>, String> {
    let output = Command::new("ffmpeg")
        .args(["-v", "error", "-ss", &start.to_string(), "-i", path])
        .args(["-t", &duration.to_string(), "-vn", "-ac", "1", "-ar", "16000"])
        .args(["-f", "f32le", "-"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| format!("failed to run ffmpeg: {}", e))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(output
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect())
}

fn transcribe(
    ctx: &WhisperContext,
    samples: &[f32],
    threads: i32,
    lang: &str,
) -> Result<String, String> {
    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    params.set_translate(false);
    params.set_language(Some(lang));
    params.set_n_threads(threads);

    let mut state = ctx.create_state().map_err(|e| e.to_string())?;
    state.full(params, samples).map_err(|e| e.to_string())?;

    let mut text = String::new();
    for i in 0..state.full_n_segments() {
        if let Some(seg) = state.get_segment(i) {
            text.push_str(&seg.to_str_lossy().map_err(|e| e.to_string())?);
        }
    }
    Ok(text.trim().to_string())
}

fn check_vad(model: &PathBuf, samples: &[f32]) {
    let started = Instant::now();
    let mut ctx = match WhisperVadContext::new(
        model.to_str().unwrap(),
        WhisperVadContextParams::default(),
    ) {
        Ok(ctx) => ctx,
        Err(e) => {
            println!("VAD: failed to load {}: {}", model.display(), e);
            return;
        }
    };
    match ctx.segments_from_samples(WhisperVadParams::default(), samples) {
        Ok(segments) => {
            let audio_secs = samples.len() as f64 / SAMPLE_RATE as f64;
            let n = segments.num_segments();
            let speech: f32 = (0..n)
                .filter_map(|i| segments.get_segment(i))
                .map(|s| s.end - s.start)
                .sum();
            println!(
                "VAD: OK, {} speech segments over {:.0}s of audio ({:.0}s speech, timestamps in centiseconds), {:.0} ms",
                n,
                audio_secs,
                speech / 100.0,
                started.elapsed().as_secs_f64() * 1000.0
            );
            for i in 0..n.min(5) {
                if let Some(s) = segments.get_segment(i) {
                    println!("  {:>7.2}s - {:>7.2}s", s.start / 100.0, s.end / 100.0);
                }
            }
        }
        Err(e) => println!("VAD: detection failed: {}", e),
    }
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("error: {}\n\nsee the usage notes at the top of examples/live_bench.rs", e);
            std::process::exit(2);
        }
    };

    let max_window = *args.windows.iter().max().unwrap_or(&20);
    let total = (max_window * args.count) as f64;
    println!(
        "Decoding {:.0}s of audio from {} starting at {:.0}s...",
        total, args.media, args.start
    );
    let decode_started = Instant::now();
    let audio = decode_audio(&args.media, args.start, total).unwrap_or_else(|e| {
        eprintln!("decode failed: {}", e);
        std::process::exit(1);
    });
    println!(
        "Decoded {:.1}s of audio in {:.0} ms\n",
        audio.len() as f64 / SAMPLE_RATE as f64,
        decode_started.elapsed().as_secs_f64() * 1000.0
    );

    let dir = models_dir();
    if let Some(vad) = &args.vad {
        check_vad(&dir.join(vad), &audio);
        println!();
    }

    println!(
        "{:<22} {:>7} {:>7} {:>9} {:>9} {:>9}",
        "model", "threads", "window", "min RTF", "avg RTF", "max ms"
    );
    let mut samples_text = Vec::new();

    for model in &args.models {
        let path = dir.join(model);
        if !path.exists() {
            println!("{:<22} (not found, skipped)", model);
            continue;
        }
        let load_started = Instant::now();
        let ctx = match WhisperContext::new_with_params(
            path.to_str().unwrap(),
            WhisperContextParameters::default(),
        ) {
            Ok(ctx) => ctx,
            Err(e) => {
                println!("{:<22} (failed to load: {})", model, e);
                continue;
            }
        };
        let load_ms = load_started.elapsed().as_secs_f64() * 1000.0;

        for &threads in &args.threads {
            for &window in &args.windows {
                let window_samples = window * SAMPLE_RATE;
                let mut rtfs = Vec::new();
                let mut max_ms: f64 = 0.0;
                for i in 0..args.count {
                    let from = i * window_samples;
                    let to = (from + window_samples).min(audio.len());
                    if from >= to {
                        break;
                    }
                    let chunk = &audio[from..to];
                    let started = Instant::now();
                    match transcribe(&ctx, chunk, threads, &args.lang) {
                        Ok(text) => {
                            let secs = started.elapsed().as_secs_f64();
                            max_ms = max_ms.max(secs * 1000.0);
                            rtfs.push(chunk.len() as f64 / SAMPLE_RATE as f64 / secs);
                            if i == 0 && threads == args.threads[0] && window == max_window {
                                samples_text.push((model.clone(), text));
                            }
                        }
                        Err(e) => println!("{:<22} transcription failed: {}", model, e),
                    }
                }
                if rtfs.is_empty() {
                    continue;
                }
                let min = rtfs.iter().cloned().fold(f64::INFINITY, f64::min);
                let avg = rtfs.iter().sum::<f64>() / rtfs.len() as f64;
                println!(
                    "{:<22} {:>7} {:>6}s {:>8.1}x {:>8.1}x {:>9.0}",
                    model, threads, window, min, avg, max_ms
                );
            }
        }
        println!("{:<22} (model load {:.0} ms)", "", load_ms);
    }

    println!("\nFirst {}s window, per model:", max_window);
    for (model, text) in samples_text {
        println!("--- {}\n{}\n", model, text);
    }
}
