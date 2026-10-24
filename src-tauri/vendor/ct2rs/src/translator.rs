// translator.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides a text translator with a tokenizer.

use std::fmt::{Debug, Formatter};
use std::path::Path;

use anyhow::{anyhow, Result};

pub use sys::TranslationOptions;

use super::tokenizer::encode_all;
use super::{sys, Config, GenerationStepResult, Tokenizer};

/// A text translator with a tokenizer.
///
/// # Example
/// The following example translates two strings using default settings and outputs each to
/// the standard output.
///
/// ```no_run
/// # use anyhow::Result;
/// #
/// use ct2rs::{Config, Translator, TranslationOptions, GenerationStepResult};
///
/// # fn main() -> Result<()> {
/// let sources = vec![
///     "Hallo World!",
///     "This crate provides Rust bindings for CTranslate2."
/// ];
/// let translator = Translator::new("/path/to/model", &Default::default())?;
/// let results = translator.translate_batch(&sources, &Default::default(), None)?;
/// for (r, _) in results{
///     println!("{}", r);
/// }
/// # Ok(())
/// # }
///```
///
/// The following example translates a single string and uses a callback closure for streaming
/// the output to standard output.
///
///```no_run
/// use std::io::{stdout, Write};
/// use anyhow::Result;
///
/// use ct2rs::{Config, Translator, TranslationOptions, GenerationStepResult};
///
/// # fn main() -> Result<()> {
/// let sources = vec![
///     "Hallo World! This crate provides Rust bindings for CTranslate2."
/// ];
/// let options = TranslationOptions {
///     // beam_size must be 1 to use the stream API.
///     beam_size: 1,
///     ..Default::default()
/// };
/// let mut callback = |step_result: GenerationStepResult| -> Result<()> {
///     print!("{:?}", step_result.text);
///     stdout().flush()?;
///     Ok(())
/// };
/// let translator = Translator::new("/path/to/model", &Config::default())?;
/// let results = translator.translate_batch(&sources, &options, Some(&mut callback))?;
/// # Ok(())
/// # }
/// ```
pub struct Translator<T: Tokenizer> {
    translator: sys::Translator,
    tokenizer: T,
}

impl Translator<crate::tokenizers::auto::Tokenizer> {
    /// Initializes the translator with [`crate::tokenizers::auto::Tokenizer`].
    ///
    /// # Arguments
    /// * `path` - A path to the directory containing the language model to be loaded.
    /// * `config` - A reference to a `Config` structure that specifies various settings
    ///   and configurations for the `Translator`.
    ///
    /// # Returns
    /// Returns a `Result` that, if successful, contains the initialized `Translator`. If an error
    /// occurs during initialization, the function will return an error wrapped in the `Result`.
    pub fn new<U: AsRef<Path>>(path: U, config: &Config) -> anyhow::Result<Self> {
        Self::with_tokenizer(
            &path,
            crate::tokenizers::auto::Tokenizer::new(&path)?,
            config,
        )
    }
}

impl<T: Tokenizer> Translator<T> {
    /// Initializes the translator with the given tokenizer.
    ///
    /// # Arguments
    /// * `path` - The path to the directory containing the language model.
    /// * `tokenizer` - An instance of a tokenizer.
    /// * `config` - A reference to a `Config` structure specifying the settings for the
    ///   `Translator`.
    ///
    /// # Returns
    /// Returns a `Result` containing the initialized `Translator`, or an error if initialization
    /// fails.
    ///
    /// # Example
    /// This example demonstrates creating a `Translator` instance with a Sentencepiece tokenizer.
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// use ct2rs::{Config, TranslationOptions, Translator};
    /// use ct2rs::tokenizers::sentencepiece::Tokenizer;
    ///
    /// # fn main() -> Result<()> {
    /// let translator = Translator::with_tokenizer(
    ///     "/path/to/model",
    ///     Tokenizer::from_file("/path/to/source.spm", "/path/to/target.spm")?,
    ///     &Config::default()
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    pub fn with_tokenizer<U: AsRef<Path>>(
        path: U,
        tokenizer: T,
        config: &Config,
    ) -> anyhow::Result<Self> {
        Ok(Translator {
            translator: sys::Translator::new(path, config)?,
            tokenizer,
        })
    }

    /// Translates multiple lists of strings in a batch processing manner.
    ///
    /// This function takes a vector of strings and performs batch translation according to the
    /// specified settings in `options`. The results of the batch translation are returned as a
    /// vector. An optional `callback` closure can be provided which is invoked for each new token
    /// generated during the translation process. This allows for step-by-step reception of the
    /// batch translation results. If the callback returns `Err`, it will stop the translation for
    /// that batch. Note that if a callback is provided, `options.beam_size` must be set to `1`.
    ///
    /// # Arguments
    /// * `source` - A vector of strings to be translated.
    /// * `options` - Settings applied to the batch translation process.
    /// * `callback` - An optional mutable reference to a closure that is called for each token
    ///   generation step. The closure takes a `GenerationStepResult` and returns a
    ///   `anyhow::Result<()>`. If it returns `Err`, the translation process for the current batch
    ///   will stop.
    ///
    /// # Returns
    /// Returns a `Result` containing a vector of `TranslationResult` if successful, or an error if
    /// the translation fails.
    ///
    pub fn translate_batch<U, V, W>(
        &self,
        sources: &[U],
        options: &TranslationOptions<V, W>,
        callback: Option<&mut dyn FnMut(GenerationStepResult) -> Result<()>>,
    ) -> anyhow::Result<Vec<(String, Option<f32>)>>
    where
        U: AsRef<str>,
        V: AsRef<str>,
        W: AsRef<str>,
    {
        let output = if let Some(callback) = callback {
            let mut callback_result = Ok(());
            let mut wrapped_callback = |r: sys::GenerationStepResult| -> bool {
                if let Err(e) =
                    GenerationStepResult::from_ffi(r, &self.tokenizer).and_then(|r| callback(r))
                {
                    callback_result = Err(e);
                    return true;
                }
                false
            };
            let output = self.translator.translate_batch(
                &encode_all(&self.tokenizer, sources)?,
                options,
                Some(&mut wrapped_callback),
            )?;
            callback_result?;
            output
        } else {
            self.translator.translate_batch(
                &encode_all(&self.tokenizer, sources)?,
                options,
                None,
            )?
        };

        let mut res = Vec::new();
        for r in output.into_iter() {
            let score = r.score();
            let hypotheses = r
                .hypotheses
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("no results are returned"))?;
            res.push((
                self.tokenizer
                    .decode(hypotheses)
                    .map_err(|err| anyhow!("failed to decode: {err}"))?,
                score,
            ));
        }
        Ok(res)
    }

    /// Translates multiple lists of strings with target prefixes in a batch processing manner.
    ///
    /// This function takes a vector of strings and corresponding target prefixes, performing
    /// batch translation according to the specified settings in `options`. An optional `callback`
    /// closure can be provided which is invoked for each new token generated during the translation
    /// process.
    ///
    /// This function is similar to `translate_batch`, with the addition of handling target prefixes
    /// that guide the translation process. For more detailed parameter and option descriptions,
    /// refer to the documentation for [`Translator::translate_batch`].
    ///
    /// # Arguments
    /// * `sources` - A vector of strings translated.
    /// * `target_prefix` - A vector of token lists, each list representing a sequence of target
    ///   prefix tokens that provide a starting point for the translation output.
    /// * `options` - Settings applied to the batch translation process.
    /// * `callback` - An optional mutable reference to a closure that is called for each token
    ///   generation step. The closure takes a `GenerationStepResult` and returns a
    ///   `anyhow::Result<()>`. If it returns `Err`, the translation process for the current batch
    ///   will stop.
    ///
    /// # Returns
    /// Returns a `Result` containing a vector of `TranslationResult` if successful, or an error if
    /// the translation fails.
    pub fn translate_batch_with_target_prefix<U, V, W, E>(
        &self,
        sources: &[U],
        target_prefixes: &Vec<Vec<V>>,
        options: &TranslationOptions<W, E>,
        callback: Option<&mut dyn FnMut(GenerationStepResult) -> Result<()>>,
    ) -> anyhow::Result<Vec<(String, Option<f32>)>>
    where
        U: AsRef<str>,
        V: AsRef<str>,
        W: AsRef<str>,
        E: AsRef<str>,
    {
        let output = if let Some(callback) = callback {
            let mut callback_result = Ok(());
            let mut wrapped_callback = |r: sys::GenerationStepResult| -> bool {
                if let Err(e) =
                    GenerationStepResult::from_ffi(r, &self.tokenizer).and_then(|r| callback(r))
                {
                    callback_result = Err(e);
                    return true;
                }
                false
            };
            let output = self.translator.translate_batch_with_target_prefix(
                &encode_all(&self.tokenizer, sources)?,
                target_prefixes,
                options,
                Some(&mut wrapped_callback),
            )?;
            callback_result?;
            output
        } else {
            self.translator.translate_batch_with_target_prefix(
                &encode_all(&self.tokenizer, sources)?,
                target_prefixes,
                options,
                None,
            )?
        };

        let mut res = Vec::new();
        for (r, prefix) in output.into_iter().zip(target_prefixes) {
            let score = r.score();
            let mut hypotheses = r
                .hypotheses
                .into_iter()
                .next()
                .ok_or_else(|| anyhow!("no results are returned"))?;
            hypotheses.drain(0..prefix.len());

            res.push((
                self.tokenizer
                    .decode(hypotheses)
                    .map_err(|err| anyhow!("failed to decode: {err}"))?,
                score,
            ));
        }
        Ok(res)
    }

    /// Number of batches in the work queue.
    #[inline]
    pub fn num_queued_batches(&self) -> anyhow::Result<usize> {
        self.translator.num_queued_batches()
    }

    /// Number of batches in the work queue or currently processed by a worker.
    #[inline]
    pub fn num_active_batches(&self) -> anyhow::Result<usize> {
        self.translator.num_active_batches()
    }

    /// Number of parallel replicas.
    #[inline]
    pub fn num_replicas(&self) -> anyhow::Result<usize> {
        self.translator.num_replicas()
    }
}

impl<T: Tokenizer> Debug for Translator<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.translator)
    }
}

#[cfg(test)]
#[cfg(feature = "hub")]
mod tests {
    use crate::{download_model, Config, Device, TranslationOptions, Translator};

    const MODEL_ID: &str = "jkawamoto/fugumt-en-ja-ct2";

    #[test]
    #[ignore]
    fn test_translate() {
        let model_path = download_model(MODEL_ID).unwrap();
        let t = Translator::new(
            &model_path,
            &Config {
                device: if cfg!(feature = "cuda") {
                    Device::CUDA
                } else {
                    Device::CPU
                },
                ..Default::default()
            },
        )
        .unwrap();

        let res = t
            .translate_batch(
                &["Hellow"],
                &TranslationOptions {
                    beam_size: 1,
                    sampling_temperature: 0.,
                    ..Default::default()
                },
                None,
            )
            .unwrap();
        assert_eq!(res[0].0, "こんにちは");
    }

    #[test]
    #[ignore]
    fn test_translator_debug() {
        let model_path = download_model(MODEL_ID).unwrap();
        let t = Translator::new(
            &model_path,
            &Config {
                device: if cfg!(feature = "cuda") {
                    Device::CUDA
                } else {
                    Device::CPU
                },
                ..Default::default()
            },
        )
        .unwrap();

        assert!(format!("{:?}", t).contains(model_path.file_name().unwrap().to_str().unwrap()));
    }
}
