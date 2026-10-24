// bpe.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides a tokenizer based on the Byte Pair Encoding (BPE) model.
//!
//! Byte Pair Encoding is a sub-word tokenization technique that can dynamically adjust
//! the vocabulary based on the provided corpus, often leading to more efficient
//! representation of text data in machine learning models. For a deeper understanding
//! of BPE, refer to the [original paper](https://www.aclweb.org/anthology/P16-1162/).
//!
//! This module allows for the creation of a [`Tokenizer`] structure instance by specifying a path
//! to a directory containing `vocab.json` and `merges.txt`, which are essential for the BPE
//! algorithm.
//! You can also specify optional suffixes for the decoder to fine-tune the behavior of the decoding
//! process. For more details on how to use suffixes with the BPE decoder, please see the
//! documentation for
//! [BPEDecoder](https://docs.rs/tokenizers/latest/tokenizers/decoders/bpe/struct.BPEDecoder.html).
//!
//! The tokenizer instances created can be utilized in conjunction with structures like
//! [`Translator`][crate::Translator] and [`Generator`][crate::Generator] for tasks such as
//! translation or text generation.
//!
//! ## Example
//! Here is an example of how to create an instance of the Tokenizer
//! and then use it to create an instance of the [`Generator`][crate::Generator] structure:
//!
//! ```no_run
//! # use anyhow::Result;
//! #
//! use ct2rs::{Config, Generator};
//! use ct2rs::tokenizers::bpe;
//!
//! # fn main() -> Result<()> {
//! let path = "/path/to/model";
//! let t = Generator::with_tokenizer(&path, bpe::new(&path, None)?, &Config::default())?;
//! # Ok(())
//! # }
//! ```

use std::path::Path;

use anyhow::{anyhow, Result};
use tokenizers::decoders::bpe::BPEDecoder;
use tokenizers::models::bpe::BPE;
use tokenizers::processors::roberta::RobertaProcessing;
use tokenizers::Tokenizer as HFTokenizer;

use super::hf::Tokenizer;

const VOCAB_FILE: &str = "vocab.json";
const MERGES_FILE: &str = "merges.txt";

/// Create a tokenizer instance by specifying the path to a directory containing `vocab.json`
/// and `mergers.txt`.
pub fn new<T: AsRef<Path>>(path: T, decoder_suffix: Option<String>) -> Result<Tokenizer> {
    from_file(
        path.as_ref().join(VOCAB_FILE),
        path.as_ref().join(MERGES_FILE),
        decoder_suffix,
    )
}

/// Create a tokenizer instance by specifying the path to `vocab.json` and `mergers.txt`.
pub fn from_file<T: AsRef<Path>, U: AsRef<Path>>(
    vocab: T,
    merges: U,
    decoder_suffix: Option<String>,
) -> Result<Tokenizer> {
    let mut res = Tokenizer::from(HFTokenizer::new(
        BPE::from_file(
            vocab
                .as_ref()
                .to_str()
                .ok_or_else(|| anyhow!("invalid path: {}", vocab.as_ref().display()))?,
            merges
                .as_ref()
                .to_str()
                .ok_or_else(|| anyhow!("invalid path: {}", merges.as_ref().display()))?,
        )
        .build()
        .map_err(|err| anyhow!("failed to build a tokenizer: {err}"))?,
    ));

    res.with_decoder(Some(match decoder_suffix {
        None => BPEDecoder::default(),
        Some(s) => BPEDecoder::new(s),
    }))
    .with_post_processor(Some(
        RobertaProcessing::new(("</s>".to_string(), 2), ("<s>".to_string(), 0)).trim_offsets(true),
    ));

    Ok(res)
}
