// Shapes Whisper segments into readable subtitle cues following the Netflix Timed Text
// Style Guide for English: at most 2 lines of 42 characters, at most 20 characters per
// second, between 5/6 s and 7 s on screen, and a 2-frame gap between cues.
// Used by both live and batch subtitle generation.

use whisper_rs::FullParams;

pub(crate) const MAX_LINE_CHARS: usize = 42;
pub(crate) const MAX_LINES: usize = 2;
const MAX_CUE_CHARS: usize = MAX_LINE_CHARS * MAX_LINES;
const MAX_CPS: f64 = 20.0;
const MIN_DURATION: f64 = 5.0 / 6.0;
const MAX_DURATION: f64 = 7.0;
// Two frames at 24 fps.
const MIN_GAP: f64 = 2.0 / 24.0;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

// Makes Whisper cut segments at word boundaries once they reach two lines' worth of
// text, so most cues already fit before `conform` runs. Whisper only honours max_len
// when token timestamps are on.
pub(crate) fn apply_segment_limits(params: &mut FullParams) {
    params.set_token_timestamps(true);
    params.set_max_len(MAX_CUE_CHARS as i32);
    params.set_split_on_word(true);
}

// Splits, line-breaks and retimes cues to fit the limits above. `limit` is the latest
// time any cue may extend to (the end of the transcribed audio).
pub(crate) fn conform(cues: Vec<Cue>, limit: f64) -> Vec<Cue> {
    let mut out: Vec<Cue> = cues
        .into_iter()
        .filter_map(|c| {
            let text = c.text.split_whitespace().collect::<Vec<_>>().join(" ");
            (!text.is_empty() && c.end > c.start).then_some(Cue { text, ..c })
        })
        .flat_map(split_cue)
        .collect();
    out.sort_by(|a, b| a.start.total_cmp(&b.start));
    retime(&mut out, limit);
    for cue in &mut out {
        cue.text = break_lines(&cue.text);
    }
    out
}

fn char_len(text: &str) -> usize {
    text.chars().count()
}

// Splits a cue that holds more than two lines of text or lasts longer than the maximum
// duration into consecutive cues, dividing words evenly by length and time in
// proportion to text.
fn split_cue(cue: Cue) -> Vec<Cue> {
    let len = char_len(&cue.text);
    let duration = cue.end - cue.start;
    let parts = len
        .div_ceil(MAX_CUE_CHARS)
        .max((duration / MAX_DURATION).ceil() as usize)
        .max(1);
    let words: Vec<&str> = cue.text.split(' ').collect();
    if parts == 1 || words.len() < 2 {
        return vec![cue];
    }
    let groups = split_words(&words, parts.min(words.len()));
    let total: usize = groups.iter().map(|g| char_len(g)).sum();
    let mut start = cue.start;
    let mut seen = 0;
    groups
        .into_iter()
        .map(|text| {
            seen += char_len(&text);
            let end = cue.start + duration * seen as f64 / total as f64;
            let part = Cue { start, end, text };
            start = end;
            part
        })
        .collect()
}

// Divides words into about `parts` groups of roughly equal character count. A group
// never exceeds two lines' worth of text, even if that takes an extra group.
fn split_words(words: &[&str], parts: usize) -> Vec<String> {
    // Sizes count one separator per word, so a group's text length is `size - 1`.
    let total: usize = words.iter().map(|w| char_len(w) + 1).sum();
    let target = total as f64 / parts as f64;
    let mut groups = Vec::with_capacity(parts);
    let mut current: Vec<&str> = Vec::new();
    let mut size = 0;
    for word in words {
        let w = char_len(word) + 1;
        if !current.is_empty() {
            // Close the group if it is nearer the target without this word.
            let nearer_without = (size + w) as f64 - target > target - size as f64;
            let room_left = groups.len() + 1 < parts;
            if (nearer_without && room_left) || size + w > MAX_CUE_CHARS + 1 {
                groups.push(current.join(" "));
                current.clear();
                size = 0;
            }
        }
        current.push(word);
        size += w;
    }
    if !current.is_empty() {
        groups.push(current.join(" "));
    }
    groups
}

// Applies the minimum and maximum duration and the reading speed limit, extending a cue
// into the following silence when needed but never into the next cue or past `limit`.
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
// break after punctuation. Text that cannot fit two lines is broken as evenly as possible.
fn break_lines(text: &str) -> String {
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
        if words[i - 1].ends_with([',', '.', '?', '!', ';', ':']) {
            score -= 8.0;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn cue(start: f64, end: f64, text: &str) -> Cue {
        Cue { start, end, text: text.to_string() }
    }

    fn lines(text: &str) -> Vec<usize> {
        text.split('\n').map(char_len).collect()
    }

    #[test]
    fn short_cue_is_unchanged_apart_from_minimum_duration() {
        let out = conform(vec![cue(1.0, 1.2, "Hi.")], 100.0);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].text, "Hi.");
        assert!((out[0].end - (1.0 + MIN_DURATION)).abs() < 1e-9);
    }

    #[test]
    fn long_line_is_broken_into_two_balanced_lines() {
        let text = "I thought he caught a cold after walking home in the winter.";
        let out = conform(vec![cue(0.0, 4.0, text)], 100.0);
        assert_eq!(out.len(), 1);
        let l = lines(&out[0].text);
        assert_eq!(l.len(), 2);
        assert!(l.iter().all(|&n| n <= MAX_LINE_CHARS));
        assert_eq!(out[0].text.replace('\n', " "), text);
    }

    #[test]
    fn break_prefers_punctuation_over_slightly_better_balance() {
        // Without the punctuation preference, "...already, but" / "he never..." is
        // more balanced (29/26 vs 25/30).
        let text = "I told him twice already, but he never listens to anyone";
        assert_eq!(
            break_lines(text),
            "I told him twice already,\nbut he never listens to anyone"
        );
    }

    #[test]
    fn overlong_cue_is_split_and_every_part_fits() {
        let text = "This is a very long segment that Whisper produced without any limit at all, \
                    and it keeps going well past what two subtitle lines could ever hold on screen.";
        let out = conform(vec![cue(10.0, 18.0, text)], 100.0);
        assert!(out.len() >= 2);
        for c in &out {
            let l = lines(&c.text);
            assert!(l.len() <= MAX_LINES, "{:?}", c.text);
            assert!(l.iter().all(|&n| n <= MAX_LINE_CHARS), "{:?}", c.text);
            assert!(c.end - c.start <= MAX_DURATION + 1e-9);
        }
        let joined: Vec<String> = out.iter().map(|c| c.text.replace('\n', " ")).collect();
        assert_eq!(joined.join(" "), text.split_whitespace().collect::<Vec<_>>().join(" "));
        assert!((out[0].start - 10.0).abs() < 1e-9);
    }

    #[test]
    fn long_duration_is_split_at_seven_seconds() {
        let out = conform(vec![cue(0.0, 15.0, "one two three four five six")], 100.0);
        assert_eq!(out.len(), 3);
        assert!(out.iter().all(|c| c.end - c.start <= MAX_DURATION + 1e-9));
    }

    #[test]
    fn reading_speed_extends_into_silence_but_not_into_next_cue() {
        // 60 characters needs 3 s at 20 CPS.
        let text = "a".repeat(29) + " " + &"b".repeat(30);
        let out = conform(vec![cue(0.0, 1.0, &text), cue(2.0, 3.0, "Next.")], 100.0);
        assert!((out[0].end - (2.0 - MIN_GAP)).abs() < 1e-9);
        let alone = conform(vec![cue(0.0, 1.0, &text)], 100.0);
        assert!((alone[0].end - 3.0).abs() < 1e-9);
    }

    #[test]
    fn cues_never_extend_past_limit_or_overlap() {
        let out = conform(
            vec![cue(0.0, 2.5, "First line here."), cue(2.4, 3.0, "Second.")],
            3.2,
        );
        assert!(out[0].end <= out[1].start);
        assert!(out[1].end <= 3.2 + 1e-9);
    }
}
