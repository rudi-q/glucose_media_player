// sentencepiece.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! A module for utilizing the tokenizer based on
//! [Sentencepiece crate](https://docs.rs/sentencepiece/).
//!
//! This module facilitates the creation of a [`Tokenizer`] structure instance by specifying
//! the path to a directory containing `source.spm` and `target.spm`. The created `Tokenizer`
//! instance can then be utilized in [`Translator`](crate::Translator)
//! and [`Generator`](crate::Generator).
//!
//! ## Example
//! Here is an example of how to create an instance of the [`Tokenizer`] structure
//! and then use it to create an instance of the [`Generator`](crate::Generator) structure:
//!
//! ```no_run
//! # use anyhow::Result;
//! #
//! use ct2rs::{Config, Generator};
//! use ct2rs::tokenizers::sentencepiece::Tokenizer;
//!
//! # fn main() -> Result<()> {
//! let path = "/path/to/model";
//! let t = Generator::with_tokenizer(&path, Tokenizer::new(&path)?, &Config::default())?;
//! # Ok(())
//! # }
//! ```

use std::path::Path;

use anyhow::{Error, Result};
use sentencepiece::SentencePieceProcessor;

const SOURCE_SPM_FILE: &str = "source.spm";
const TARGET_SPM_FILE: &str = "target.spm";

pub struct Tokenizer {
    encoder: SentencePieceProcessor,
    decoder: SentencePieceProcessor,
}

impl Tokenizer {
    /// Create a tokenizer instance by specifying the path to a directory containing `source.spm`
    /// and `target.spm`.
    pub fn new<T: AsRef<Path>>(path: T) -> Result<Self> {
        Tokenizer::from_file(
            path.as_ref().join(SOURCE_SPM_FILE),
            path.as_ref().join(TARGET_SPM_FILE),
        )
    }

    /// Create a tokenizer instance by specifying the path to `source.spm` and `target.spm`.
    pub fn from_file<T: AsRef<Path>, U: AsRef<Path>>(src: T, target: U) -> Result<Self> {
        Ok(Self {
            encoder: SentencePieceProcessor::open(src)?,
            decoder: SentencePieceProcessor::open(target)?,
        })
    }
}

impl crate::Tokenizer for Tokenizer {
    /// Encodes a given string into a sequence of tokens.
    ///
    /// This function takes a reference to a string and returns a vector of token strings
    /// resulting from the tokenization process.
    ///
    /// # Arguments
    /// * `input` - A reference to the string to be tokenized.
    ///
    /// # Returns
    /// A `Result` containing either the vector of tokens if successful or an error if the
    /// tokenization fails.
    fn encode(&self, input: &str) -> Result<Vec<String>> {
        let mut source: Vec<String> = self
            .encoder
            .encode(input)?
            .iter()
            .map(|v| v.piece.to_string())
            .collect();
        source.push("</s>".to_string());
        Ok(source)
    }

    /// Decodes a given sequence of tokens back into a single string.
    ///
    /// This function takes a vector of token strings and reconstructs the original string.
    ///
    /// # Arguments
    /// * `tokens` - A vector of strings representing the tokens to be decoded.
    ///
    /// # Returns
    /// A `Result` containing either the reconstructed string if successful or an error if the
    /// decoding fails.
    fn decode(&self, tokens: Vec<String>) -> Result<String> {
        self.decoder.decode_pieces(&tokens).map_err(Error::new)
    }
}
