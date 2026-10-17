// marian-mt.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! Translate a file using Marian-MT models.
//!
//! In this example, we will use
//! the [MarianMT](https://huggingface.co/docs/transformers/model_doc/marian) model
//! to perform a translation from English to German.
//! The original Python version of the code can be found in the
//! [CTranslate2 documentation](https://opennmt.net/CTranslate2/guides/transformers.html#marianmt).
//!
//! First, convert the model files with the following command:
//!
//! ```bash
//! pip install -U ctranslate2 huggingface_hub torch transformers
//!
//! ct2-transformers-converter --model Helsinki-NLP/opus-mt-en-de --output_dir opus-mt-en-de \
//!     --copy_files source.spm target.spm
//! ```
//!
//! Note: The above command copies `source.spm` and `target.spm` because they are provided by the
//! [Helsinki-NLP/opus-mt-en-de](https://huggingface.co/Helsinki-NLP/opus-mt-en-de) repository.
//! If you prefer to use another repository that offers `tokenizer.json`,
//! you can copy it using the option `--copy_files tokenizer.json`.
//!
//! Create a file named `prompt.txt`, write the sentence you want to translate into it,
//! and save the file.
//! Then, execute the sample code below with the following command:
//!
//! ```bash
//! cargo run --example marian-mt -- ./opus-mt-en-de
//! ```
//!

use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::time;

use anyhow::Result;
use clap::Parser;

use ct2rs::{Config, Device, Translator};

/// Translate a file using Marian-MT models.
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

    let t = Translator::new(&args.path, &cfg)?;

    let sources = BufReader::new(File::open(args.prompt)?)
        .lines()
        .collect::<std::result::Result<Vec<String>, io::Error>>()?;

    let now = time::Instant::now();
    let res = t.translate_batch(&sources, &Default::default(), None)?;
    let elapsed = now.elapsed();

    for (r, _) in res {
        println!("{r}");
    }
    println!("Time taken: {elapsed:?}");

    Ok(())
}
