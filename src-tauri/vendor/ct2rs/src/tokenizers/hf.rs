// hf.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! A module for utilizing the tokenizer provided by the Hugging Face's
//! [`tokenizers` crate](https://docs.rs/tokenizers/).
//!
//! This module allows the creation of a [`Tokenizer`] structure instance by specifying
//! the path to a directory containing `tokenizer.json`. The created instance can then be
//! used in [`Translator`](crate::Translator) and [`Generator`](crate::Generator).
//!
//! # Example
//! Here is an example of how to create an instance of the [`Tokenizer`] structure
//! and then use it to create an instance of the [`Translator`](crate::Translator):
//!
//! ```no_run
//! # use anyhow::Result;
//! #
//! use ct2rs::{Config, Translator};
//! use ct2rs::tokenizers::hf::Tokenizer;
//!
//! # fn main() -> Result<()> {
//! let path = "/path/to/model";
//! let t = Translator::with_tokenizer(&path, Tokenizer::new(&path)?, &Config::default())?;
//! # Ok(())
//! # }
//! ```

use std::ops::{Deref, DerefMut};
use std::path::Path;

use anyhow::{anyhow, Result};
use tokenizers::Decoder;

const TOKENIZER_FILENAME: &str = "tokenizer.json";

/// Tokenizer that utilize the tokenizer provided by the Hugging Face's `tokenizers` crate.
pub struct Tokenizer {
    tokenizer: tokenizers::Tokenizer,
    special_token: bool,
}

impl Tokenizer {
    /// Create a tokenizer instance by specifying the path to a directory containing
    /// `tokenizer.json`.
    pub fn new<T: AsRef<Path>>(path: T) -> Result<Self> {
        Tokenizer::from_file(path.as_ref().join(TOKENIZER_FILENAME))
    }

    /// Create a tokenizer instance by specifying the path to `tokenizer.json`.
    pub fn from_file<T: AsRef<Path>>(path: T) -> Result<Self> {
        Ok(Self {
            tokenizer: tokenizers::tokenizer::Tokenizer::from_file(path)
                .map_err(|err| anyhow!("failed to load a tokenizer: {err}"))?,
            special_token: true,
        })
    }

    /// Disable adding special tokens.
    ///
    /// See also [Special tokens in translation](https://opennmt.net/CTranslate2/guides/transformers.html#special-tokens-in-translation).
    pub fn disable_spacial_token(&mut self) -> &mut Self {
        self.special_token = false;
        self
    }

    /// Returns a mutable reference of the `tokenizers::Tokenizer`.
    #[inline]
    pub fn inner(&mut self) -> &mut tokenizers::Tokenizer {
        self.deref_mut()
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
        self.tokenizer
            .encode(input, self.special_token)
            .map(|r| r.get_tokens().to_vec())
            .map_err(|err| anyhow!("failed to encode the given input: {err}"))
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
        let decoder = self
            .tokenizer
            .get_decoder()
            .ok_or_else(|| anyhow!("no decoder is provided"))?;

        decoder
            .decode(tokens)
            .map_err(|err| anyhow!("failed to decode: {err}"))
    }
}

impl From<tokenizers::Tokenizer> for Tokenizer {
    fn from(tokenizer: tokenizers::Tokenizer) -> Self {
        Self {
            tokenizer,
            special_token: true,
        }
    }
}

impl Deref for Tokenizer {
    type Target = tokenizers::Tokenizer;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.tokenizer
    }
}

impl DerefMut for Tokenizer {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.tokenizer
    }
}
