// translator.h
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

#pragma once

#include <memory>

#include <ctranslate2/translator.h>

#include "rust/cxx.h"

#include "config.h"

struct VecStr;
struct TranslationOptions;
struct TranslationResult;
struct GenerationStepResult;
struct TranslationCallbackBox;

class Translator {
private:
    std::unique_ptr<ctranslate2::Translator> impl;

public:
    Translator(std::unique_ptr<ctranslate2::Translator> impl)
        : impl(std::move(impl)) { }

    rust::Vec<TranslationResult>
    translate_batch(
        const rust::Vec<VecStr>& source,
        const TranslationOptions& options,
        bool has_callback,
        TranslationCallbackBox& callback
    ) const;

    rust::Vec<TranslationResult>
    translate_batch_with_target_prefix(
        const rust::Vec<VecStr>& source,
        const rust::Vec<VecStr>& target_prefix,
        const TranslationOptions& options,
        bool has_callback,
        TranslationCallbackBox& callback
    ) const;

    inline size_t num_queued_batches() const {
        return this->impl->num_queued_batches();
    }

    inline size_t num_active_batches() const {
        return this->impl->num_active_batches();
    }

    inline size_t num_replicas() const {
        return this->impl->num_replicas();
    }
};

inline std::unique_ptr<Translator> translator(
    rust::Str model_path,
    std::unique_ptr<Config> config
) {
    return std::make_unique<Translator>(std::make_unique<ctranslate2::Translator>(
        static_cast<std::string>(model_path),
        config->device,
        config->compute_type,
        std::vector<int>(config->device_indices.begin(), config->device_indices.end()),
        config->tensor_parallel,
        *config->replica_pool_config
    ));
}
