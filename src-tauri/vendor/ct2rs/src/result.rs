// result.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides [`GenerationStepResult`].

use crate::{sys, Tokenizer};

/// The result for a single generation step.
#[derive(Clone, Debug)]
pub struct GenerationStepResult {
    /// The decoding step.
    pub step: usize,
    /// The batch index.
    pub batch_id: usize,
    /// Index of the hypothesis in the batch.
    pub hypothesis_id: usize,
    /// The generated text.
    pub text: String,
    /// Score of the token.
    pub score: Option<f32>,
    /// Whether this step is the last step for this batch.
    pub is_last: bool,
}

impl GenerationStepResult {
    pub(crate) fn from_ffi<T: Tokenizer>(
        r: sys::GenerationStepResult,
        tokenizer: &T,
    ) -> anyhow::Result<Self> {
        let text = tokenizer.decode(vec![r.token])?;
        Ok(Self {
            step: r.step,
            batch_id: r.batch_id,
            hypothesis_id: r.hypothesis_id,
            text,
            score: if r.has_score { Some(r.score) } else { None },
            is_last: r.is_last,
        })
    }
}
