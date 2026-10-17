// generator.rs
//
// Copyright (c) 2023-2025 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides a text generator with a tokenizer.

use std::fmt::{Debug, Formatter};
use std::path::Path;

use anyhow::{anyhow, Result};

pub use sys::GenerationOptions;

use crate::tokenizer::encode_all;

use super::{sys, Config, GenerationStepResult, ScoringOptions, ScoringResult, Tokenizer};

/// A text generator with a tokenizer.
///
/// # Example
/// The following example generates text following two prompts in a batch process,
/// with each result output to the standard output.
///
/// ```no_run
/// # use anyhow::Result;
/// use ct2rs::{Config, Device, Generator, GenerationOptions};
///
/// # fn main() -> Result<()> {
/// let generator = Generator::new("/path/to/model", &Config::default())?;
/// let res = generator.generate_batch(
///     &vec!["Hello, I am"],
///     &GenerationOptions::default(),
///     None
/// )?;
/// for r in res {
///     println!("{:?}", r);
/// }
/// # Ok(())
/// # }
/// ```
///
/// The following example generates text following a single prompt and outputs it to the standard
/// output using a callback closure for stream processing.
///
/// ```no_run
/// use std::io::{stdout, Write};
/// # use anyhow::Result;
///
/// use ct2rs::{Config, Device, Generator, GenerationOptions};
///
/// # fn main() -> Result<()> {
/// use ct2rs::GenerationStepResult;
/// let generator = Generator::new("/path/to/model", &Config::default())?;
/// let _ = generator.generate_batch(
///     &vec!["Hello, I am"],
///     &GenerationOptions{
///         // beam_size must be 1 to use the stream API.
///         beam_size: 1,
///         ..Default::default()
///     },
///     Some(&mut |step_result: GenerationStepResult| -> Result<()> {
///         print!("{:?}", step_result.text);
///         stdout().flush()?;
///         Ok(())
///     })
/// )?;
/// # Ok(())
/// # }
/// ```
pub struct Generator<T: Tokenizer> {
    generator: sys::Generator,
    tokenizer: T,
}

impl Generator<crate::tokenizers::auto::Tokenizer> {
    /// Initializes the generator with [`crate::tokenizers::auto::Tokenizer`].
    ///
    /// # Arguments
    /// * `path` - A path to the directory containing the language model to be loaded.
    /// * `config` - A reference to a `Config` structure that specifies various settings
    ///   and configurations for the `Generator`.
    ///
    /// # Returns
    /// Returns a `Result` that, if successful, contains the initialized `Generator`. If an error
    /// occurs during initialization, the function will return an error wrapped in the `Result`.
    pub fn new<T: AsRef<Path>>(path: T, config: &Config) -> anyhow::Result<Self> {
        Self::with_tokenizer(
            &path,
            crate::tokenizers::auto::Tokenizer::new(&path)?,
            config,
        )
    }
}

impl<T: Tokenizer> Generator<T> {
    /// Initializes the generator with the given tokenizer.
    ///
    /// # Arguments
    /// * `path` - A path to the directory containing the language model to be loaded.
    /// * `tokenizer` - An instance of the tokenizer.
    /// * `config` - A reference to a `Config` structure that specifies various settings
    ///   and configurations for the `Generator`.
    ///
    /// # Returns
    /// Returns a `Result` that, if successful, contains the initialized `Generator`. If an error
    /// occurs during initialization, the function will return an error wrapped in the `Result`.
    ///
    /// # Example
    /// The following example creates a generator instance with the tokenizer provided by
    /// [tokenizers](https://huggingface.co/docs/tokenizers).
    ///
    /// ```no_run
    /// # use anyhow::Result;
    /// use ct2rs::{Config, Generator};
    /// use ct2rs::tokenizers::hf::Tokenizer;
    ///
    /// # fn main() -> Result<()> {
    /// let generator = Generator::with_tokenizer(
    ///     "/path/to/model",
    ///     Tokenizer::from_file("/path/to/tokenizer.json")?,
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
        Ok(Generator {
            generator: sys::Generator::new(path, config)?,
            tokenizer,
        })
    }

    /// Generates texts following the provided batch of start strings.
    ///
    /// This function generates texts sequentially starting from the given `prompts`.
    /// The generation continues according to the
    /// options specified in `options`.
    ///
    /// An optional `callback` closure can be provided which is invoked for each new token
    /// generated during the translation process. This allows for step-by-step reception of the
    /// batch translation results. If the callback returns `Err`, it will stop the translation for
    /// that batch. Note that if a callback is provided, `options.beam_size` must be set to `1`.
    ///
    /// # Arguments
    /// * `prompts` - A vector of prompts. These prompts represent the initial state of the
    ///   generation process.
    /// * `options` - Settings applied to the generation process, such as beam size and other
    ///   generation-specific configurations.
    /// * `callback` - An optional mutable reference to a closure that is called for each token
    ///   generation step. The closure takes a `GenerationStepResult` and returns a
    ///   `anyhow::Result<()>`. If it returns `Err`, the translation process for the current batch
    ///   will stop.
    ///
    /// # Returns
    /// Returns a `Result` containing a vector of `GenerationResult` if successful, encapsulating
    /// the generated sequences for each input start token batch, or an error if the generation
    /// fails.
    pub fn generate_batch<U, V, W, E>(
        &self,
        prompts: &[U],
        options: &GenerationOptions<V, E, W>,
        callback: Option<&mut dyn FnMut(GenerationStepResult) -> Result<()>>,
    ) -> anyhow::Result<Vec<(Vec<String>, Vec<f32>)>>
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
            let output = self.generator.generate_batch(
                &encode_all(&self.tokenizer, prompts)?,
                options,
                Some(&mut wrapped_callback),
            )?;
            callback_result?;
            output
        } else {
            self.generator
                .generate_batch(&encode_all(&self.tokenizer, prompts)?, options, None)?
        };

        let mut res = Vec::new();
        for r in output.into_iter() {
            let sequence = r
                .sequences
                .into_iter()
                .map(|seq| self.tokenizer.decode(seq))
                .collect::<anyhow::Result<Vec<_>, _>>()
                .map_err(|err| anyhow!("failed to decode: {err}"))?;
            let scores = r.scores;
            res.push((sequence, scores))
        }
        Ok(res)
    }

    /// Scores a batch of tokens.
    ///
    /// # Arguments
    /// * `tokens` - Batch of strings to score.
    ///   If the model expects special start or end tokens, they should also be added to this input.
    /// * `options` - Settings applied to the scoring process.
    ///
    /// # Returns
    /// Returns a `Result` containing a vector of `ScoringResult` if successful,
    /// or an error if the generation fails.
    ///
    pub fn score_batch<U>(
        &self,
        prompts: &[U],
        options: &ScoringOptions,
    ) -> Result<Vec<ScoringResult>>
    where
        U: AsRef<str>,
    {
        self.generator
            .score_batch(&encode_all(&self.tokenizer, prompts)?, options)
    }

    /// Number of batches in the work queue.
    #[inline]
    pub fn num_queued_batches(&self) -> anyhow::Result<usize> {
        self.generator.num_queued_batches()
    }

    /// Number of batches in the work queue or currently processed by a worker.
    #[inline]
    pub fn num_active_batches(&self) -> anyhow::Result<usize> {
        self.generator.num_active_batches()
    }

    /// Number of parallel replicas.
    #[inline]
    pub fn num_replicas(&self) -> anyhow::Result<usize> {
        self.generator.num_replicas()
    }
}

impl<T: Tokenizer> Debug for Generator<T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.generator)
    }
}

#[cfg(test)]
#[cfg(feature = "hub")]
mod tests {
    use super::Generator;
    use crate::tokenizers::auto::Tokenizer;
    use crate::{download_model, Config, Device, GenerationOptions};
    use anyhow::Result;
    use std::path::PathBuf;

    const MODEL_ID: &str = "jkawamoto/gpt2-ct2";

    fn new_generator(model_path: &PathBuf) -> Result<Generator<Tokenizer>> {
        Generator::new(
            model_path,
            &Config {
                device: if cfg!(feature = "cuda") {
                    Device::CUDA
                } else {
                    Device::CPU
                },
                ..Default::default()
            },
        )
    }

    #[test]
    #[ignore]
    fn test_generate() {
        let model_path = download_model(MODEL_ID).unwrap();
        let g = new_generator(&model_path).unwrap();

        let prompt = "CTranslate2 is a library";
        let res = g
            .generate_batch(
                &[prompt],
                &GenerationOptions {
                    max_length: 32,
                    ..Default::default()
                },
                None,
            )
            .unwrap();

        assert!(res[0].0[0].starts_with(prompt));
    }

    #[test]
    #[ignore]
    fn test_scoring() {
        let model_path = download_model(MODEL_ID).unwrap();
        let g = new_generator(&model_path).unwrap();

        let prompt = "CTranslate2 is a library";
        let res = g.score_batch(&[prompt], &Default::default()).unwrap();

        assert_eq!(
            res[0].tokens,
            vec!["Trans", "late", "2", "Ġis", "Ġa", "Ġlibrary"]
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<String>>()
        );
        assert_ne!(res[0].normalized_score(), 0.0);
    }

    #[test]
    #[ignore]
    fn test_generator_debug() {
        let model_path = download_model(MODEL_ID).unwrap();
        let g = new_generator(&model_path).unwrap();

        assert!(format!("{:?}", g).contains(model_path.file_name().unwrap().to_str().unwrap()));
    }
}
