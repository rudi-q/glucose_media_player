// storage_view.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module provides a Rust binding to the
//! [`ctranslate2::StorageView`](https://opennmt.net/CTranslate2/python/ctranslate2.StorageView.html).

use std::fmt::{Debug, Formatter};
use std::ops::Deref;

use anyhow::Result;
use cxx::UniquePtr;

#[cxx::bridge]
pub(crate) mod ffi {
    unsafe extern "C++" {
        include!("ct2rs/include/model_memory_reader.h");

        type ModelMemoryReader;

        fn model_memory_reader(model_name: &str) -> Result<UniquePtr<ModelMemoryReader>>;

        fn get_model_id(self: &ModelMemoryReader) -> String;

        fn register_file(self: Pin<&mut ModelMemoryReader>, filename: &str, content: &[u8]);
    }
}

/// An allocated buffer with shape information.
pub struct ModelMemoryReader {
    ptr: UniquePtr<ffi::ModelMemoryReader>,
    model_name: String,
}

impl ModelMemoryReader {
    /// Creates a storage view with the given shape from the given array of float values.
    pub fn new(model_name: &str) -> Result<Self> {
        Ok(Self {
            ptr: ffi::model_memory_reader(model_name)?,
            model_name: String::from(model_name),
        })
    }

    pub fn get_model_id(&self) -> String {
        self.ptr.get_model_id()
    }

    /// Add the contents of a file.
    pub fn register_file(&mut self, filename: &str, content: &[u8]) {
        ffi::ModelMemoryReader::register_file(self.ptr.pin_mut(), filename, content)
    }

    pub(crate) fn pin_mut_impl(&mut self) -> std::pin::Pin<&mut ffi::ModelMemoryReader> {
        self.ptr.pin_mut()
    }
}

impl Debug for ModelMemoryReader {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "ModelMemoryReader {{{} }}", self.model_name)
    }
}

impl Deref for ModelMemoryReader {
    type Target = ffi::ModelMemoryReader;

    fn deref(&self) -> &Self::Target {
        &self.ptr
    }
}

unsafe impl Send for ModelMemoryReader {}
unsafe impl Sync for ModelMemoryReader {}

#[cfg(test)]
mod tests {
    use super::ModelMemoryReader;

    #[test]
    fn test_model_memory_reader() {
        let mut reader = ModelMemoryReader::new("whisper").expect("Constructor failed.");
        let bytes = vec![65_u8; 32];
        reader.register_file("model.bytes", &bytes);
    }
}
