// translator.cpp
//
// Copyright (c) 2023-2024 Junpei Kawamoto
//
// This software is released under the MIT License.
//
// http://opensource.org/licenses/mit-license.php

#include "ct2rs/src/sys/translator.rs.h"

#include "ct2rs/include/types.h"

using rust::Fn;
using rust::Str;
using rust::String;
using rust::Vec;
using std::string;
using std::variant;
using std::vector;

inline std::function<bool(ctranslate2::GenerationStepResult)> convert_callback(
    bool has_callback,
    TranslationCallbackBox& callback
) {
    if (!has_callback) {
        return nullptr;
    }

    return [&](ctranslate2::GenerationStepResult res) -> bool {
        return execute_translation_callback(
            callback,
            GenerationStepResult {
                res.step,
                res.batch_id,
                res.token_id,
                res.hypothesis_id,
                rust::String(res.token),
                res.score.has_value(),
                res.score.value_or(0),
                res.is_last,
            }
        );
    };
}

inline ctranslate2::TranslationOptions convert_options(
    const TranslationOptions& options,
    bool has_callback,
    TranslationCallbackBox& callback
) {
    variant<string, vector<string>, vector<size_t>> end_token;
    if (!options.end_token.empty()) {
        end_token = from_rust(options.end_token);
    }

    return ctranslate2::TranslationOptions {
        options.beam_size,
        options.patience,
        options.length_penalty,
        options.coverage_penalty,
        options.repetition_penalty,
        options.no_repeat_ngram_size,
        options.disable_unk,
        from_rust(options.suppress_sequences),
        options.prefix_bias_beta,
        end_token,
        options.return_end_token,
        options.max_input_length,
        options.max_decoding_length,
        options.min_decoding_length,
        options.sampling_topk,
        options.sampling_topp,
        options.sampling_temperature,
        options.use_vmap,
        options.num_hypotheses,
        options.return_scores,
        options.return_attention,
        options.return_logits_vocab,
        options.return_alternatives,
        options.min_alternative_expansion_prob,
        options.replace_unknowns,
        convert_callback(has_callback, callback),
    };
}

inline Vec<TranslationResult> convert_results(const std::vector<ctranslate2::TranslationResult> results) {
    Vec<TranslationResult> res;
    for (const auto& item : results) {
        res.push_back(TranslationResult {
            to_rust<VecString>(item.hypotheses),
            to_rust(item.scores),
            // to_rust(item.attention),
        });
    }
    return res;
}

Vec<TranslationResult> Translator::translate_batch(
    const Vec<VecStr>& source,
    const TranslationOptions& options,
    bool has_callback,
    TranslationCallbackBox& callback
) const {
    return convert_results(this->impl->translate_batch(
        from_rust(source),
        convert_options(options, has_callback, callback),
        options.max_batch_size,
        options.batch_type
    ));
}

Vec<TranslationResult> Translator::translate_batch_with_target_prefix(
    const Vec<VecStr>& source,
    const Vec<VecStr>& target_prefix,
    const TranslationOptions& options,
    bool has_callback,
    TranslationCallbackBox& callback
) const {
    return convert_results(this->impl->translate_batch(
        from_rust(source),
        from_rust(target_prefix),
        convert_options(options, has_callback, callback),
        options.max_batch_size,
        options.batch_type
    ));
}
