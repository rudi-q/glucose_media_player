// whisper.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides a speach transcriber.

use std::fmt::{Debug, Formatter};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{anyhow, Result};
use mel_spec::mel::{log_mel_spectrogram, mel, norm_mel};
use mel_spec::stft::Spectrogram;
use ndarray::{s, stack, Array2, Array3, Axis};
use serde::{Deserialize, Serialize};

pub use super::sys::WhisperOptions;
use super::tokenizers::hf;
use super::{sys, Config, Tokenizer};

/// Represents a transcribed word with detailed timing and probability.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Word {
    /// The transcribed word text.
    pub word: String,
    /// Start time in seconds relative to the audio start.
    pub start: f32,
    /// End time in seconds relative to the audio start.
    pub end: f32,
    /// Confidence probability score bounded between 0.0 and 1.0.
    pub probability: f32,
}

/// Represents a transcribed audio segment with start/end timestamps and word-level information.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Segment {
    /// Segment ID.
    pub id: usize,
    /// Text content of the segment.
    pub text: String,
    /// Start time in seconds relative to the audio start.
    pub start: f32,
    /// End time in seconds relative to the audio start.
    pub end: f32,
    /// Word-level alignment information.
    pub words: Option<Vec<Word>>,
}

const PREPROCESSOR_CONFIG_FILE: &str = "preprocessor_config.json";

/// A speach transcriber using the Whisper speech recognition model published by OpenAI.
///
/// # Example
/// ```no_run
/// use ct2rs::Whisper;
///
/// # fn main() -> anyhow::Result<()>{
/// let whisper = Whisper::new("/path/to/model", Default::default())?;
///
/// let sampling_rate = whisper.sampling_rate();
/// // Sample the source audio at the sampling rates shown above.
/// // Each sample must be normalized to the range [-1, 1].
/// let samples = vec![];
///
/// let res = whisper.generate(&samples, None, false, &Default::default())?;
/// for r in res {
///     println!("{}", r);
/// }
/// # Ok(())
/// # }
/// ```
pub struct Whisper {
    whisper: sys::Whisper,
    tokenizer: hf::Tokenizer,
    config: PreprocessorConfig,
}

impl Whisper {
    /// Initializes the transcriber.
    ///
    /// # Arguments
    /// * `path` - A path to the directory containing the language model to be loaded.
    /// * `config` - A [`Config`] structure that specifies various settings
    ///   and configurations for the `Whisper`.
    ///
    /// # Returns
    /// Returns a `Result` that, if successful, contains the initialized `Whisper`. If an error
    /// occurs during initialization, the function will return an error wrapped in the `Result`.
    pub fn new<T: AsRef<Path>>(model_path: T, config: Config) -> Result<Self> {
        Ok(Self {
            whisper: sys::Whisper::new(&model_path, config)?,
            tokenizer: hf::Tokenizer::new(&model_path)?,
            config: PreprocessorConfig::read(model_path.as_ref().join(PREPROCESSOR_CONFIG_FILE))?,
        })
    }

    /// Transcribe the given samples.
    ///
    /// # Arguments
    /// * `samples` - Samples of the source audio. They must be sampled at the sampling rate
    ///   returned by [`sampling_rate`][Whisper::sampling_rate] method and normalized to the range
    ///   `[-1, 1]`. If the samples are longer than the maximum number of samples returned by
    ///   [`n_samples`][Whisper::n_samples] method, they will be processed in segments.
    /// * `language` - An optional language setting. It transcribes assuming the specified language.
    ///   If `None`, it uses Whisper's language detection.
    /// * `timestamp` - If `true`, the output will include timestamps.
    /// * `options` - Settings.
    ///
    /// # Returns
    /// Returns a `Result` containing a vector of transcribed strings if successful,
    /// or an error if the translation fails.
    pub fn generate(
        &self,
        samples: &[f32],
        language: Option<&str>,
        timestamp: bool,
        options: &WhisperOptions,
    ) -> Result<Vec<String>> {
        let (mut mel_spectrogram, num_chunks) = self.generate_mel_spectrogram(samples)?;
        if num_chunks == 0 {
            return Ok(Vec::new());
        }

        let shape = mel_spectrogram.shape().to_vec();
        let storage_view = sys::StorageView::new(
            &shape,
            mel_spectrogram.as_slice_mut().unwrap(),
            Default::default(),
        )?;

        let lang_token = self.detect_language_token(&storage_view, language)?;

        let prompt = self.generate_prompt(&lang_token, timestamp);

        self.whisper
            .generate(&storage_view, &vec![prompt; num_chunks], options)?
            .into_iter()
            .map(|res| {
                let r = res
                    .sequences
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("failed to transcribe samples"))?;
                self.tokenizer.decode(r)
            })
            .collect()
    }

    /// Generate transcription segments for the given samples with detailed word-level and segment-level timelines.
    ///
    /// # Arguments
    /// * `samples` - Samples of the source audio. They must be sampled at the sampling rate
    ///   returned by [`sampling_rate`][Whisper::sampling_rate] method and normalized to the range
    ///   `[-1, 1]`. If the samples are longer than the maximum number of samples returned by
    ///   [`n_samples`][Whisper::n_samples] method, they will be processed in segments.
    /// * `language` - An optional language setting. It generates segments assuming the specified language.
    ///   If `None`, it uses Whisper's language detection.
    /// * `options` - Settings.
    ///
    /// # Returns
    /// Returns a `Result` containing a vector of transcribed `Segment`s if successful,
    /// or an error if the segment generation fails.
    pub fn generate_segments(
        &self,
        samples: &[f32],
        language: Option<&str>,
        options: &WhisperOptions,
    ) -> Result<Vec<Segment>> {
        let (mut mel_spectrogram, num_chunks) = self.generate_mel_spectrogram(samples)?;
        if num_chunks == 0 {
            return Ok(Vec::new());
        }

        let shape = mel_spectrogram.shape().to_vec();
        let storage_view = sys::StorageView::new(
            &shape,
            mel_spectrogram.as_slice_mut().unwrap(),
            Default::default(),
        )?;

        let lang_token = self.detect_language_token(&storage_view, language)?;

        // Pass features through the encoder network to get encoder outputs
        let encoder_output = self.whisper.encode(&storage_view, false)?;

        let prompt = self.generate_prompt(&lang_token, true);

        // For alignment and timing, we do want timestamps
        let gen_results =
            self.whisper
                .generate(&encoder_output, &vec![prompt.clone(); num_chunks], options)?;

        // Build start sequence token IDs for alignment FFI
        let start_seq: Vec<usize> = prompt
            .iter()
            .map(|t| {
                self.tokenizer
                    .token_to_id(t)
                    .map(|id| id as usize)
                    .unwrap_or(0)
            })
            .collect();

        let num_frames = vec![self.config.nb_max_frames; num_chunks];
        let text_tokens: Vec<Vec<usize>> = gen_results
            .iter()
            .map(|res| res.sequences_ids[0].clone())
            .collect();

        // Run DTW alignments on the encoder output cross-attention weights
        let alignment_results = self.whisper.align(
            &encoder_output,
            &start_seq,
            &text_tokens,
            &num_frames,
            7, // median filter width
        )?;

        let mut segments = Vec::new();

        for (chunk_idx, (res, align_res)) in
            gen_results.iter().zip(alignment_results.iter()).enumerate()
        {
            let tokens = &res.sequences[0];
            let alignments = &align_res.alignments;
            let text_token_probs = &align_res.text_token_probs;

            let word_token_ranges = group_tokens_into_words(tokens);
            let chunk_words = process_word_timings(
                &word_token_ranges,
                alignments,
                text_token_probs,
                tokens.len(),
            );

            let chunk_offset =
                (chunk_idx * self.config.n_samples) as f32 / self.config.sampling_rate as f32;

            let mut final_words = Vec::new();
            for (range, mut word) in word_token_ranges.into_iter().zip(chunk_words.into_iter()) {
                let word_text = self.tokenizer.decode(tokens[range.clone()].to_vec())?;
                let clean_word_text = word_text.trim().to_string();
                if clean_word_text.is_empty() {
                    continue;
                }
                word.word = clean_word_text;
                word.start += chunk_offset;
                word.end += chunk_offset;
                final_words.push(word);
            }

            let clean_tokens: Vec<String> = tokens
                .iter()
                .filter(|t| !is_special_token(t))
                .cloned()
                .collect();
            let chunk_text = self.tokenizer.decode(clean_tokens)?.trim().to_string();

            let seg_start = final_words.first().map(|w| w.start).unwrap_or(chunk_offset);
            let seg_end = final_words.last().map(|w| w.end).unwrap_or(chunk_offset);

            segments.push(Segment {
                id: chunk_idx,
                text: chunk_text,
                start: seg_start,
                end: seg_end,
                words: Some(final_words),
            });
        }

        Ok(segments)
    }

    /// Returns the expected sampling rate.
    pub fn sampling_rate(&self) -> usize {
        self.config.sampling_rate
    }

    /// Max number of samples per batch.
    pub fn n_samples(&self) -> usize {
        self.config.n_samples
    }

    /// Returns `true` if this model is multilingual.
    #[inline]
    pub fn is_multilingual(&self) -> bool {
        self.whisper.is_multilingual()
    }

    /// Returns the number of languages supported.
    #[inline]
    pub fn num_languages(&self) -> usize {
        self.whisper.num_languages()
    }

    /// Number of batches in the work queue.
    #[inline]
    pub fn num_queued_batches(&self) -> usize {
        self.whisper.num_queued_batches()
    }

    /// Number of batches in the work queue or currently processed by a worker.
    #[inline]
    pub fn num_active_batches(&self) -> usize {
        self.whisper.num_active_batches()
    }

    /// Number of parallel replicas.
    #[inline]
    pub fn num_replicas(&self) -> usize {
        self.whisper.num_replicas()
    }

    /// Generates a log-mel spectrogram for the given audio samples.
    ///
    /// It partitions the samples into chunks and extracts log-mel features
    /// for each chunk.
    ///
    /// # Returns
    /// A tuple containing:
    /// - An `Array3<f32>` with the stacked log-mel spectrogram.
    /// - The number of chunks processed.
    fn generate_mel_spectrogram(&self, samples: &[f32]) -> Result<(Array3<f32>, usize)> {
        let mut stft = Spectrogram::new(self.config.n_fft, self.config.hop_length);

        let mut mel_spectrogram_vec = vec![];
        for chunk in samples.chunks(self.config.n_samples) {
            let mut mel_spectrogram_per_chunk =
                Array2::zeros((self.config.feature_size, self.config.nb_max_frames));
            for (i, flame) in chunk.chunks(self.config.hop_length).enumerate() {
                if let Some(fft_frame) = stft.add(flame) {
                    let mel = norm_mel(&log_mel_spectrogram(&fft_frame, &self.config.mel_filters))
                        .mapv(|v| v as f32);
                    mel_spectrogram_per_chunk
                        .slice_mut(s![.., i])
                        .assign(&mel.slice(s![.., 0]));
                }
            }
            mel_spectrogram_vec.push(mel_spectrogram_per_chunk);
        }

        let num_chunks = mel_spectrogram_vec.len();
        if num_chunks == 0 {
            return Ok((Array3::zeros((0, 0, 0)), 0));
        }

        let mut mel_spectrogram = stack(
            Axis(0),
            &mel_spectrogram_vec
                .iter()
                .map(|a| a.view())
                .collect::<Vec<_>>(),
        )?;
        if !mel_spectrogram.is_standard_layout() {
            mel_spectrogram = mel_spectrogram.as_standard_layout().into_owned();
        }

        Ok((mel_spectrogram, num_chunks))
    }

    /// Detects or formats the language token.
    ///
    /// If `language` is specified, it returns the formatted token (e.g. `<|en|>`).
    /// Otherwise, it runs the language detector on the given storage view.
    fn detect_language_token(
        &self,
        storage_view: &sys::StorageView,
        language: Option<&str>,
    ) -> Result<String> {
        let lang_token = match language {
            Some(lang) => {
                format!("<|{}|>", lang)
            }
            None => {
                let detection_result = self.whisper.detect_language(storage_view)?;
                detection_result
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("failed to detect language"))?
                    .into_iter()
                    .next()
                    .ok_or_else(|| anyhow!("failed to detect language"))?
                    .language
            }
        };
        Ok(lang_token)
    }

    /// Generates the transcript prompt tokens.
    ///
    /// # Arguments
    /// * `lang_token` - The language token (e.g. `<|en|>`).
    /// * `timestamp` - If `true`, timestamps will be generated. Otherwise, adds `"<|notimestamps|>"`.
    fn generate_prompt<'a>(&self, lang_token: &'a str, timestamp: bool) -> Vec<&'a str> {
        let mut prompt = vec!["<|startoftranscript|>", lang_token, "<|transcribe|>"];
        if !timestamp {
            prompt.push("<|notimestamps|>");
        }
        prompt
    }
}

fn starts_new_word(token: &str) -> bool {
    // If the token starts with 'Ġ' (GPT-2/Whisper space representation)
    if token.starts_with('Ġ') {
        return true;
    }
    // If the token starts with ' ' (SentencePiece space representation)
    if token.starts_with(' ') {
        return true;
    }
    // If the token starts with a regular space
    if token.starts_with(' ') {
        return true;
    }
    // If the token is a punctuation/special character (excluding letters and digits)
    if let Some(first_char) = token.chars().next() {
        if first_char.is_ascii_punctuation() {
            return true;
        }
    }
    false
}

fn is_special_token(token: &str) -> bool {
    token.starts_with("<|") && token.ends_with("|>")
}

fn group_tokens_into_words(tokens: &[String]) -> Vec<std::ops::Range<usize>> {
    let mut word_ranges = Vec::new();
    let mut current_word_start = None;

    for (i, token) in tokens.iter().enumerate() {
        if is_special_token(token) {
            if let Some(start) = current_word_start {
                word_ranges.push(start..i);
                current_word_start = None;
            }
            continue;
        }

        if current_word_start.is_none() {
            current_word_start = Some(i);
        } else if starts_new_word(token) {
            if let Some(start) = current_word_start {
                word_ranges.push(start..i);
            }
            current_word_start = Some(i);
        }
    }

    if let Some(start) = current_word_start {
        word_ranges.push(start..tokens.len());
    }

    word_ranges
}

fn process_word_timings(
    word_token_ranges: &[std::ops::Range<usize>],
    alignments: &[sys::WhisperTokenAlignment],
    text_token_probs: &[f32],
    num_tokens: usize,
) -> Vec<Word> {
    if num_tokens == 0 || word_token_ranges.is_empty() {
        return Vec::new();
    }

    let mut token_start_frames = vec![-1i64; num_tokens];
    let mut token_end_frames = vec![-1i64; num_tokens];

    // First pass: extract from alignments
    for m in 0..num_tokens {
        let aligned_frames: Vec<i64> = alignments
            .iter()
            .filter(|a| a.token_x == m as i64)
            .map(|a| a.frame_x)
            .collect();
        if !aligned_frames.is_empty() {
            token_start_frames[m] = *aligned_frames.iter().min().unwrap();
            token_end_frames[m] = *aligned_frames.iter().max().unwrap() + 1;
        }
    }

    // Second pass: fill in missing/empty and enforce monotonicity
    let mut last_end = 0;
    for m in 0..num_tokens {
        if token_start_frames[m] == -1 {
            token_start_frames[m] = last_end;
            token_end_frames[m] = last_end;
        } else {
            if token_start_frames[m] < last_end {
                token_start_frames[m] = last_end;
            }
            if token_end_frames[m] < token_start_frames[m] {
                token_end_frames[m] = token_start_frames[m];
            }
        }
        last_end = token_end_frames[m];
    }

    // Convert token frames to words
    let mut words = Vec::new();
    for range in word_token_ranges {
        let u = range.start;
        let v = range.end - 1;

        let word_start_frame = token_start_frames[u];
        let word_end_frame = token_end_frames[v];

        // 50.0 is the downsampled temporal resolution constant of Whisper encoder output (1 frame = 20ms)
        let word_start_sec = word_start_frame as f32 / 50.0;
        let word_end_sec = word_end_frame as f32 / 50.0;

        // Confidence probability score as the arithmetic mean of token probabilities
        let sum_prob: f32 = text_token_probs[u..=v].iter().sum();
        let word_prob = sum_prob / (v - u + 1) as f32;

        words.push(Word {
            word: "".to_string(), // Text will be filled later by the caller
            start: word_start_sec,
            end: word_end_sec,
            probability: word_prob,
        });
    }

    // Apply Heuristic 2: Median-Based Duration Capping
    let mut durations: Vec<f32> = words
        .iter()
        .map(|w| w.end - w.start)
        .filter(|&d| d > 0.0)
        .collect();
    if !durations.is_empty() {
        durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median_idx = durations.len() / 2;
        let mut d_median = durations[median_idx];
        if d_median > 0.7 {
            d_median = 0.7;
        }
        let d_max = 2.0 * d_median;
        for w in &mut words {
            let duration = w.end - w.start;
            if duration > d_max {
                w.end = w.start + d_max;
            }
        }
    }

    words
}

#[cfg(test)]
mod tests_grouping {
    use super::*;

    #[test]
    fn test_starts_new_word() {
        assert!(starts_new_word("ĠHello"));
        assert!(starts_new_word(" world"));
        assert!(starts_new_word(" "));
        assert!(starts_new_word("!"));
        assert!(!starts_new_word("llo"));
    }

    #[test]
    fn test_group_tokens_into_words() {
        let tokens = vec![
            "<|startoftranscript|>".to_string(),
            "<|en|>".to_string(),
            "<|transcribe|>".to_string(),
            "ĠHello".to_string(),
            "llo".to_string(),
            "Ġworld".to_string(),
            "!".to_string(),
        ];
        let ranges = group_tokens_into_words(&tokens);
        assert_eq!(ranges, vec![3..5, 5..6, 6..7]);
    }

    #[test]
    fn test_process_word_timings() {
        use crate::sys::WhisperTokenAlignment;

        let word_token_ranges = vec![0..2, 2..3]; // Two words: token 0..2, token 2..3
                                                  // Token 0 aligned to frames 10..15, Token 1 has no alignments, Token 2 aligned to frame 20..22
        let alignments = vec![
            WhisperTokenAlignment {
                token_x: 0,
                frame_x: 10,
            },
            WhisperTokenAlignment {
                token_x: 0,
                frame_x: 14,
            },
            WhisperTokenAlignment {
                token_x: 2,
                frame_x: 20,
            },
            WhisperTokenAlignment {
                token_x: 2,
                frame_x: 21,
            },
        ];
        let text_token_probs = vec![0.9, 0.8, 0.95];

        let words = process_word_timings(&word_token_ranges, &alignments, &text_token_probs, 3);
        assert_eq!(words.len(), 2);

        // Word 0 (tokens 0..1):
        // Token 0: starts at 10, ends at 15
        // Token 1: starts at last_end (15), ends at last_end (15)
        // Word 0: starts at 10 (0.2s), ends at 15 (0.3s)
        assert_eq!(words[0].start, 0.2);
        assert_eq!(words[0].end, 0.3);
        // Average probability: (0.9 + 0.8) / 2 = 0.85
        assert_eq!(words[0].probability, 0.85);

        // Word 1 (token 2):
        // Token 2: starts at 20, ends at 22
        // Word 1: starts at 20 (0.4s), ends at 22 (0.44s)
        assert_eq!(words[1].start, 0.4);
        assert_eq!(words[1].end, 0.44);
        assert_eq!(words[1].probability, 0.95);
    }
}

impl Debug for Whisper {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.whisper)
    }
}

#[derive(Debug)]
#[allow(dead_code)]
struct PreprocessorConfig {
    chunk_length: usize,
    feature_extractor_type: String,
    feature_size: usize,
    hop_length: usize,
    n_fft: usize,
    n_samples: usize,
    nb_max_frames: usize,
    padding_side: String,
    padding_value: f32,
    processor_class: String,
    return_attention_mask: bool,
    sampling_rate: usize,
    mel_filters: Array2<f64>,
}

impl PreprocessorConfig {
    fn read<T: AsRef<Path>>(path: T) -> Result<Self> {
        let file = File::open(path)?;
        let reader = BufReader::new(file);

        #[derive(Deserialize)]
        struct PreprocessorConfigAux {
            chunk_length: usize,
            feature_extractor_type: String,
            feature_size: usize,
            hop_length: usize,
            n_fft: usize,
            n_samples: usize,
            nb_max_frames: usize,
            padding_side: String,
            padding_value: f32,
            processor_class: String,
            return_attention_mask: bool,
            sampling_rate: usize,
            mel_filters: Option<Vec<Vec<f64>>>,
        }
        let aux: PreprocessorConfigAux = serde_json::from_reader(reader)?;

        let mel_filters = if let Some(mel_filters) = aux.mel_filters {
            let rows = mel_filters.len();
            let cols = mel_filters.first().map(|row| row.len()).unwrap_or_default();
            Array2::from_shape_vec((rows, cols), mel_filters.into_iter().flatten().collect())?
        } else {
            mel(
                aux.sampling_rate as f64,
                aux.n_fft,
                aux.feature_size,
                None,
                None,
                false,
                true,
            )
        };

        Ok(Self {
            chunk_length: aux.chunk_length,
            feature_extractor_type: aux.feature_extractor_type,
            feature_size: aux.feature_size,
            hop_length: aux.hop_length,
            n_fft: aux.n_fft,
            n_samples: aux.n_samples,
            nb_max_frames: aux.nb_max_frames,
            padding_side: aux.padding_side,
            padding_value: aux.padding_value,
            processor_class: aux.processor_class,
            return_attention_mask: aux.return_attention_mask,
            sampling_rate: aux.sampling_rate,
            mel_filters,
        })
    }
}

#[cfg(test)]
#[cfg(feature = "hub")]
mod tests {
    use crate::{download_model, Config, Device, Whisper};
    use std::path::Path;

    const MODEL_ID: &str = "jkawamoto/whisper-tiny-ct2";

    fn read_audio<T: AsRef<Path>>(path: T, sample_rate: usize) -> anyhow::Result<Vec<f32>> {
        use hound::WavReader;

        fn resample(samples: Vec<f32>, src_rate: usize, target_rate: usize) -> Vec<f32> {
            if src_rate == target_rate {
                return samples;
            }
            if src_rate > target_rate {
                let step = src_rate / target_rate;
                samples.into_iter().step_by(step).collect()
            } else {
                let factor = target_rate as f32 / src_rate as f32;
                let new_len = (samples.len() as f32 * factor) as usize;
                let mut resampled = Vec::with_capacity(new_len);
                for i in 0..new_len {
                    let src_idx = i as f32 / factor;
                    let idx_low = src_idx.floor() as usize;
                    let idx_high = (idx_low + 1).min(samples.len() - 1);
                    let weight = src_idx - idx_low as f32;
                    let val = samples[idx_low] * (1.0 - weight) + samples[idx_high] * weight;
                    resampled.push(val);
                }
                resampled
            }
        }

        let mut reader = WavReader::open(path)?;
        let spec = reader.spec();

        let max = 2_i32.pow((spec.bits_per_sample - 1) as u32) as f32;
        let samples = reader
            .samples::<i32>()
            .map(|s| s.unwrap() as f32 / max)
            .collect::<Vec<f32>>();

        if spec.channels == 1 {
            return Ok(resample(samples, spec.sample_rate as usize, sample_rate));
        }

        let mut mono = vec![];
        for chunk in samples.chunks(2) {
            if chunk.len() == 2 {
                mono.push((chunk[0] + chunk[1]) / 2.);
            }
        }

        Ok(resample(mono, spec.sample_rate as usize, sample_rate))
    }

    #[test]
    #[ignore]
    fn test_whisper_debug() {
        let model_path = download_model(MODEL_ID).unwrap();
        let w = Whisper::new(
            &model_path,
            Config {
                device: if cfg!(feature = "cuda") {
                    Device::CUDA
                } else {
                    Device::CPU
                },
                ..Default::default()
            },
        )
        .unwrap();

        assert!(format!("{:?}", w).contains(model_path.file_name().unwrap().to_str().unwrap()));
    }

    #[test]
    #[ignore]
    fn test_whisper_generate_segments() {
        let model_path = download_model(MODEL_ID).unwrap();
        let w = Whisper::new(
            &model_path,
            Config {
                device: if cfg!(feature = "cuda") {
                    Device::CUDA
                } else {
                    Device::CPU
                },
                ..Default::default()
            },
        )
        .unwrap();

        let wav_path = std::path::Path::new("tests/assets/test.wav");
        if !wav_path.exists() {
            if let Some(parent) = wav_path.parent() {
                std::fs::create_dir_all(parent).expect("failed to create directory for wav file");
            }
            let url =
                "https://www.voiptroubleshooter.com/open_speech/american/OSR_us_000_0010_8k.wav";
            let response = ureq::get(url).call().expect("failed to download wav file");
            let mut out = std::fs::File::create(wav_path).expect("failed to create wav file");
            std::io::copy(&mut response.into_reader(), &mut out).expect("failed to write wav file");
        }

        let samples = read_audio(wav_path, w.sampling_rate()).unwrap();

        let segments = w
            .generate_segments(&samples, Some("en"), &Default::default())
            .unwrap();
        assert!(
            !segments.is_empty(),
            "Generated segments should not be empty"
        );

        for segment in &segments {
            println!(
                "Segment {}: [{:.2} - {:.2}]: {}",
                segment.id, segment.start, segment.end, segment.text
            );
            if let Some(words) = &segment.words {
                for word in words {
                    println!(
                        "  Word: '{}' [{:.2} - {:.2}] prob={:.3}",
                        word.word, word.start, word.end, word.probability
                    );
                    assert!(
                        word.start <= word.end,
                        "Word start time must be less than or equal to end time"
                    );
                    assert!(
                        word.probability >= 0.0 && word.probability <= 1.0,
                        "Word probability must be between 0.0 and 1.0"
                    );
                }
            }
        }
    }
}
