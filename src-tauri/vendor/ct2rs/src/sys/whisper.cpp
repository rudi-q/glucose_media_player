// whisper.cpp
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

#include <utility>

#include "ct2rs/src/sys/whisper.rs.h"

#include "ct2rs/include/types.h"

using rust::Slice;
using rust::Vec;

std::unique_ptr<StorageView> Whisper::encode(
    const StorageView& features,
    const bool to_cpu
) const {
    auto future = impl->encode(
        features,
        to_cpu
    );

    StorageView storage_view = future.get();
    return std::make_unique<StorageView>(std::move(storage_view));
}

Vec<WhisperGenerationResult> Whisper::generate(
    const StorageView& features,
    const Slice<const VecStr> prompts,
    const WhisperOptions& opts
) const {
    auto futures = impl->generate(
        features,
        from_rust(prompts),
        ctranslate2::models::WhisperOptions {
            opts.beam_size,
            opts.patience,
            opts.length_penalty,
            opts.repetition_penalty,
            opts.no_repeat_ngram_size,
            opts.max_length,
            opts.sampling_topk,
            opts.sampling_temperature,
            opts.num_hypotheses,
            opts.return_scores,
            opts.return_logits_vocab,
            opts.return_no_speech_prob,
            opts.max_initial_timestamp_index,
            opts.suppress_blank,
            from_rust(opts.suppress_tokens),
        }
    );

    Vec<WhisperGenerationResult> res;
    for (auto& future : futures) {
        const auto& r = future.get();
        res.push_back(WhisperGenerationResult {
            to_rust<VecString>(r.sequences),
            to_rust<VecUSize>(r.sequences_ids),
            to_rust(r.scores),
            r.no_speech_prob,
        });
    }

    return res;
}

Vec<VecDetectionResult> Whisper::detect_language(const StorageView& features) const {
    auto futures = impl->detect_language(features);

    Vec<VecDetectionResult> res;
    for (auto& future : futures) {
        const auto& r = future.get();

        Vec<DetectionResult> pairs;
        for (auto& pair : r) {
            pairs.push_back(DetectionResult {
                std::get<0>(pair),
                std::get<1>(pair),
            });
        }

        res.push_back(VecDetectionResult { pairs });
    }

    return res;
}

Vec<WhisperAlignmentResult> Whisper::align(
    const StorageView& features,
    const Slice<const size_t> start_sequence,
    const Slice<const Vec<size_t>> text_tokens,
    const Slice<const size_t> num_frames,
    int64_t median_filter_width
) const {
    std::vector<size_t> start_sequence_cxx(start_sequence.begin(), start_sequence.end());

    std::vector<std::vector<size_t>> text_tokens_cxx;
    for (auto& seq_text_tokens : text_tokens) {
        text_tokens_cxx.emplace_back(seq_text_tokens.begin(), seq_text_tokens.end());
    }

    std::vector<size_t> num_frames_cxx(num_frames.begin(), num_frames.end());

    auto futures = impl->align(
        features, start_sequence_cxx, text_tokens_cxx, num_frames_cxx, median_filter_width
    );

    Vec<WhisperAlignmentResult> res;
    for (auto& future : futures) {
        const auto& result_cxx = future.get();

        WhisperAlignmentResult result;
        for (auto& token_alignment_cxx : result_cxx.alignments) {
            result.alignments.push_back(
                WhisperTokenAlignment { token_alignment_cxx.first, token_alignment_cxx.second }
            );
        }
        result.text_token_probs = to_rust(result_cxx.text_token_probs);

        res.push_back(result);
    }

    return res;
}
