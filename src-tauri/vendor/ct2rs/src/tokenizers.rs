// tokenizers.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides tokenizers.
//!
//! Currently, this module implements four tokenizers:
//! * [`auto`] module provides a tokenizer that automatically determines the appropriate tokenizer.
//! * [`hf`] module provides the tokenizer provided by the Hugging Face's
//!   [`tokenizers` crate](https://docs.rs/tokenizers/),
//! * [`sentencepiece`] module provides a tokenizer based on
//!   [Sentencepiece crate](https://docs.rs/sentencepiece/).
//! * [`bpe`] module provides a tokenizer based on the Byte Pair Encoding (BPE) model,
//!
//! ## Examples:
//! Here is an example of using [`auto::Tokenizer`] to build a Translator and translate a string:
//!
//! ```no_run
//! # use anyhow::Result;
//! #
//! use ct2rs::{Config, Translator};
//!
//! # fn main() -> Result<()> {
//! // Translator::new creates a translator instance with auto::Tokenizer.
//! let t = Translator::new("/path/to/model", &Config::default())?;
//! let res = t.translate_batch(
//!     &vec!["Hallo World!"],
//!     &Default::default(),
//!     None,
//! )?;
//! for r in res {
//!     println!("{:?}", r);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! The following example translates English to German and Japanese using the tokenizer provided by
//! the Hugging Face's [`tokenizers` crate](https://docs.rs/tokenizers/).
//! ```no_run
//! # use anyhow::Result;
//!
//! use ct2rs::{Config, TranslationOptions, Translator};
//! use ct2rs::tokenizers::hf::Tokenizer;
//!
//! # fn main() -> Result<()> {
//! let path = "/path/to/model";
//! let t = Translator::with_tokenizer(&path, Tokenizer::new(&path)?, &Config::default())?;
//! let res = t.translate_batch_with_target_prefix(
//!     &vec![
//!         "Hello world!",
//!         "This library provides Rust bindings for CTranslate2.",
//!     ],
//!     &vec![vec!["deu_Latn"], vec!["jpn_Jpan"]],
//!     &TranslationOptions {
//!         return_scores: true,
//!         ..Default::default()
//!     },
//!     None
//! )?;
//! for r in res {
//!     println!("{}, (score: {:?})", r.0, r.1);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! The following example generates text using the tokenizer provided by
//! [Sentencepiece crate](https://docs.rs/sentencepiece/).
//! ```no_run
//! # use anyhow::Result;
//! use ct2rs::{Config, Device, Generator, GenerationOptions};
//! use ct2rs::tokenizers::sentencepiece::Tokenizer;
//!
//! # fn main() -> Result<()> {
//! let path = "/path/to/model";
//! let g = Generator::with_tokenizer(&path, Tokenizer::new(&path)?, &Config::default())?;
//! let res = g.generate_batch(
//!     &vec!["prompt"],
//!     &GenerationOptions::default(),
//!     None,
//! )?;
//! for r in res {
//!     println!("{:?}", r.0);
//! }
//! # Ok(())
//! # }
//! ```

pub mod auto;
#[cfg(feature = "tokenizers")]
pub mod bpe;
#[cfg(feature = "tokenizers")]
pub mod hf;
#[cfg(feature = "sentencepiece")]
pub mod sentencepiece;
