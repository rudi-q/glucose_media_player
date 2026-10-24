// nllb.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! Translate a file using NLLB models.
//!
//! In this example, we will use
//! the [NLLB](https://huggingface.co/docs/transformers/model_doc/nllb) model
//! to perform a translation from English to Japanese.
//! The original Python version of the code can be found in the
//! [CTranslate2 documentation](https://opennmt.net/CTranslate2/guides/transformers.html#nllb).
//!
//! First, convert the model files with the following command:
//!
//! ```bash
//! pip install -U ctranslate2 huggingface_hub torch transformers
//!
//! ct2-transformers-converter --model facebook/nllb-200-distilled-600M \
//!     --output_dir nllb-200-distilled-600M --copy_files tokenizer.json
//! ```
//!
//! Note: The above command copies `tokenizer.json` because it is provided by the
//! [facebook/nllb-200-distilled-600M](https://huggingface.co/facebook/nllb-200-distilled-600M)
//! repository.
//! If you prefer to use another repository that offers `source.spm` and `target.spm`,
//! you can copy it using the option `--copy_files source.spm target.spm`.
//!
//! Create a file named `prompt.txt`, write the sentence you want to translate into it,
//! and save the file.
//! Then, execute the sample code below with the following command:
//!
//! ```bash
//! cargo run --example nllb -- ./nllb-200-distilled-600M
//! ```
//!

use std::fs::File;
use std::io;
use std::io::{BufRead, BufReader};
use std::time;

use anyhow::Result;
use clap::Parser;

use ct2rs::{Config, Device, Translator};

/// Translate a file using NLLB.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the file contains prompts.
    #[arg(short, long, value_name = "FILE", default_value = "prompt.txt")]
    prompt: String,
    /// Target language.
    #[arg(short, long, value_name = "LANG", default_value = "jpn_Jpan")]
    target: String,
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
    let target_prefixes = vec![vec![args.target]; sources.len()];

    let now = time::Instant::now();
    let res = t.translate_batch_with_target_prefix(
        &sources,
        &target_prefixes,
        &Default::default(),
        None,
    )?;
    let elapsed = now.elapsed();

    for (r, _) in res {
        println!("{r}");
    }
    println!("Time taken: {:?}", elapsed);

    Ok(())
}
