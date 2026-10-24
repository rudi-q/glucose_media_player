// opt.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! Generate text using OPT models.
//!
//! In this example, we will use
//! the [OPT](https://huggingface.co/docs/transformers/model_doc/opt) model
//! to generate text.
//!
//! The original Python version of the code can be found in the
//! [CTranslate2 documentation](https://opennmt.net/CTranslate2/guides/transformers.html#opt).
//!
//! First, convert the model files with the following command:
//!
//! ```bash
//! pip install -U ctranslate2 huggingface_hub torch transformers
//!
//! ct2-transformers-converter --model facebook/opt-350m --output_dir opt-350m \
//!     --copy_files vocab.json merges.txt
//! ```
//!
//! Create a file named `prompt.txt`, write the prompt, and save the file.
//! Then, execute the sample code below with the following command:
//!
//! ```bash
//! cargo run --example opt -- ./opt-350m
//! ```
//!

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time;

use anyhow::Result;
use clap::Parser;

use ct2rs::tokenizers::bpe;
use ct2rs::{Config, Device, GenerationOptions, Generator};

/// Generate text using OPT models.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the file contains prompts.
    #[arg(short, long, value_name = "FILE", default_value = "prompt.txt")]
    prompt: String,
    /// Use CUDA.
    #[arg(short, long)]
    cuda: bool,
    /// Path to the directory that contains model.bin.
    path: String,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let cfg = if args.cuda {
        Config {
            device: Device::CUDA,
            device_indices: vec![0],
            ..Config::default()
        }
    } else {
        Config::default()
    };

    // Use BPE tokenizer.
    let g = Generator::with_tokenizer(
        &args.path,
        bpe::new(&args.path, Some("Ġ".to_string()))?,
        &cfg,
    )?;
    let prompts = BufReader::new(File::open(args.prompt)?).lines().try_fold(
        String::new(),
        |mut acc, line| {
            line.map(|l| {
                acc.push_str(&l);
                acc
            })
        },
    )?;

    let now = time::Instant::now();
    let res = g.generate_batch(
        &[prompts],
        &GenerationOptions {
            beam_size: 15,
            max_length: 50,
            include_prompt_in_result: false,
            ..GenerationOptions::default()
        },
        None,
    )?;
    let elapsed = now.elapsed();

    for (r, _) in res {
        println!("{}", r.join("\n"));
    }
    println!("Time taken: {elapsed:?}");

    Ok(())
}
