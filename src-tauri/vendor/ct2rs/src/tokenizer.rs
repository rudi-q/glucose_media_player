// tokenizer.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module defines [`Tokenizer`] trait.

/// Defines the necessary functions for a tokenizer.
///
/// This trait provides the core functionality needed to convert strings to sequences of tokens
/// and vice versa. It is essential for text processing tasks such as natural language processing,
/// where text needs to be broken down into manageable pieces or reconstructed from tokenized forms.
pub trait Tokenizer {
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
    fn encode(&self, input: &str) -> anyhow::Result<Vec<String>>;

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
    fn decode(&self, tokens: Vec<String>) -> anyhow::Result<String>;
}

#[inline]
pub(crate) fn encode_all<T: Tokenizer, U: AsRef<str>>(
    tokenizer: &T,
    sources: &[U],
) -> anyhow::Result<Vec<Vec<String>>> {
    sources
        .iter()
        .map(|s| tokenizer.encode(s.as_ref()))
        .collect()
}
