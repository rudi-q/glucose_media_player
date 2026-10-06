// Live subtitles spike: transcribes part of a media file the way live mode does (20 s
// chunks, one cue builder per chunk) and reports how many cues break the subtitle
// formatting rules in src/subtitle_format.rs.
//
// Usage:
//   cargo run --example format_check --manifest-path=src-tauri/Cargo.toml -- <media_path> [options]
//
// Options:
//   --start  0                        offset into the media, in seconds
//   --secs   300                      how much audio to transcribe
//   --map    1                        absolute audio stream index (default: FFmpeg's choice)
//   --model  ggml-small-q5_1.bin      model file in ~/.whisper/models
//   --lang   en                       language code
//   --list                            print every cue, not just those breaking a rule

#[path = "../src/subtitle_format.rs"]
mod subtitle_format;

use std::process::{Command, Stdio};
use subtitle_format::{
    build_cues, check, request_word_timestamps, resume_point, words_from_state, Cue, MIN_GAP,
};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

const SAMPLE_RATE: usize = 16_000;
const CHUNK_SECS: usize = 20;

fn main() {
    let mut args = std::env::args().skip(1);
    let media = args.next().expect("missing <media_path>");
    let (mut start, mut secs, mut map, mut list) = (0.0f64, 300.0f64, None::<String>, false);
    let mut model = "ggml-small-q5_1.bin".to_string();
    let mut lang = "en".to_string();
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--list" => list = true,
            "--start" => start = args.next().unwrap().parse().unwrap(),
            "--secs" => secs = args.next().unwrap().parse().unwrap(),
            "--map" => map = args.next(),
            "--model" => model = args.next().unwrap(),
            "--lang" => lang = args.next().unwrap(),
            other => panic!("unknown flag {}", other),
        }
    }

    let mut cmd = Command::new("ffmpeg");
    cmd.args(["-v", "error", "-ss", &start.to_string(), "-i", &media]);
    if let Some(index) = &map {
        cmd.args(["-map", &format!("0:{}", index)]);
    }
    cmd.args(["-t", &secs.to_string(), "-vn", "-ac", "1", "-ar", "16000", "-f", "f32le", "-"]);
    let output = cmd.stdout(Stdio::piped()).output().expect("failed to run ffmpeg");
    let audio: Vec<f32> = output
        .stdout
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    let home = std::env::var("USERPROFILE").or_else(|_| std::env::var("HOME")).unwrap();
    let path = format!("{}/.whisper/models/{}", home, model);
    let ctx = WhisperContext::new_with_params(&path, WhisperContextParameters::default())
        .expect("failed to load model");

    let mut cues: Vec<Cue> = Vec::new();
    let mut pos = 0;
    while pos < audio.len() {
        let chunk = &audio[pos..(pos + CHUNK_SECS * SAMPLE_RATE).min(audio.len())];
        let is_last = pos + chunk.len() == audio.len();
        let offset = start + pos as f64 / SAMPLE_RATE as f64;
        let chunk_secs = chunk.len() as f64 / SAMPLE_RATE as f64;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_language(Some(&lang));
        params.set_n_threads(8);
        params.set_no_context(true);
        params.set_suppress_blank(true);
        request_word_timestamps(&mut params);
        let mut state = ctx.create_state().unwrap();
        state.full(params, chunk).expect("transcription failed");
        let words = words_from_state(&state, offset, chunk_secs);
        let mut built = build_cues(&words, offset + chunk_secs - MIN_GAP);
        // Same as live mode: a cue cut off by the chunk end is redone in the next chunk.
        let resume = (!is_last)
            .then(|| resume_point(&words, &built, offset, offset + chunk_secs))
            .flatten();
        let advance = match resume {
            Some(time) => {
                built.retain(|c| c.start < time);
                ((time - offset) * SAMPLE_RATE as f64).ceil() as usize
            }
            None => chunk.len(),
        };
        cues.extend(built);
        pos += advance.min(chunk.len());
        eprint!(".");
    }
    eprintln!();

    let v = check(&cues);
    let chars: usize = cues.iter().map(|c| c.text.chars().count()).sum();
    println!(
        "{} cues over {:.0}s of audio ({} characters)\n{:#?}",
        cues.len(),
        audio.len() as f64 / SAMPLE_RATE as f64,
        chars,
        v
    );
    println!();
    for (k, cue) in cues.iter().enumerate() {
        let duration = cue.end - cue.start;
        let cps = cue.text.chars().count() as f64 / duration;
        let max_line = cue.text.split('\n').map(|l| l.chars().count()).max().unwrap_or(0);
        let gap = cues.get(k + 1).map(|n| n.start - cue.end);
        let flags: Vec<&str> = [
            (cue.text.split('\n').count() > 2, "lines"),
            (max_line > 42, "cpl"),
            (duration < 5.0 / 6.0 - 1e-6, "short"),
            (duration > 7.0 + 1e-6, "long"),
            (cps > 20.0 + 1e-6, "cps"),
            (gap.is_some_and(|g| g < MIN_GAP - 1e-6), "gap"),
        ]
        .into_iter()
        .filter_map(|(bad, name)| bad.then_some(name))
        .collect();
        if list || !flags.is_empty() {
            println!(
                "{:>8.2} {:>8.2} {:>5.1}cps {:<14} {:?}",
                cue.start,
                cue.end,
                cps,
                flags.join(","),
                cue.text
            );
        }
    }
}
