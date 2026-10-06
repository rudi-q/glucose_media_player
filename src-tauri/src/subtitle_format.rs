// Builds readable subtitle cues from Whisper's word timings, following the Netflix Timed
// Text Style Guide for English: at most 2 lines of 42 characters, at most 7 s and at
// least 5/6 s on screen, a 2-frame gap between cues, and a reading speed of at most
// 20 characters per second where the timing allows it. Fast verbatim speech can still
// exceed 20 CPS, since meeting it would need the text to be condensed.
// Used by both live and batch subtitle generation.

use whisper_rs::{FullParams, WhisperState};

pub(crate) const MAX_LINE_CHARS: usize = 42;
const MAX_CUE_CHARS: usize = MAX_LINE_CHARS * 2;
pub(crate) const MAX_CPS: f64 = 20.0;
pub(crate) const MIN_DURATION: f64 = 5.0 / 6.0;
pub(crate) const MAX_DURATION: f64 = 7.0;
// Two frames at 24 fps.
pub(crate) const MIN_GAP: f64 = 2.0 / 24.0;
// A silence this long always ends a cue, so text never stays up across it.
const SILENCE_BREAK: f64 = 1.0;
// A finished sentence ends the cue once the cue is at least this long; shorter
// sentences are kept together with the next one.
const SENTENCE_MIN_SECS: f64 = 2.0;
const SENTENCE_MIN_CHARS: usize = 30;
// Cues shorter than both of these are merged into a neighbour when possible.
const ORPHAN_MAX_CHARS: usize = 15;
// Speech ending this close to the end of a chunk may continue into the next chunk.
const BOUNDARY_SPEECH: f64 = 0.5;
// A chunk is never rewound so far that it covers less than this.
const MIN_CHUNK_ADVANCE: f64 = 5.0;

// Words a line or cue should not end on, since they belong with what follows.
const WEAK_ENDINGS: &[&str] = &[
    "a", "an", "the", "of", "to", "in", "on", "at", "for", "with", "by", "from", "and", "or",
    "but", "that", "which", "who", "is", "are", "was", "were", "my", "your", "his", "her",
    "their", "our", "its", "this", "these", "those", "as", "if", "so", "than", "into",
];

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Word {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

// Makes Whisper return one segment per word, with timestamps, so cues can be cut at
// sentence and clause boundaries instead of at a fixed length. Whisper only splits
// segments when token timestamps are on.
pub(crate) fn request_word_timestamps(params: &mut FullParams) {
    params.set_token_timestamps(true);
    params.set_max_len(1);
    params.set_split_on_word(true);
}

// Reads the word segments produced with `request_word_timestamps`. Times are in
// seconds, shifted by `offset` and clamped to `[0, duration]` before shifting.
pub(crate) fn words_from_state(state: &WhisperState, offset: f64, duration: f64) -> Vec<Word> {
    let mut words: Vec<Word> = Vec::new();
    for i in 0..state.full_n_segments() {
        let Some(segment) = state.get_segment(i) else {
            continue;
        };
        let raw = segment.to_str_lossy().map(|t| t.into_owned()).unwrap_or_default();
        let text = raw.trim();
        if text.is_empty() {
            continue;
        }
        let start = (segment.start_timestamp() as f64 / 100.0).clamp(0.0, duration);
        let end = (segment.end_timestamp() as f64 / 100.0).clamp(start, duration);
        // Punctuation Whisper emits as its own piece belongs to the previous word.
        let attaches = !raw.starts_with(' ') && text.starts_with(|c: char| c.is_ascii_punctuation());
        if let (true, Some(prev)) = (attaches, words.last_mut()) {
            prev.text.push_str(text);
            prev.end = prev.end.max(offset + end);
            continue;
        }
        words.push(Word {
            start: offset + start,
            end: offset + end,
            text: text.to_string(),
        });
    }
    words
}

// Groups words into cues, merges fragments, applies the timing rules and breaks lines.
// `limit` is the latest time any cue may extend to (the end of the transcribed audio).
pub(crate) fn build_cues(words: &[Word], limit: f64) -> Vec<Cue> {
    let groups = merge_orphans(words, segment(words));
    let mut cues: Vec<Cue> = groups
        .into_iter()
        .map(|(i, j)| Cue {
            start: words[i].start,
            end: words[j].end,
            text: join(&words[i..=j]),
        })
        .collect();
    retime(&mut cues, limit);
    for cue in &mut cues {
        cue.text = break_lines(&cue.text);
    }
    cues
}

// When speech runs up to the end of a chunk, the chunk's last cue may be cut off
// mid-phrase. Returns the time the next chunk should start from (that cue's start), so
// the cue is transcribed whole there; the caller drops cues from that time on.
pub(crate) fn resume_point(words: &[Word], cues: &[Cue], chunk_start: f64, chunk_end: f64) -> Option<f64> {
    let last_word = words.last()?;
    if chunk_end - last_word.end > BOUNDARY_SPEECH {
        return None;
    }
    let resume = cues.last()?.start;
    (resume - chunk_start >= MIN_CHUNK_ADVANCE).then_some(resume)
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

fn join(words: &[Word]) -> String {
    words.iter().map(|w| w.text.as_str()).collect::<Vec<_>>().join(" ")
}

fn ends_sentence(word: &Word) -> bool {
    word.text
        .trim_end_matches(['"', '\'', ')', ']'])
        .ends_with(['.', '?', '!'])
}

fn ends_clause(word: &Word) -> bool {
    word.text.ends_with([',', ';', ':']) || word.text.ends_with(['-', '\u{2014}'])
}

fn weak_ending(word: &Word) -> bool {
    let bare = word.text.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
    WEAK_ENDINGS.contains(&bare.as_str())
}

fn gap_after(words: &[Word], k: usize) -> f64 {
    words.get(k + 1).map(|n| n.start - words[k].end).unwrap_or(f64::INFINITY)
}

// Whether words[i..=j] can be one cue: within the length and duration limits and
// breakable into at most two lines of 42 characters.
fn fits(words: &[Word], i: usize, j: usize) -> bool {
    let text = join(&words[i..=j]);
    char_len(&text) <= MAX_CUE_CHARS
        && words[j].end - words[i].start <= MAX_DURATION
        && break_lines(&text).split('\n').all(|l| char_len(l) <= MAX_LINE_CHARS)
}

// Splits words into cues as (first, last) index pairs. A cue ends at a long silence or
// a finished sentence; when the next word would not fit, the best earlier break is
// chosen by `break_score`.
fn segment(words: &[Word]) -> Vec<(usize, usize)> {
    let mut groups = Vec::new();
    let mut i = 0;
    while i < words.len() {
        let mut j = i;
        while j + 1 < words.len() {
            if gap_after(words, j) >= SILENCE_BREAK {
                break;
            }
            let long_enough = words[j].end - words[i].start >= SENTENCE_MIN_SECS
                || char_len(&join(&words[i..=j])) >= SENTENCE_MIN_CHARS;
            if ends_sentence(&words[j]) && long_enough {
                break;
            }
            if !fits(words, i, j + 1) {
                j = (i..=j)
                    .max_by(|&a, &b| break_score(words, i, a).total_cmp(&break_score(words, i, b)))
                    .unwrap_or(j);
                break;
            }
            j += 1;
        }
        groups.push((i, j));
        i = j + 1;
    }
    groups
}

// How good a place the end of words[k] is to end a cue that starts at words[i].
fn break_score(words: &[Word], i: usize, k: usize) -> f64 {
    let chars = char_len(&join(&words[i..=k]));
    let duration = words[k].end - words[i].start;
    let mut score = 40.0 * chars as f64 / MAX_CUE_CHARS as f64;
    if ends_sentence(&words[k]) {
        score += 60.0;
    } else if ends_clause(&words[k]) {
        score += 35.0;
    }
    score += 40.0 * gap_after(words, k).min(1.0);
    if weak_ending(&words[k]) {
        score -= 25.0;
    }
    if chars < ORPHAN_MAX_CHARS && duration < MIN_DURATION {
        score -= 100.0;
    }
    score
}

// Folds cues that are both very short and on screen too briefly into the neighbour they
// are closest to in time, as long as the result still fits.
fn merge_orphans(words: &[Word], mut groups: Vec<(usize, usize)>) -> Vec<(usize, usize)> {
    let is_orphan = |(i, j): (usize, usize)| {
        char_len(&join(&words[i..=j])) < ORPHAN_MAX_CHARS && words[j].end - words[i].start < MIN_DURATION
    };
    let mut k = 0;
    while k < groups.len() {
        if !is_orphan(groups[k]) {
            k += 1;
            continue;
        }
        let (i, j) = groups[k];
        let gap_before = (k > 0).then(|| words[i].start - words[groups[k - 1].1].end);
        let gap_next = groups.get(k + 1).map(|&(ni, _)| words[ni].start - words[j].end);
        let can_prev = gap_before.is_some_and(|g| g < SILENCE_BREAK) && fits(words, groups[k - 1].0, j);
        let can_next = gap_next.is_some_and(|g| g < SILENCE_BREAK) && fits(words, i, groups[k + 1].1);
        let prefer_prev = match (gap_before, gap_next) {
            (Some(b), Some(n)) => b <= n,
            _ => true,
        };
        if can_prev && (prefer_prev || !can_next) {
            groups[k - 1].1 = j;
            groups.remove(k);
        } else if can_next {
            groups[k + 1].0 = i;
            groups.remove(k);
        } else {
            k += 1;
        }
    }
    groups
}

// Applies the minimum and maximum duration and the reading speed target, extending a
// cue into the following silence when needed but never into the next cue or past
// `limit`, and keeping a 2-frame gap before the next cue.
fn retime(cues: &mut [Cue], limit: f64) {
    for i in 0..cues.len() {
        let next_start = cues.get(i + 1).map(|n| n.start).unwrap_or(f64::INFINITY);
        let cue = &mut cues[i];
        let reading = char_len(&cue.text) as f64 / MAX_CPS;
        let wanted = cue.start + (cue.end - cue.start).max(MIN_DURATION).max(reading);
        let ceiling = (next_start - MIN_GAP).min(limit).min(cue.start + MAX_DURATION);
        // Never shorten a cue below what Whisper reported unless it would overlap the
        // next one or run past the audio.
        let floor = cue.end.min(ceiling);
        cue.end = wanted.min(ceiling).max(floor);
        if cue.end <= cue.start {
            cue.end = (cue.start + 0.01).min(next_start);
        }
    }
}

// Breaks text longer than one line into two lines, as evenly as possible, preferring a
// break after punctuation and avoiding a line that ends on a word like "the".
pub(crate) fn break_lines(text: &str) -> String {
    let len = char_len(text);
    if len <= MAX_LINE_CHARS {
        return text.to_string();
    }
    let words: Vec<&str> = text.split(' ').collect();
    let mut best: Option<(f64, usize)> = None;
    let mut first = 0;
    for i in 1..words.len() {
        first += char_len(words[i - 1]) + usize::from(i > 1);
        let second = len - first - 1;
        let overflow = first.saturating_sub(MAX_LINE_CHARS) + second.saturating_sub(MAX_LINE_CHARS);
        let mut score = overflow as f64 * 100.0 + first.abs_diff(second) as f64;
        let last = words[i - 1];
        if last.ends_with([',', '.', '?', '!', ';', ':']) {
            score -= 8.0;
        }
        let bare = last.trim_matches(|c: char| !c.is_alphanumeric()).to_lowercase();
        if WEAK_ENDINGS.contains(&bare.as_str()) {
            score += 6.0;
        }
        if best.is_none_or(|(s, _)| score < s) {
            best = Some((score, i));
        }
    }
    match best {
        Some((_, i)) => format!("{}\n{}", words[..i].join(" "), words[i..].join(" ")),
        None => text.to_string(),
    }
}

// Counts how many cues break each rule. Used by tests and examples/format_check.rs.
#[allow(dead_code)]
#[derive(Debug, Default, PartialEq)]
pub(crate) struct Violations {
    pub over_two_lines: usize,
    pub line_too_long: usize,
    pub too_short: usize,
    pub too_long: usize,
    pub too_fast: usize,
    pub gap_too_small: usize,
}

#[allow(dead_code)]
pub(crate) fn check(cues: &[Cue]) -> Violations {
    const EPS: f64 = 1e-6;
    let mut v = Violations::default();
    for (k, cue) in cues.iter().enumerate() {
        let lines: Vec<&str> = cue.text.split('\n').collect();
        let duration = cue.end - cue.start;
        v.over_two_lines += usize::from(lines.len() > 2);
        v.line_too_long += usize::from(lines.iter().any(|l| char_len(l) > MAX_LINE_CHARS));
        v.too_short += usize::from(duration < MIN_DURATION - EPS);
        v.too_long += usize::from(duration > MAX_DURATION + EPS);
        v.too_fast += usize::from(char_len(&cue.text) as f64 / duration > MAX_CPS + EPS);
        if let Some(next) = cues.get(k + 1) {
            v.gap_too_small += usize::from(next.start - cue.end < MIN_GAP - EPS);
        }
    }
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    // Builds words spaced evenly at `secs_per_word`, with `pause` seconds of silence
    // inserted after any word ending in "|".
    fn words(text: &str, start: f64, secs_per_word: f64) -> Vec<Word> {
        let mut t = start;
        text.split(' ')
            .map(|w| {
                let pause = w.ends_with('|');
                let word = Word {
                    start: t,
                    end: t + secs_per_word * 0.9,
                    text: w.trim_end_matches('|').to_string(),
                };
                t += secs_per_word + if pause { 1.5 } else { 0.0 };
                word
            })
            .collect()
    }

    fn texts(cues: &[Cue]) -> Vec<String> {
        cues.iter().map(|c| c.text.replace('\n', " ")).collect()
    }

    #[test]
    fn short_sentence_gets_minimum_duration() {
        let cues = build_cues(&words("Hi.", 1.0, 0.2), 100.0);
        assert_eq!(texts(&cues), vec!["Hi."]);
        assert!((cues[0].end - (1.0 + MIN_DURATION)).abs() < 1e-9);
    }

    #[test]
    fn cues_end_at_sentences_not_at_a_fixed_length() {
        let text = "Teaching a machine to do that exact same thing is without a doubt one of the \
                    greatest technological challenges of our time. You've probably heard the term \
                    full stack developer before.";
        let cues = build_cues(&words(text, 0.0, 0.3), 1000.0);
        assert!(cues.iter().all(|c| !c.text.ends_with("the")), "{:?}", texts(&cues));
        let joined = texts(&cues).join(" ");
        assert_eq!(joined, text.split_whitespace().collect::<Vec<_>>().join(" "));
        // The second sentence starts a new cue.
        assert!(texts(&cues).iter().any(|t| t.starts_with("You've")));
        assert_eq!(check(&cues).line_too_long, 0);
        assert_eq!(check(&cues).over_two_lines, 0);
    }

    #[test]
    fn no_orphan_fragments() {
        // "the" would otherwise be left on its own at the end.
        let text = "It's a highly dynamic physical signal that is literally shaped by the \
                    constraints of the human body and the";
        let cues = build_cues(&words(text, 0.0, 0.25), 1000.0);
        assert!(cues.iter().all(|c| char_len(&c.text) >= ORPHAN_MAX_CHARS || c.end - c.start >= MIN_DURATION),
            "{:?}", texts(&cues));
    }

    #[test]
    fn silence_ends_a_cue() {
        let cues = build_cues(&words("Wait for it| and now the rest", 0.0, 0.3), 100.0);
        assert_eq!(texts(&cues), vec!["Wait for it", "and now the rest"]);
    }

    #[test]
    fn text_that_cannot_fit_two_lines_is_split() {
        let text = "understand the ethical, moral, and societal impacts of the systems you're putting out";
        let cues = build_cues(&words(text, 0.0, 0.2), 100.0);
        let v = check(&cues);
        assert_eq!(v.line_too_long, 0, "{:?}", texts(&cues));
        assert_eq!(v.over_two_lines, 0);
    }

    #[test]
    fn long_speech_is_split_at_seven_seconds() {
        let cues = build_cues(&words("one two three four five six seven eight nine ten", 0.0, 1.2), 100.0);
        assert!(cues.len() >= 2);
        assert_eq!(check(&cues).too_long, 0);
    }

    #[test]
    fn reading_speed_extends_into_silence_but_not_into_next_cue() {
        // 60 characters needs 3 s at 20 CPS.
        let long = Word { start: 0.0, end: 1.0, text: "a".repeat(29) + " " + &"b".repeat(30) };
        let next = Word { start: 2.0, end: 3.0, text: "Next.".into() };
        let cues = build_cues(&[long.clone(), next], 100.0);
        assert!((cues[0].end - (2.0 - MIN_GAP)).abs() < 1e-9);
        let alone = build_cues(&[long], 100.0);
        assert!((alone[0].end - 3.0).abs() < 1e-9);
    }

    #[test]
    fn last_cue_stops_before_limit() {
        let cues = build_cues(&words("Right at the end.", 0.0, 0.5), 2.0 - MIN_GAP);
        assert!(cues.last().unwrap().end <= 2.0 - MIN_GAP + 1e-9);
    }

    #[test]
    fn resume_point_rewinds_only_when_speech_reaches_the_chunk_end() {
        let text = "First sentence is here and it is long enough. Then speech runs into the";
        let w = words(text, 0.0, 0.7);
        let end = w.last().unwrap().end + 0.1;
        let cues = build_cues(&w, end);
        let resume = resume_point(&w, &cues, 0.0, end).expect("should rewind");
        assert_eq!(resume, cues.last().unwrap().start);
        assert!(texts(&cues).last().unwrap().starts_with("Then"));
        // Silence before the chunk end: nothing is cut off.
        assert_eq!(resume_point(&w, &cues, 0.0, end + 2.0), None);
        // Rewinding would leave less than the minimum advance.
        assert_eq!(resume_point(&w, &cues, resume - 1.0, end), None);
    }

    #[test]
    fn break_prefers_punctuation_over_slightly_better_balance() {
        let text = "I told him twice already, but he never listens to anyone";
        assert_eq!(break_lines(text), "I told him twice already,\nbut he never listens to anyone");
    }

    #[test]
    fn check_counts_violations() {
        let cues = vec![
            Cue { start: 0.0, end: 0.5, text: "a".repeat(43) },
            Cue { start: 0.51, end: 9.0, text: "x\ny\nz".into() },
        ];
        let v = check(&cues);
        assert_eq!(v.line_too_long, 1);
        assert_eq!(v.too_short, 1);
        assert_eq!(v.too_fast, 1);
        assert_eq!(v.gap_too_small, 1);
        assert_eq!(v.over_two_lines, 1);
        assert_eq!(v.too_long, 1);
    }
}
