// sys.rs
//
// Copyright (c) 2023-2025 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides raw bindings for CTranslate2.
//!
//! This module provides Rust bindings for
//! [ctranslate2::Translator](https://opennmt.net/CTranslate2/python/ctranslate2.Translator.html),
//! [ctranslate2::Generator](https://opennmt.net/CTranslate2/python/ctranslate2.Generator.html),
//! and
//! [ctranslate2::Whisper](https://opennmt.net/CTranslate2/python/ctranslate2.models.Whisper.html)
//! along with
//! related structures.
//!
//! # Translator
//! The main structure is the [`Translator`], which serves as
//! the interface to the translation functionalities of the `ctranslate2` library.
//!
//! In addition to the `Translator`, this module also offers various supportive structures such
//! as [`TranslationOptions`] and [`TranslationResult`].
//!
//! # Generator
//! The [`Generator`] structure is the primary interface, offering the capability
//! to generate text based on a trained model. It is designed for tasks such as text generation,
//! autocompletion, and other similar language generation tasks.
//!
//! Alongside the `Generator`, this module also includes structures that are critical for
//! controlling and understanding the generation process:
//!
//! - [`GenerationOptions`]: A structure containing configuration options for the generation
//!   process,
//!
//! - [`GenerationResult`]: A structure that holds the results of the generation process.
//!
//! # Whisper
//! The main structure is the [`Whisper`].
//!
//! In addition to the `Whisper`, this module also offers various supportive structures such
//! as [`WhisperOptions`], [`DetectionResult`], and [`WhisperGenerationResult`].
//!
//! For more detailed information on each structure and its usage, please refer to their respective
//! documentation within this module.

pub use config::*;
pub use generator::*;
pub use model_memory_reader::*;
pub use scoring::*;
pub use storage_view::*;
pub use translator::*;
pub use types::*;
pub use whisper::*;

mod config;
mod generator;
mod model_memory_reader;
mod scoring;
mod storage_view;
mod translator;
mod types;
mod whisper;
