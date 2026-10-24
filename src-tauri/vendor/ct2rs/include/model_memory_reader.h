// storage_view.h
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

#pragma once

#include <memory>
#include <sstream>
#include <string>

#include <ctranslate2/models/model_reader.h>

#include "rust/cxx.h"

class ModelMemoryReader {
private:
    std::shared_ptr<ctranslate2::models::ModelMemoryReader> impl;

public:
    ModelMemoryReader(std::shared_ptr<ctranslate2::models::ModelMemoryReader> ct2_reader)
        : impl(std::move(ct2_reader)) { }

    ModelMemoryReader(rust::Str model_name) {
        impl = std::make_shared<ctranslate2::models::ModelMemoryReader>(
            std::string(model_name)
        );
    }

    const std::shared_ptr<ctranslate2::models::ModelMemoryReader>& get_impl() const {
        return impl;
    }

    rust::String get_model_id() const {
        std::string model_id_cxx = impl->get_model_id();
        return rust::String(model_id_cxx);
    }

    void register_file(rust::Str filename, rust::Slice<const uint8_t> content) {
        std::string filename_cxx(filename.begin(), filename.end());
        std::string content_cxx(content.begin(), content.end());
        impl->register_file(filename_cxx, content_cxx);
    }
};

inline std::unique_ptr<ModelMemoryReader> model_memory_reader(
    rust::Str model_name
) {
    return std::make_unique<ModelMemoryReader>(model_name);
}
