// lib.rs
//
// Copyright (c) 2023-2025 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This crate provides Rust bindings for [OpenNMT/CTranslate2](https://github.com/OpenNMT/CTranslate2).
//!
//! This crate provides the following:
//!
//! * Rust bindings for
//!   [ctranslate2::Translator](https://opennmt.net/CTranslate2/python/ctranslate2.Translator.html),
//!   [ctranslate2::Generator](https://opennmt.net/CTranslate2/python/ctranslate2.Generator.html),
//!   and
//!   [ctranslate2::Whisper](https://opennmt.net/CTranslate2/python/ctranslate2.models.Whisper.html)
//!   provided by CTranslate2, specifically [`sys::Translator`], [`sys::Generator`], and
//!   [`sys::Whisper`].
//! * More user-friendly versions of these, [`Translator`], [`Generator`],
//!   and [`Whisper`] (`whisper` feature is required),
//!   which incorporate tokenizers for easier handling.
//!
//! # Basic Usage
//! The following example translates two strings using default settings and outputs each to
//! the standard output.
//!
//! ```no_run
//! # use anyhow::Result;
//! #
//! use ct2rs::{Config, Translator, TranslationOptions};
//!
//! # fn main() -> Result<()> {
//! let sources = vec![
//!     "Hallo World!",
//!     "This crate provides Rust bindings for CTranslate2."
//! ];
//! let translator = Translator::new("/path/to/model", &Default::default())?;
//! let results = translator.translate_batch(&sources, &Default::default(), None)?;
//! for (r, _) in results{
//!     println!("{}", r);
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Supported Models
//! The `ct2rs` crate has been tested and confirmed to work with the following models:
//!
//! - BART
//! - BLOOM
//! - FALCON
//! - Marian-MT
//! - MPT
//! - NLLB
//! - GPT-2
//! - GPT-J
//! - OPT
//! - T5
//! - Whisper
//!
//! Please see the respective
//! [examples](https://github.com/jkawamoto/ctranslate2-rs/tree/main/examples) for each model.
//!
//! # Stream API
//! This crate also offers a streaming API that utilizes callback closures.
//! Please refer to
//! [the example code](https://github.com/jkawamoto/ctranslate2-rs/blob/main/examples/stream.rs)
//! for more information.
//!
//!

#![cfg_attr(docsrs, feature(doc_cfg))]

#[cfg(feature = "mkl")]
extern crate intel_onemkl_prebuild;

#[cfg(feature = "openblas")]
extern crate openblas_src;

#[cfg(feature = "dnnl")]
extern crate onednn_src;

pub use generator::{GenerationOptions, Generator};
#[cfg(feature = "hub")]
#[cfg_attr(docsrs, doc(cfg(feature = "hub")))]
pub use hub::download_model;
pub use result::GenerationStepResult;
pub use sys::{
    set_log_level, set_random_seed, BatchType, ComputeType, Config, Device, LogLevel,
    ScoringOptions, ScoringResult,
};
pub use tokenizer::Tokenizer;
pub use translator::{TranslationOptions, Translator};
#[cfg(feature = "whisper")]
#[cfg_attr(docsrs, doc(cfg(feature = "whisper")))]
pub use whisper::{Segment, Whisper, WhisperOptions, Word};

mod generator;
mod result;
pub mod sys;
mod tokenizer;
pub mod tokenizers;
mod translator;

#[cfg(feature = "whisper")]
#[cfg_attr(docsrs, doc(cfg(feature = "whisper")))]
mod whisper;

#[cfg(feature = "hub")]
#[cfg_attr(docsrs, doc(cfg(feature = "hub")))]
mod hub;
