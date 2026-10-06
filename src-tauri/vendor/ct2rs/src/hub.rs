// hub.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides functions for preparing model files.

use anyhow::{anyhow, Result};
use hf_hub::api::sync::Api;
use std::path::PathBuf;

/// Downloads a model by its ID and returns the path to the directory where the model files are stored.
///
/// # Arguments
/// * `model_id` - The ID of the model to be downloaded from Hugging Face.
///
/// # Returns
/// Returns a `PathBuf` pointing to the directory where the model files are stored if the download
/// is successful. If no model files are found, it returns an error.
pub fn download_model<T: AsRef<str>>(model_id: T) -> Result<PathBuf> {
    let api = Api::new()?;
    let repo = api.model(model_id.as_ref().to_string());

    let mut res = None;
    for f in repo.info()?.siblings {
        let path = repo.get(&f.rfilename)?;
        if res.is_none() {
            res = path.parent().map(PathBuf::from);
        }
    }

    res.ok_or_else(|| anyhow!("no model files are found"))
}
