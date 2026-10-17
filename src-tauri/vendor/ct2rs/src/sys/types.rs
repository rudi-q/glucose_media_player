// types.rs
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

//! This module defines common structures.

use std::fmt::{Debug, Formatter};

pub use self::ffi::GenerationStepResult;
pub(super) use self::ffi::{VecStr, VecString, VecUSize};

#[cxx::bridge]
pub(crate) mod ffi {
    /// The result for a single generation step.
    ///
    /// This struct is a Rust binding to the
    /// [`ctranslate2.GenerationStepResult`](https://opennmt.net/CTranslate2/python/ctranslate2.GenerationStepResult.html).
    #[derive(Clone, Debug)]
    pub struct GenerationStepResult {
        /// The decoding step.
        pub step: usize,
        /// The batch index.
        pub batch_id: usize,
        /// ID of the generated token.
        pub token_id: usize,
        /// Index of the hypothesis in the batch.
        pub hypothesis_id: usize,
        /// String value of the generated token.
        pub token: String,
        /// true if score is given.
        pub has_score: bool,
        ///  Score of the token.
        pub score: f32,
        /// Whether this step is the last generation step for this batch.
        pub is_last: bool,
    }

    #[derive(PartialEq, Clone)]
    pub struct VecString {
        v: Vec<String>,
    }

    #[derive(PartialEq, Clone)]
    pub struct VecStr<'a> {
        v: Vec<&'a str>,
    }

    #[derive(PartialEq, Clone)]
    pub struct VecUSize {
        v: Vec<usize>,
    }

    struct _dummy<'a> {
        _vec_string: Vec<VecString>,
        _vec_str: Vec<VecStr<'a>>,
        _vec_usize: Vec<VecUSize>,
    }
}

#[inline]
pub(crate) fn vec_ffi_vecstr<T: AsRef<str>>(src: &[Vec<T>]) -> Vec<VecStr<'_>> {
    src.iter()
        .map(|v| VecStr {
            v: v.iter().map(AsRef::as_ref).collect(),
        })
        .collect()
}

impl From<VecString> for Vec<String> {
    fn from(value: VecString) -> Self {
        value.v
    }
}

impl From<Vec<String>> for VecString {
    fn from(v: Vec<String>) -> Self {
        Self { v }
    }
}

impl Debug for VecString {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.v.fmt(f)
    }
}

impl<'a> Debug for VecStr<'a> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.v.fmt(f)
    }
}

impl From<VecUSize> for Vec<usize> {
    fn from(value: VecUSize) -> Self {
        value.v
    }
}

impl From<Vec<usize>> for VecUSize {
    fn from(v: Vec<usize>) -> Self {
        Self { v }
    }
}

impl Debug for VecUSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.v.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::ffi::{VecString, VecUSize};
    use super::vec_ffi_vecstr;

    #[test]
    fn str_vectors() {
        let data = vec![vec!["a", "b", "c"], vec!["1", "2"]];
        let res = vec_ffi_vecstr(&data);

        assert_eq!(res.len(), data.len());
        for (i, list) in data.iter().enumerate() {
            let v = &res.get(i).unwrap().v;
            assert_eq!(v.len(), list.len());
            for (j, s) in list.iter().enumerate() {
                assert_eq!(v.get(j).unwrap(), s);
            }
        }
    }

    #[test]
    fn empty_inner_vectors() {
        let data: Vec<Vec<&str>> = vec![vec![], vec![]];
        let res = vec_ffi_vecstr(&data);

        assert_eq!(res.len(), data.len());
        for item in res.iter() {
            assert_eq!(item.v.len(), 0);
        }
    }

    #[test]
    fn empty_vectors() {
        let data: Vec<Vec<&str>> = vec![];
        let res = vec_ffi_vecstr(&data);

        assert_eq!(res.len(), 0);
    }

    #[test]
    fn from_vec_string() {
        let s = vec!["a".to_string(), "b".to_string()];
        let v = VecString { v: s.clone() };

        let res: Vec<String> = v.into();
        assert_eq!(s, res);
    }

    #[test]
    fn into_vec_string() {
        let v = vec!["a".to_string(), "b".to_string()];
        let res: VecString = v.clone().into();

        assert_eq!(res, VecString { v });
    }

    #[test]
    fn from_vec_usize() {
        let s: Vec<usize> = vec![1, 2, 3];
        let v = VecUSize { v: s.clone() };

        let res: Vec<usize> = v.into();
        assert_eq!(s, res);
    }

    #[test]
    fn into_vec_usize() {
        let v: Vec<usize> = vec![1, 2, 3];
        let res: VecUSize = v.clone().into();

        assert_eq!(res, VecUSize { v });
    }
}
