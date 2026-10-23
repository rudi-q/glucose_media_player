// On-device translation of live subtitle cues with CTranslate2 and Helsinki-NLP's OPUS-MT
// models: from English into several languages, and from several languages into English.
// See docs/spikes/subtitle-translation.md.

use crate::subtitle_format::{self, Cue, Word};
use ct2rs::tokenizers::sentencepiece::Tokenizer;
use ct2rs::{ComputeType, Config, TranslationOptions, Translator};
use std::path::PathBuf;

// Source language, target language, model folder (inside a Whisper model folder). Each
// folder holds a CTranslate2 int8 conversion of the Hugging Face model plus its
// source.spm and target.spm. The `opus-mt_tiny_*` models are the compact distilled ones;
// `opus-mt-hi-en` is standard size, since no compact model exists for that pair.
pub(crate) const MODELS: &[(&str, &str, &str)] = &[
    ("en", "de", "opus-mt_tiny_eng-deu"),
    ("en", "fr", "opus-mt_tiny_eng-fra"),
    ("en", "es", "opus-mt_tiny_eng-spa"),
    ("en", "ru", "opus-mt_tiny_eng-rus"),
    ("en", "tr", "opus-mt_tiny_eng-tur"),
    ("en", "it", "opus-mt_tiny_eng-ita"),
    ("en", "nl", "opus-mt_tiny_eng-nld"),
    ("de", "en", "opus-mt_tiny_deu-eng"),
    ("fr", "en", "opus-mt_tiny_fra-eng"),
    ("es", "en", "opus-mt_tiny_spa-eng"),
    ("it", "en", "opus-mt_tiny_ita-eng"),
    ("nl", "en", "opus-mt_tiny_nld-eng"),
    ("ru", "en", "opus-mt_tiny_rus-eng"),
    ("tr", "en", "opus-mt_tiny_tur-eng"),
    ("hi", "en", "opus-mt-hi-en"),
];

const LANGUAGE_NAMES: &[(&str, &str)] = &[
    ("en", "English"),
    ("de", "German"),
    ("fr", "French"),
    ("es", "Spanish"),
    ("ru", "Russian"),
    ("tr", "Turkish"),
    ("it", "Italian"),
    ("nl", "Dutch"),
    ("hi", "Hindi"),
    // Languages no model covers, named in "not available" messages.
    ("zh", "Chinese"),
    ("ja", "Japanese"),
    ("ko", "Korean"),
    ("ar", "Arabic"),
    ("pt", "Portuguese"),
    ("pl", "Polish"),
    ("uk", "Ukrainian"),
    ("sv", "Swedish"),
];

// A sentence group longer than this is translated in pieces, to stay well inside the
// model's input limit.
const MAX_GROUP_CHARS: usize = 400;

// The English name of a Whisper language code, or the code itself for languages not listed.
pub(crate) fn language_name(code: &str) -> String {
    LANGUAGE_NAMES
        .iter()
        .find(|(c, _)| *c == code)
        .map_or_else(|| code.to_string(), |(_, name)| name.to_string())
}

// Whether any model translates `source` speech into `target`, installed or not.
pub(crate) fn is_supported(source: &str, target: &str) -> bool {
    MODELS.iter().any(|(s, t, _)| *s == source && *t == target)
}

// Files a model folder needs; a folder missing any of them (for example after an
// interrupted copy) counts as not installed.
// Also the files translation_models.rs downloads, so a folder counts as installed
// exactly when a download would have completed it.
pub(crate) const MODEL_FILES: &[&str] = &["config.json", "model.bin", "shared_vocabulary.json", "source.spm", "target.spm"];

pub(crate) fn installed_dir(folder: &str) -> Option<PathBuf> {
    super::model_candidate_dirs()
        .into_iter()
        .map(|dir| dir.join(folder))
        .find(|dir| MODEL_FILES.iter().all(|f| dir.join(f).exists()))
}

fn model_dir(source: &str, target: &str) -> Result<PathBuf, String> {
    let (_, _, folder) = MODELS
        .iter()
        .find(|(s, t, _)| *s == source && *t == target)
        .ok_or_else(|| {
            format!(
                "Translation from {} to {} is not supported",
                language_name(source),
                language_name(target)
            )
        })?;
    installed_dir(folder).ok_or_else(|| {
        format!(
            "The {} to {} translation model is not installed",
            language_name(source),
            language_name(target)
        )
    })
}

// Fails early, before a live session starts, if translating `source` speech into `target`
// is not possible. With a known source this checks that exact pair; with `None` (the
// speech language is auto-detected) it checks that some model translates into `target`.
pub(crate) fn check_installed(source: Option<&str>, target: &str) -> Result<(), String> {
    if let Some(source) = source {
        return if source == target { Ok(()) } else { model_dir(source, target).map(|_| ()) };
    }
    let supported: Vec<_> = MODELS.iter().filter(|(_, t, _)| *t == target).collect();
    if supported.is_empty() {
        return Err(format!("Translation into {} is not supported", language_name(target)));
    }
    if supported.iter().any(|(_, _, folder)| installed_dir(folder).is_some()) {
        Ok(())
    } else {
        Err(format!("The {} translation model is not installed", language_name(target)))
    }
}

pub(crate) struct CueTranslator {
    pub source: String,
    pub target: String,
    translator: Translator<Tokenizer>,
}

impl CueTranslator {
    pub fn load(source: &str, target: &str, threads: usize) -> Result<Self, String> {
        let dir = model_dir(source, target)?;
        let tokenizer = Tokenizer::new(&dir).map_err(|e| format!("Failed to load the translation tokenizer: {}", e))?;
        let config = Config {
            compute_type: ComputeType::INT8,
            num_threads_per_replica: threads,
            ..Config::default()
        };
        let translator = Translator::with_tokenizer(&dir, tokenizer, &config)
            .map_err(|e| format!("Failed to load the translation model: {}", e))?;
        Ok(Self {
            source: source.to_string(),
            target: target.to_string(),
            translator,
        })
    }

    // Translates cues in the source language and reshapes the result into cues for the
    // target language.
    // `limit` is the latest time a cue may extend to, as for `build_cues`.
    pub fn translate_cues(&self, cues: &[Cue], limit: f64) -> Result<Vec<Cue>, String> {
        if cues.is_empty() {
            return Ok(Vec::new());
        }
        let groups = sentence_groups(cues);
        let texts: Vec<String> = groups.iter().map(|g| group_text(g)).collect();
        // The tiny int8 models could loop on a short fragment ("the the the ..."), so the
        // output may be at most about twice as long as the longest input (measured in
        // characters here, which is more than its tokens). Repetition penalties are not
        // used: tried on the German and French models, they dropped deliberate repeats
        // ("Nein, nein, nein") and introduced grammar errors.
        let longest = texts.iter().map(|t| t.chars().count()).max().unwrap_or(0);
        let options: TranslationOptions<String, String> = TranslationOptions {
            beam_size: 2,
            max_decoding_length: (2 * longest + 10).min(MAX_GROUP_CHARS * 2 + 10),
            ..Default::default()
        };
        let translated = self
            .translator
            .translate_batch(&texts, &options, None)
            .map_err(|e| format!("Translation failed: {}", e))?;
        // Each group must get its translation, or its cues would silently vanish.
        if translated.len() != texts.len() {
            return Err(format!(
                "Translation returned {} results for {} sentences",
                translated.len(),
                texts.len()
            ));
        }

        let words: Vec<Word> = groups
            .iter()
            .zip(translated)
            .flat_map(|(group, (text, _))| spread_words(&text, group))
            .collect();
        Ok(subtitle_format::build_cues(&words, limit, &self.target))
    }
}

fn cue_text(cue: &Cue) -> String {
    cue.text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn group_text(group: &[Cue]) -> String {
    group.iter().map(cue_text).collect::<Vec<_>>().join(" ")
}

// Splits cues into runs that each end with a finished sentence, so the model sees whole
// sentences rather than fragments cut at cue boundaries.
fn sentence_groups(cues: &[Cue]) -> Vec<&[Cue]> {
    let mut groups = Vec::new();
    let mut start = 0;
    let mut chars = 0;
    for (i, cue) in cues.iter().enumerate() {
        chars += cue_text(cue).chars().count() + 1;
        let last = i + 1 == cues.len();
        if last || subtitle_format::ends_sentence(&cue.text) || chars >= MAX_GROUP_CHARS {
            groups.push(&cues[start..=i]);
            start = i + 1;
            chars = 0;
        }
    }
    groups
}

// Spreads a translation over the timing of the source cues it came from. Each source cue
// gets a share of the translation in proportion to its share of the source text, and
// words are spaced evenly within that share. Pauses between source cues are kept, so the
// cue builder still breaks on them. Word alignments from the model would make this exact;
// proportional placement is close enough for sentence-sized groups.
fn spread_words(translation: &str, source: &[Cue]) -> Vec<Word> {
    let words: Vec<&str> = translation.split_whitespace().collect();
    if words.is_empty() || source.is_empty() {
        return Vec::new();
    }
    let lens: Vec<f64> = source.iter().map(|c| cue_text(c).chars().count().max(1) as f64).collect();
    let total_src: f64 = lens.iter().sum();
    // Each source cue's share of the text, as [from, to) fractions of it.
    let mut bounds = Vec::with_capacity(source.len());
    let mut acc = 0.0;
    for len in &lens {
        bounds.push((acc, acc + len / total_src));
        acc += len / total_src;
    }
    // A time inside cue `i` for a fraction of the text, clamped to that cue.
    let time_in = |i: usize, fraction: f64| -> f64 {
        let ((from, to), cue) = (bounds[i], &source[i]);
        let within = ((fraction - from) / (to - from)).clamp(0.0, 1.0);
        cue.start + within * (cue.end - cue.start)
    };

    let total_tgt = words.iter().map(|w| w.chars().count() + 1).sum::<usize>() as f64;
    let mut pos = 0.0;
    words
        .into_iter()
        .map(|w| {
            let len = (w.chars().count() + 1) as f64;
            let from = pos / total_tgt;
            // The trailing separator is not part of the word.
            let to = ((pos + len - 1.0) / total_tgt).max(from);
            pos += len;
            // A word belongs wholly to the source cue holding its middle, so none spans
            // the pause between two cues (or runs past the 7 s cue limit through it).
            let middle = (from + to) / 2.0;
            let i = bounds.iter().position(|&(_, end)| middle < end).unwrap_or(source.len() - 1);
            let start = time_in(i, from);
            Word {
                start,
                end: time_in(i, to).max(start),
                text: w.to_string(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f64, end: f64, text: &str) -> Cue {
        Cue { start, end, text: text.to_string() }
    }

    #[test]
    fn groups_end_at_sentences() {
        let cues = vec![
            cue(0.0, 2.0, "Teaching a machine to do that exact\nsame thing is without a doubt one"),
            cue(2.1, 4.0, "of the greatest challenges."),
            cue(4.5, 6.0, "You've heard of it, right?"),
            cue(6.1, 7.0, "Someone who"),
        ];
        let groups = sentence_groups(&cues);
        let sizes: Vec<usize> = groups.iter().map(|g| g.len()).collect();
        assert_eq!(sizes, vec![2, 1, 1]);
        assert!(group_text(groups[0]).contains("exact same thing"));
    }

    #[test]
    fn spread_words_follows_source_timing() {
        let source = vec![cue(10.0, 12.0, "aaaa bbbb"), cue(15.0, 17.0, "cccc dddd")];
        let words = spread_words("eins zwei drei vier", &source);
        assert_eq!(words.len(), 4);
        // The first half of the translation lands in the first cue, the rest in the second.
        assert!(words[0].start >= 10.0 && words[1].end <= 12.0 + 1e-9, "{:?}", words);
        assert!(words[2].start >= 15.0 - 1e-9 && words[3].end <= 17.0 + 1e-9, "{:?}", words);
        // Times never go backwards.
        assert!(words.windows(2).all(|p| p[0].start <= p[1].start));
        // The pause between the cues stays a pause, so the cue builder can break there.
        assert!(words[2].start - words[1].end > 2.0);
    }

    #[test]
    fn a_word_straddling_two_cues_stays_in_one() {
        let source = vec![cue(10.0, 12.0, "aaaa bbbb"), cue(15.0, 17.0, "cccc dddd")];
        let words = spread_words("ein zwei drei", &source);
        // "zwei" sits across the boundary; it goes wholly into one cue, not through the pause.
        for w in &words {
            let in_first = w.start >= 10.0 && w.end <= 12.0 + 1e-9;
            let in_second = w.start >= 15.0 - 1e-9 && w.end <= 17.0 + 1e-9;
            assert!(in_first || in_second, "{:?}", words);
        }
        assert!(words.windows(2).all(|p| p[0].start <= p[1].start));
    }

    // Needs the German model installed; run with `cargo test --lib -- --ignored`.
    #[test]
    #[ignore]
    fn translates_real_cues_with_installed_model() {
        let translator = CueTranslator::load("en", "de", 4).expect("German model installed");
        let cues = vec![
            cue(46.4, 49.1, "Teaching a machine to do that exact\nsame thing is without a doubt one"),
            cue(49.2, 51.0, "of the greatest technological challenges."),
            cue(56.2, 60.1, "You've probably heard the term \"full-stack\ndeveloper\" in computer science, right?"),
        ];
        let out = translator.translate_cues(&cues, 62.0).unwrap();
        for c in &out {
            println!("{:>6.2} {:>6.2} {:?}", c.start, c.end, c.text);
        }
        assert!(!out.is_empty());
        let v = subtitle_format::check(&out);
        assert_eq!((v.over_two_lines, v.line_too_long, v.too_long), (0, 0, 0), "{:?}", v);
        assert!(out.iter().all(|c| c.start >= 46.4 - 1e-9 && c.end <= 62.0 + 1e-9));
    }

    // Needs the it/nl/ru/tr -> en models installed; run with
    // `cargo test --lib translates_more_languages -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn translates_more_languages_into_english() {
        for (source, text) in [
            ("it", "Non sapevo che avesse venduto il suo appartamento a Berlino."),
            ("nl", "Ik wist niet dat ze haar appartement in Berlijn had verkocht."),
            ("ru", "Я не знал, что она продала свою квартиру в Берлине."),
            ("tr", "Berlin'deki dairesini sattığını bilmiyordum."),
        ] {
            let translator = CueTranslator::load(source, "en", 4).expect("model installed");
            let out = translator.translate_cues(&[cue(1.0, 4.0, text)], 5.0).unwrap();
            let english: String = out.iter().map(|c| c.text.replace('\n', " ")).collect::<Vec<_>>().join(" ");
            println!("{source}: {english}");
            assert!(english.to_lowercase().contains("berlin"), "{source}: {english}");
        }
    }

    // Needs the hi-en model installed; run with `cargo test --lib -- --ignored`.
    #[test]
    #[ignore]
    fn translates_devanagari_cues_into_english() {
        let translator = CueTranslator::load("hi", "en", 4).expect("hi-en model installed");
        let cues = vec![
            cue(0.0, 2.0, "इस वीडियो में आपका स्वागत है।"),
            cue(2.5, 4.0, "आज हम एक बहुत दिलचस्प"),
            cue(4.1, 6.0, "विषय के बारे में बात करेंगे।"),
        ];
        // The danda ends the first sentence; the second spans two cues.
        assert_eq!(sentence_groups(&cues).len(), 2);
        let out = translator.translate_cues(&cues, 8.0).unwrap();
        for c in &out {
            println!("{:>6.2} {:>6.2} {:?}", c.start, c.end, c.text);
        }
        assert!(!out.is_empty());
        assert!(out.iter().all(|c| c.text.is_ascii()), "expected English output: {:?}", out);
    }

    #[test]
    fn spread_words_handles_empty_input() {
        assert!(spread_words("", &[cue(0.0, 1.0, "x")]).is_empty());
        assert!(spread_words("x", &[]).is_empty());
    }
}
