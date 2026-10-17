// generator.rs
//
// Copyright (c) 2023-2025 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides Rust bindings for the
//! [`ctranslate2::Generator`](https://opennmt.net/CTranslate2/python/ctranslate2.Generator.html).

use std::ffi::{OsStr, OsString};
use std::fmt::{Debug, Formatter};
use std::path::Path;

use anyhow::{anyhow, Error, Result};
use cxx::UniquePtr;

use super::{
    config, vec_ffi_vecstr, BatchType, Config, GenerationStepResult, ScoringOptions, ScoringResult,
    VecStr, VecString, VecUSize,
};

trait GenerationCallback {
    fn execute(&mut self, res: GenerationStepResult) -> bool;
}

impl<F: FnMut(GenerationStepResult) -> bool> GenerationCallback for F {
    fn execute(&mut self, args: GenerationStepResult) -> bool {
        self(args)
    }
}

type GenerationCallbackBox<'a> = Box<dyn GenerationCallback + 'a>;

impl<'a> From<Option<&'a mut dyn FnMut(GenerationStepResult) -> bool>>
    for GenerationCallbackBox<'a>
{
    fn from(opt: Option<&'a mut dyn FnMut(GenerationStepResult) -> bool>) -> Self {
        match opt {
            None => Box::new(|_| false) as GenerationCallbackBox,
            Some(c) => Box::new(c) as GenerationCallbackBox,
        }
    }
}

fn execute_generation_callback(f: &mut GenerationCallbackBox, arg: GenerationStepResult) -> bool {
    f.execute(arg)
}

#[cxx::bridge]
mod ffi {
    struct GenerationOptions<'a> {
        beam_size: usize,
        patience: f32,
        length_penalty: f32,
        repetition_penalty: f32,
        no_repeat_ngram_size: usize,
        disable_unk: bool,
        suppress_sequences: Vec<VecStr<'a>>,
        end_token: Vec<&'a str>,
        return_end_token: bool,
        max_length: usize,
        min_length: usize,
        sampling_topk: usize,
        sampling_topp: f32,
        sampling_temperature: f32,
        num_hypotheses: usize,
        return_scores: bool,
        return_logits_vocab: bool,
        return_alternatives: bool,
        min_alternative_expansion_prob: f32,
        static_prompt: Vec<&'a str>,
        cache_static_prompt: bool,
        include_prompt_in_result: bool,
        max_batch_size: usize,
        batch_type: BatchType,
    }

    struct GenerationResult {
        sequences: Vec<VecString>,
        sequences_ids: Vec<VecUSize>,
        scores: Vec<f32>,
    }

    extern "Rust" {
        type GenerationCallbackBox<'a>;
        fn execute_generation_callback(
            f: &mut GenerationCallbackBox,
            arg: GenerationStepResult,
        ) -> bool;
    }

    unsafe extern "C++" {
        include!("ct2rs/include/generator.h");
        include!("ct2rs/src/sys/types.rs.h");
        include!("ct2rs/src/sys/scoring.rs.h");

        type VecString = super::VecString;
        type VecStr<'a> = super::VecStr<'a>;
        type VecUSize = super::VecUSize;

        type Config = super::config::ffi::Config;
        type BatchType = super::BatchType;
        type GenerationStepResult = super::GenerationStepResult;

        type ScoringOptions = super::ScoringOptions;
        type ScoringResult = super::ScoringResult;

        type Generator;

        fn generator(model_path: &str, config: UniquePtr<Config>) -> Result<UniquePtr<Generator>>;

        fn generate_batch(
            self: &Generator,
            start_tokens: &Vec<VecStr>,
            options: &GenerationOptions,
            has_callback: bool,
            callback: &mut GenerationCallbackBox,
        ) -> Result<Vec<GenerationResult>>;

        fn score_batch(
            self: &Generator,
            tokens: &Vec<VecStr>,
            options: &ScoringOptions,
        ) -> Result<Vec<ScoringResult>>;

        fn num_queued_batches(self: &Generator) -> Result<usize>;

        fn num_active_batches(self: &Generator) -> Result<usize>;

        fn num_replicas(self: &Generator) -> Result<usize>;
    }
}

unsafe impl Send for ffi::Generator {}
unsafe impl Sync for ffi::Generator {}

/// A text generator.
///
/// This struct is a Rust binding to the
/// [`ctranslate2::Generator`](https://opennmt.net/CTranslate2/python/ctranslate2.Generator.html).
///
/// # Example
///
/// ```no_run
/// # use anyhow::Result;
/// use ct2rs::sys::{Config, Device, Generator, GenerationOptions};
///
/// # fn main() -> Result<()> {
/// let generator = Generator::new("/path/to/model", &Config::default())?;
/// let res = generator.generate_batch(
///     &[vec!["▁Hello", "▁world", "!", "</s>", "<unk>"]],
///     &GenerationOptions::default(),
///     None
/// )?;
/// for r in res {
///     println!("{:?}", r);
/// }
/// # Ok(())
/// # }
/// ```
pub struct Generator {
    model: OsString,
    ptr: UniquePtr<ffi::Generator>,
}

impl Generator {
    /// Creates and initializes an instance of `Generator`.
    ///
    /// This function constructs a new `Generator` by loading a language model from the specified
    /// `model_path` and applying the provided `config` settings.
    ///
    /// # Arguments
    /// * `model_path` - A path to the directory containing the language model to be loaded.
    /// * `config` - A reference to a `Config` structure that specifies various settings
    ///   and configurations for the `Generator`.
    ///
    /// # Returns
    /// Returns a `Result` that, if successful, contains the initialized `Generator`. If an error
    /// occurs during initialization, the function will return an error wrapped in the `Result`.
    ///
    /// # Example
    /// ```no_run
    /// # use anyhow::Result;
    /// #
    /// use ct2rs::sys::{Config, Generator};
    ///
    /// # fn main() -> Result<()> {
    /// let config = Config::default();
    /// let generator = Generator::new("/path/to/model", &config)?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn new<T: AsRef<Path>>(model_path: T, config: &Config) -> Result<Generator> {
        let model_path = model_path.as_ref();
        Ok(Generator {
            model: model_path
                .file_name()
                .map(OsStr::to_os_string)
                .unwrap_or_default(),
            ptr: ffi::generator(
                model_path
                    .to_str()
                    .ok_or_else(|| anyhow!("invalid path: {}", model_path.display()))?,
                config.to_ffi(),
            )?,
        })
    }

    /// Generates a sequence of tokens following the provided batch of start tokens.
    ///
    /// This function generates tokens sequentially starting from the given `start_tokens`.
    /// If the decoding process should start with a specific start token such as `<s>`,
    /// it needs to be included in the input. The generation continues according to the
    /// options specified in `options`.
    ///
    /// An optional `callback` can be provided, which is called with each token generated
    /// during the process. This callback allows for monitoring and reacting to the generation
    /// step-by-step. If the callback returns `true`, the generation process for the current batch
    /// will be stopped. It's important to note that if a callback is used, `options.beam_size`
    /// must be set to `1`.
    ///
    /// # Arguments
    /// * `start_tokens` - A vector of vectors containing start tokens for each sequence in the
    ///   batch. These tokens represent the initial state of the generation process.
    /// * `options` - Settings applied to the generation process, such as beam size and other
    ///   generation-specific configurations.
    /// * `callback` - An optional mutable reference to a closure that is invoked for each
    ///   generation step. The closure takes a `GenerationStepResult` and returns a `bool`.
    ///   Returning `true` will stop the generation for that batch.
    ///
    /// # Returns
    /// Returns a `Result` containing a vector of `GenerationResult` if successful, encapsulating
    /// the generated sequences for each input start token batch, or an error if the generation fails.
    ///
    /// # Example
    /// ```no_run
    /// # use anyhow::Result;
    /// #
    /// use ct2rs::sys::{Config, Generator, GenerationOptions, GenerationStepResult};
    ///
    /// # fn main() -> Result<()> {
    /// let start_tokens = [vec!["<s>".to_string()]];
    /// let options = GenerationOptions::default();
    /// let mut callback = |step_result: GenerationStepResult| -> bool {
    ///     println!("{:?}", step_result);
    ///     false // Continue processing
    /// };
    /// let generator = Generator::new("/path/to/model", &Config::default())?;
    /// let results = generator.generate_batch(&start_tokens, &options, Some(&mut callback))?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn generate_batch<T: AsRef<str>, U: AsRef<str>, V: AsRef<str>, W: AsRef<str>>(
        &self,
        start_tokens: &[Vec<T>],
        options: &GenerationOptions<U, V, W>,
        callback: Option<&mut dyn FnMut(GenerationStepResult) -> bool>,
    ) -> Result<Vec<GenerationResult>> {
        Ok(self
            .ptr
            .generate_batch(
                &vec_ffi_vecstr(start_tokens),
                &options.to_ffi(),
                callback.is_some(),
                &mut GenerationCallbackBox::from(callback),
            )?
            .into_iter()
            .map(GenerationResult::from)
            .collect())
    }

    /// Scores a batch of tokens.
    ///
    /// # Arguments
    /// * `tokens` - Batch of tokens to score.
    ///   If the model expects special start or end tokens, they should also be added to this input.
    /// * `options` - Settings applied to the scoring process.
    ///
    /// # Returns
    /// Returns a `Result` containing a vector of `ScoringResult` if successful,
    /// or an error if the generation fails.
    pub fn score_batch<T: AsRef<str>>(
        &self,
        tokens: &[Vec<T>],
        options: &ScoringOptions,
    ) -> Result<Vec<ScoringResult>> {
        self.ptr
            .score_batch(&vec_ffi_vecstr(tokens), options)
            .map_err(Error::from)
    }

    /// Number of batches in the work queue.
    #[inline]
    pub fn num_queued_batches(&self) -> Result<usize> {
        self.ptr.num_queued_batches().map_err(Error::from)
    }

    /// Number of batches in the work queue or currently processed by a worker.
    #[inline]
    pub fn num_active_batches(&self) -> Result<usize> {
        self.ptr.num_active_batches().map_err(Error::from)
    }

    /// Number of parallel replicas.
    #[inline]
    pub fn num_replicas(&self) -> Result<usize> {
        self.ptr.num_replicas().map_err(Error::from)
    }
}

impl Debug for Generator {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Generator")
            .field("model", &self.model)
            .field("queued_batches", &self.num_queued_batches())
            .field("active_batches", &self.num_active_batches())
            .field("replicas", &self.num_replicas())
            .finish()
    }
}

// Releasing `UniquePtr<Generator>` invokes joining threads.
// However, on Windows, this causes a deadlock.
// As a workaround, it is bypassed here.
// See also https://github.com/jkawamoto/ctranslate2-rs/issues/64
#[cfg(target_os = "windows")]
impl Drop for Generator {
    fn drop(&mut self) {
        let ptr = std::mem::replace(&mut self.ptr, UniquePtr::null());
        unsafe {
            std::ptr::drop_in_place(ptr.into_raw());
        }
    }
}

/// The set of generation options.
///
/// # Examples
///
/// Example of creating a default `GenerationOptions`:
///
/// ```
/// use ct2rs::sys::GenerationOptions;
///
/// let options = GenerationOptions::default();
/// # assert_eq!(options.beam_size, 1);
/// # assert_eq!(options.patience, 1.);
/// # assert_eq!(options.length_penalty, 1.);
/// # assert_eq!(options.repetition_penalty, 1.);
/// # assert_eq!(options.no_repeat_ngram_size, 0);
/// # assert!(!options.disable_unk);
/// # assert!(options.suppress_sequences.is_empty());
/// # assert!(options.end_token.is_empty());
/// # assert!(!options.return_end_token);
/// # assert_eq!(options.max_length, 512);
/// # assert_eq!(options.min_length, 0);
/// # assert_eq!(options.sampling_topk, 1);
/// # assert_eq!(options.sampling_topp, 1.);
/// # assert_eq!(options.sampling_temperature, 1.);
/// # assert_eq!(options.num_hypotheses, 1);
/// # assert!(!options.return_scores);
/// # assert!(!options.return_logits_vocab);
/// # assert!(!options.return_alternatives);
/// # assert_eq!(options.min_alternative_expansion_prob, 0.);
/// # assert!(options.static_prompt.is_empty());
/// # assert!(options.cache_static_prompt);
/// # assert!(options.include_prompt_in_result);
/// # assert_eq!(options.max_batch_size, 0);
/// # assert_eq!(options.batch_type, Default::default());
/// ```
///
#[derive(Clone, Debug)]
pub struct GenerationOptions<T: AsRef<str>, U: AsRef<str>, V: AsRef<str>> {
    /// Beam size to use for beam search (set 1 to run greedy search). (default: 1)
    pub beam_size: usize,
    /// Beam search patience factor, as described in <https://arxiv.org/abs/2204.05424>.
    /// The decoding will continue until beam_size*patience hypotheses are finished. (default: 1.0)
    pub patience: f32,
    /// Exponential penalty applied to the length during beam search.
    /// The scores are normalized with:
    /// ```math
    /// hypothesis_score /= (hypothesis_length ** length_penalty)
    /// ```
    /// (default: 1.0)
    pub length_penalty: f32,
    /// Penalty applied to the score of previously generated tokens, as described in
    /// <https://arxiv.org/abs/1909.05858> (set > 1 to penalize). (default: 1.0)
    pub repetition_penalty: f32,
    /// Prevent repetitions of ngrams with this size (set 0 to disable). (default: 0)
    pub no_repeat_ngram_size: usize,
    /// Disable the generation of the unknown token. (default: false)
    pub disable_unk: bool,
    /// Disable the generation of some sequences of tokens. (default: empty)
    pub suppress_sequences: Vec<Vec<T>>,
    /// Stop the decoding on one of these tokens (defaults to the model EOS token).
    pub end_token: Vec<U>,
    /// Include the end token in the result. (default: false)
    pub return_end_token: bool,
    /// Length constraints. (default: 512)
    pub max_length: usize,
    /// Length constraints. (default: 0)
    pub min_length: usize,
    /// Randomly sample from the top K candidates (set 0 to sample from the full output
    /// distribution). (default: 1)
    pub sampling_topk: usize,
    /// Keep the most probable tokens whose cumulative probability exceeds this value.
    /// (default: 1.0)
    pub sampling_topp: f32,
    /// High temperature increase randomness. (default: 1.0)
    pub sampling_temperature: f32,
    /// Number of hypotheses to include in the result. (default: 1.0)
    pub num_hypotheses: usize,
    /// Include scores in the result. (default: false)
    pub return_scores: bool,
    /// Include log probs of each token in the result. (default: false)
    pub return_logits_vocab: bool,
    /// Return alternatives at the first unconstrained decoding position. This is typically
    /// used with a prefix to provide alternatives at a specific location. (default: false)
    pub return_alternatives: bool,
    /// Minimum probability to expand an alternative. (default: 0)
    pub min_alternative_expansion_prob: f32,
    /// The static prompt will prefix all inputs for this model. (default: empty)
    pub static_prompt: Vec<V>,
    /// Cache the model state after the static prompt and reuse it for future runs using
    /// the same static prompt. (default: true)
    pub cache_static_prompt: bool,
    /// Include the input tokens in the generation result. (default: true)
    pub include_prompt_in_result: bool,
    /// The maximum batch size. If the number of inputs is greater than `max_batch_size`,
    /// the inputs are sorted by length and split by chunks of `max_batch_size` examples
    /// so that the number of padding positions is minimized. (default: 0)
    pub max_batch_size: usize,
    /// Whether `max_batch_size` is the number of `examples` or `tokens`.
    pub batch_type: BatchType,
}

impl Default for GenerationOptions<String, String, String> {
    fn default() -> Self {
        Self {
            beam_size: 1,
            patience: 1.,
            length_penalty: 1.,
            repetition_penalty: 1.,
            no_repeat_ngram_size: 0,
            disable_unk: false,
            suppress_sequences: vec![],
            end_token: vec![],
            return_end_token: false,
            max_length: 512,
            min_length: 0,
            sampling_topk: 1,
            sampling_topp: 1.,
            sampling_temperature: 1.,
            num_hypotheses: 1,
            return_scores: false,
            return_logits_vocab: false,
            return_alternatives: false,
            min_alternative_expansion_prob: 0.,
            static_prompt: vec![],
            cache_static_prompt: true,
            include_prompt_in_result: true,
            max_batch_size: 0,
            batch_type: Default::default(),
        }
    }
}

impl<T: AsRef<str>, U: AsRef<str>, V: AsRef<str>> GenerationOptions<T, U, V> {
    #[inline]
    fn to_ffi(&self) -> ffi::GenerationOptions<'_> {
        ffi::GenerationOptions {
            beam_size: self.beam_size,
            patience: self.patience,
            length_penalty: self.length_penalty,
            repetition_penalty: self.repetition_penalty,
            no_repeat_ngram_size: self.no_repeat_ngram_size,
            disable_unk: self.disable_unk,
            suppress_sequences: vec_ffi_vecstr(self.suppress_sequences.as_ref()),
            end_token: self.end_token.iter().map(AsRef::as_ref).collect(),
            return_end_token: self.return_end_token,
            max_length: self.max_length,
            min_length: self.min_length,
            sampling_topk: self.sampling_topk,
            sampling_topp: self.sampling_topp,
            sampling_temperature: self.sampling_temperature,
            num_hypotheses: self.num_hypotheses,
            return_scores: self.return_scores,
            return_logits_vocab: self.return_logits_vocab,
            return_alternatives: self.return_alternatives,
            min_alternative_expansion_prob: self.min_alternative_expansion_prob,
            static_prompt: self.static_prompt.iter().map(AsRef::as_ref).collect(),
            cache_static_prompt: self.cache_static_prompt,
            include_prompt_in_result: self.include_prompt_in_result,
            max_batch_size: self.max_batch_size,
            batch_type: self.batch_type,
        }
    }
}

/// A generation result.
///
/// This struct is a Rust binding to the
/// [`ctranslate2.GenerationResult`](https://opennmt.net/CTranslate2/python/ctranslate2.GenerationResult.html).
#[derive(Clone, Debug)]
pub struct GenerationResult {
    /// Generated sequences of tokens.
    pub sequences: Vec<Vec<String>>,
    /// Generated sequences of token IDs.
    pub sequences_ids: Vec<Vec<usize>>,
    /// Score of each sequence (empty if `return_scores` was disabled).
    pub scores: Vec<f32>,
}

impl From<ffi::GenerationResult> for GenerationResult {
    fn from(res: ffi::GenerationResult) -> Self {
        Self {
            sequences: res.sequences.into_iter().map(Vec::<String>::from).collect(),
            sequences_ids: res
                .sequences_ids
                .into_iter()
                .map(Vec::<usize>::from)
                .collect(),
            scores: res.scores,
        }
    }
}

impl GenerationResult {
    /// Returns the number of sequences.
    #[inline]
    pub fn num_sequences(&self) -> usize {
        self.sequences.len()
    }

    /// Returns true if this result has scores.
    #[inline]
    pub fn has_scores(&self) -> bool {
        !self.scores.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::ffi::{VecStr, VecString, VecUSize};
    use super::{ffi, GenerationOptions, GenerationResult};

    #[test]
    fn options_to_ffi() {
        let opts = GenerationOptions {
            suppress_sequences: vec![vec!["x".to_string(), "y".to_string(), "z".to_string()]],
            end_token: vec!["1".to_string(), "2".to_string()],
            static_prompt: vec!["one".to_string(), "two".to_string()],
            ..Default::default()
        };
        let res = opts.to_ffi();

        assert_eq!(res.beam_size, opts.beam_size);
        assert_eq!(res.patience, opts.patience);
        assert_eq!(res.length_penalty, opts.length_penalty);
        assert_eq!(res.repetition_penalty, opts.repetition_penalty);
        assert_eq!(res.no_repeat_ngram_size, opts.no_repeat_ngram_size);
        assert_eq!(res.disable_unk, opts.disable_unk);
        assert_eq!(
            res.suppress_sequences,
            opts.suppress_sequences
                .iter()
                .map(|v| VecStr {
                    v: v.iter().map(AsRef::as_ref).collect()
                })
                .collect::<Vec<VecStr>>()
        );
        assert_eq!(
            res.end_token,
            opts.end_token
                .iter()
                .map(AsRef::as_ref)
                .collect::<Vec<&str>>()
        );
        assert_eq!(res.return_end_token, opts.return_end_token);
        assert_eq!(res.max_length, opts.max_length);
        assert_eq!(res.min_length, opts.min_length);
        assert_eq!(res.sampling_topk, opts.sampling_topk);
        assert_eq!(res.sampling_topp, opts.sampling_topp);
        assert_eq!(res.sampling_temperature, opts.sampling_temperature);
        assert_eq!(res.num_hypotheses, opts.num_hypotheses);
        assert_eq!(res.return_scores, opts.return_scores);
        assert_eq!(res.return_alternatives, opts.return_alternatives);
        assert_eq!(
            res.min_alternative_expansion_prob,
            opts.min_alternative_expansion_prob
        );
        assert_eq!(
            res.static_prompt,
            opts.static_prompt
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<&str>>()
        );
        assert_eq!(res.cache_static_prompt, opts.cache_static_prompt);
        assert_eq!(res.include_prompt_in_result, opts.include_prompt_in_result);
        assert_eq!(res.max_batch_size, opts.max_batch_size);
        assert_eq!(res.batch_type, opts.batch_type);
    }

    #[test]
    fn generation_result() {
        let sequences = vec![
            vec!["a".to_string(), "b".to_string()],
            vec!["x".to_string(), "y".to_string(), "z".to_string()],
        ];
        let sequences_ids: Vec<Vec<usize>> = vec![vec![1, 2], vec![10, 20, 30]];
        let scores: Vec<f32> = vec![1., 2., 3.];
        let res: GenerationResult = ffi::GenerationResult {
            sequences: sequences
                .iter()
                .map(|v| VecString::from(v.clone()))
                .collect(),
            sequences_ids: sequences_ids
                .iter()
                .map(|v| VecUSize::from(v.clone()))
                .collect(),
            scores: scores.clone(),
        }
        .into();

        assert_eq!(res.sequences, sequences);
        assert_eq!(res.sequences_ids, sequences_ids);
        assert_eq!(res.scores, scores);
        assert_eq!(res.num_sequences(), sequences.len());
        assert!(res.has_scores());
    }

    #[test]
    fn generation_empty_result() {
        let res: GenerationResult = ffi::GenerationResult {
            sequences: vec![],
            sequences_ids: vec![],
            scores: vec![],
        }
        .into();

        assert!(res.sequences.is_empty());
        assert!(res.sequences_ids.is_empty());
        assert!(res.scores.is_empty());
        assert_eq!(res.num_sequences(), 0);
        assert!(!res.has_scores());
    }

    #[cfg(feature = "hub")]
    mod hub {
        use crate::sys::Generator;
        use crate::{download_model, Config, Device};

        const MODEL_ID: &str = "jkawamoto/gpt2-ct2";
        #[test]
        #[ignore]
        fn test_generator_debug() {
            let model_path = download_model(MODEL_ID).unwrap();

            let generator = Generator::new(
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
            assert!(format!("{:?}", generator)
                .contains(model_path.file_name().unwrap().to_str().unwrap()));
        }
    }
}
