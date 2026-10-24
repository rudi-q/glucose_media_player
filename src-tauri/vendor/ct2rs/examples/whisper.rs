// whisper.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! Transcribe a WAV file using Whisper models.
//!
//! In this example, we will use
//! the [Whisper](https://huggingface.co/docs/transformers/model_doc/whisper) model
//! to transcribe a WAV file.
//! The original Python version of the code can be found in the
//! [CTranslate2 documentation](https://opennmt.net/CTranslate2/guides/transformers.html#whisper).
//!
//! First, convert the model files with the following command:
//!
//! ```bash
//! pip install -U ctranslate2 huggingface_hub torch transformers
//!
//! ct2-transformers-converter --model openai/whisper-tiny --output_dir whisper-tiny-ct2 \
//!     --copy_files preprocessor_config.json tokenizer.json
//! ```
//!
//! Then, execute the sample code below with the following command:
//!
//! ```bash
//! cargo run -F whisper --example whisper -- ./whisper-tiny-ct2 audio.wav
//! ```

use std::path::{Path, PathBuf};

use anyhow::Result;
use clap::Parser;
use hound::WavReader;

use ct2rs::Whisper;

#[cfg(not(feature = "whisper"))]
compile_error!("This example requires 'whisper' feature.");

/// Transcribe a file using Whisper models.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the directory that contains model.bin.
    model_dir: PathBuf,
    /// Path to the WAVE file.
    audio_file: PathBuf,
}

fn main() -> Result<()> {
    let args = Args::parse();

    let whisper = Whisper::new(args.model_dir, Default::default())?;

    let samples = read_audio(args.audio_file, whisper.sampling_rate())?;

    let res = whisper.generate(&samples, None, false, &Default::default())?;
    for r in res {
        println!("{}", r);
    }

    Ok(())
}

fn read_audio<T: AsRef<Path>>(path: T, sample_rate: usize) -> Result<Vec<f32>> {
    // Should use a better resampling algorithm.
    fn resample(samples: Vec<f32>, src_rate: usize, target_rate: usize) -> Vec<f32> {
        samples
            .into_iter()
            .step_by(src_rate / target_rate)
            .collect()
    }

    let mut reader = WavReader::open(path)?;
    let spec = reader.spec();

    let max = 2_i32.pow((spec.bits_per_sample - 1) as u32) as f32;
    let samples = reader
        .samples::<i32>()
        .map(|s| s.unwrap() as f32 / max)
        .collect::<Vec<f32>>();

    if spec.channels == 1 {
        return Ok(resample(samples, spec.sample_rate as usize, sample_rate));
    }

    let mut mono = vec![];
    for chunk in samples.chunks(2) {
        if chunk.len() == 2 {
            mono.push((chunk[0] + chunk[1]) / 2.);
        }
    }

    Ok(resample(mono, spec.sample_rate as usize, sample_rate))
}
