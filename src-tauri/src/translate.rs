// On-device translation of live subtitle cues from English into German or French, using
// CTranslate2 and Helsinki-NLP's compact OPUS-MT models.
// See docs/spikes/subtitle-translation.md.

use crate::subtitle_format::{self, Cue, Word};
use ct2rs::tokenizers::sentencepiece::Tokenizer;
use ct2rs::{ComputeType, Config, TranslationOptions, Translator};
use std::path::PathBuf;

// Target language code, model folder name (inside a Whisper model folder), display name.
// The folder holds a CTranslate2 int8 conversion of the Hugging Face model plus its
// source.spm and target.spm.
const MODELS: &[(&str, &str, &str)] = &[
    ("de", "opus-mt_tiny_eng-deu", "German"),
    ("fr", "opus-mt_tiny_eng-fra", "French"),
];

// A sentence group longer than this is translated in pieces, to stay well inside the
// model's input limit.
const MAX_GROUP_CHARS: usize = 400;

fn model_entry(lang: &str) -> Result<&'static (&'static str, &'static str, &'static str), String> {
    MODELS
        .iter()
        .find(|(code, _, _)| *code == lang)
        .ok_or_else(|| format!("Translation to '{}' is not supported", lang))
}

fn model_dir(lang: &str) -> Result<PathBuf, String> {
    let (_, folder, name) = model_entry(lang)?;
    super::model_candidate_dirs()
        .into_iter()
        .map(|dir| dir.join(folder))
        .find(|dir| dir.join("model.bin").exists())
        .ok_or_else(|| format!("The {} translation model is not installed", name))
}

// Fails early, before a live session starts, if the model for `lang` is missing.
pub(crate) fn check_installed(lang: &str) -> Result<(), String> {
    model_dir(lang).map(|_| ())
}

pub(crate) fn language_name(lang: &str) -> &'static str {
    model_entry(lang).map(|(_, _, name)| *name).unwrap_or("translation")
}

pub(crate) struct CueTranslator {
    pub lang: String,
    translator: Translator<Tokenizer>,
}

impl CueTranslator {
    pub fn load(lang: &str, threads: usize) -> Result<Self, String> {
        let dir = model_dir(lang)?;
        let tokenizer = Tokenizer::new(&dir).map_err(|e| format!("Failed to load the translation tokenizer: {}", e))?;
        let config = Config {
            compute_type: ComputeType::INT8,
            num_threads_per_replica: threads,
            ..Config::default()
        };
        let translator = Translator::with_tokenizer(&dir, tokenizer, &config)
            .map_err(|e| format!("Failed to load the translation model: {}", e))?;
        Ok(Self {
            lang: lang.to_string(),
            translator,
        })
    }

    // Translates English cues and reshapes the result into cues for the target language.
    // `limit` is the latest time a cue may extend to, as for `build_cues`.
    pub fn translate_cues(&self, cues: &[Cue], limit: f64) -> Result<Vec<Cue>, String> {
        if cues.is_empty() {
            return Ok(Vec::new());
        }
        let groups = sentence_groups(cues);
        let texts: Vec<String> = groups.iter().map(|g| group_text(g)).collect();
        let options: TranslationOptions<String, String> = TranslationOptions {
            beam_size: 2,
            ..Default::default()
        };
        let translated = self
            .translator
            .translate_batch(&texts, &options, None)
            .map_err(|e| format!("Translation failed: {}", e))?;

        let words: Vec<Word> = groups
            .iter()
            .zip(translated)
            .flat_map(|(group, (text, _))| spread_words(&text, group))
            .collect();
        Ok(subtitle_format::build_cues(&words, limit))
    }
}

fn cue_text(cue: &Cue) -> String {
    cue.text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn group_text(group: &[Cue]) -> String {
    group.iter().map(cue_text).collect::<Vec<_>>().join(" ")
}

fn ends_sentence(text: &str) -> bool {
    text.trim_end()
        .trim_end_matches(['"', '\'', ')', ']'])
        .ends_with(['.', '?', '!'])
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
        if last || ends_sentence(&cue.text) || chars >= MAX_GROUP_CHARS {
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
    // Maps a position in [0, 1] of the text to a time inside the source cues. A position
    // exactly on a boundary between two cues belongs to the next cue when it starts a
    // word and to the previous cue when it ends one, so no word spans the pause.
    let time_at = |fraction: f64, is_end: bool| -> f64 {
        let mut acc = 0.0;
        for (cue, len) in source.iter().zip(&lens) {
            let share = len / total_src;
            let inside = if is_end { fraction <= acc + share } else { fraction < acc + share };
            if inside || std::ptr::eq(cue, source.last().unwrap()) {
                let within = ((fraction - acc) / share).clamp(0.0, 1.0);
                return cue.start + within * (cue.end - cue.start);
            }
            acc += share;
        }
        source.last().unwrap().end
    };

    let total_tgt = words.iter().map(|w| w.chars().count() + 1).sum::<usize>() as f64;
    let mut pos = 0.0;
    words
        .into_iter()
        .map(|w| {
            let len = (w.chars().count() + 1) as f64;
            let start = time_at(pos / total_tgt, false);
            // The trailing separator is not part of the word.
            let end = time_at(((pos + len - 1.0) / total_tgt).max(pos / total_tgt), true);
            pos += len;
            Word {
                start,
                end: end.max(start),
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

    // Needs the German model installed; run with `cargo test --lib -- --ignored`.
    #[test]
    #[ignore]
    fn translates_real_cues_with_installed_model() {
        let translator = CueTranslator::load("de", 4).expect("German model installed");
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

    #[test]
    fn spread_words_handles_empty_input() {
        assert!(spread_words("", &[cue(0.0, 1.0, "x")]).is_empty());
        assert!(spread_words("x", &[]).is_empty());
    }
}
