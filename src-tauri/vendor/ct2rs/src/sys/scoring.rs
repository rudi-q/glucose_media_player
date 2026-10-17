// scoring.rs
//
// Copyright (c) 2023-2025 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! Structures for scoring.

use super::BatchType;
pub use ffi::{ScoringOptions, ScoringResult};

#[cxx::bridge]
pub(crate) mod ffi {

    /// `ScoringOptions` specifies configuration options for the scoring process.
    ///
    /// # Examples
    ///
    /// Example of creating a default `ScoringOptions`:
    ///
    /// ```
    /// use ct2rs::sys::ScoringOptions;
    ///
    /// let opts = ScoringOptions::default();
    /// # assert_eq!(opts.max_input_length, 1024);
    /// # assert_eq!(opts.offset, 0);
    /// # assert_eq!(opts.max_batch_size, 0);
    /// # assert_eq!(opts.batch_type, Default::default());
    /// ```
    #[derive(Clone, Debug)]
    pub struct ScoringOptions {
        /// Truncate the inputs after this many tokens (set 0 to disable truncation).
        /// (default: 1024)
        pub max_input_length: usize,
        /// Offset. (default: 0)
        pub offset: i64,
        /// The maximum batch size.
        /// If the number of inputs is greater than `max_batch_size`,
        /// the inputs are sorted by length and split by chunks of `max_batch_size` examples
        /// so that the number of padding positions is minimized.
        /// (default: 0)
        max_batch_size: usize,
        /// Whether `max_batch_size` is the number of `examples` or `tokens`.
        batch_type: BatchType,
    }

    /// `ScoringResult` represents the result of a scoring process,
    /// containing tokens and their respective scores.
    #[derive(Clone, Debug)]
    pub struct ScoringResult {
        /// The scored tokens.
        pub tokens: Vec<String>,
        /// Log probability of each token.
        pub tokens_score: Vec<f32>,
    }

    struct _dummy {
        _vec_scoring_result: Vec<ScoringResult>,
    }

    unsafe extern "C++" {
        include!("ct2rs/include/config.h");

        type BatchType = super::BatchType;
    }
}

impl Default for ScoringOptions {
    fn default() -> Self {
        Self {
            max_input_length: 1024,
            offset: 0,
            max_batch_size: 0,
            batch_type: Default::default(),
        }
    }
}

impl ScoringResult {
    /// Calculates and returns the total sum of all token scores.
    pub fn cumulated_score(&self) -> f32 {
        self.tokens_score.iter().sum()
    }

    /// Computes the average score per token, returning 0.0 if there are no tokens.
    pub fn normalized_score(&self) -> f32 {
        let num_tokens = self.tokens_score.len();
        if num_tokens == 0 {
            return 0.0;
        }
        self.cumulated_score() / num_tokens as f32
    }
}

#[cfg(test)]
mod tests {
    use crate::sys::scoring::ScoringResult;

    const EPSILON: f32 = 1e-6;

    #[test]
    fn test_scoring_result() {
        let res = ScoringResult {
            tokens: vec!["a".to_string(), "b".to_string(), "c".to_string()],
            tokens_score: vec![1.0, 2.0, 3.0],
        };
        assert!((res.cumulated_score() - 6.0).abs() < EPSILON);
        assert!((res.normalized_score() - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_empty_scoring_result() {
        let res = ScoringResult {
            tokens: vec![],
            tokens_score: vec![],
        };

        assert_eq!(res.cumulated_score(), 0.0);
        assert_eq!(res.normalized_score(), 0.0);
    }
}
