// Names subtitle tracks by language, from what the track already says about itself: an
// embedded track's language tag, or the language tag in a subtitle file's name
// ("Movie.2021.en.srt", "Movie.English.SDH.srt", "Subs/2_Deutsch.srt"). Subtitle text is
// never read for this. A file without a tag is named by its file name instead.

const englishNames = new Intl.DisplayNames(["en"], { type: "language", fallback: "none" });

// Words that mark a variant of a track rather than its language.
const VARIANTS: Record<string, string> = {
  forced: "Forced",
  sdh: "SDH",
  hi: "SDH",
  cc: "SDH",
};
// Words that say nothing about the language and may sit between it and the extension.
const FILLER = new Set(["full", "default", "subs", "sub", "subtitles", "subtitle", "track"]);

function fold(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

// The English name of a language code (ISO 639-1, or -2 such as "ger" or "deu").
export function languageName(code: string | null | undefined): string | null {
  if (!code) return null;
  try {
    return englishNames.of(code.trim().toLowerCase()) ?? null;
  } catch {
    return null;
  }
}

// Language names, in English and in the language itself, mapped to their English name.
// Built once from every two-letter code the runtime knows.
let namesByWord: Map<string, string> | null = null;

function nameForWord(word: string): string | null {
  if (!namesByWord) {
    namesByWord = new Map();
    const letters = "abcdefghijklmnopqrstuvwxyz";
    for (const a of letters) {
      for (const b of letters) {
        const code = a + b;
        const english = languageName(code);
        if (!english) continue;
        namesByWord.set(fold(english), english);
        try {
          const native = new Intl.DisplayNames([code], { type: "language", fallback: "none" }).of(code);
          if (native) namesByWord.set(fold(native), english);
        } catch {
          // A code the runtime names but has no locale data for.
        }
      }
    }
  }
  return namesByWord.get(fold(word)) ?? null;
}

export interface FileLanguage {
  language: string | null;
  variant: string | null;
}

// Reads the language tag at the end of a subtitle file name, after any variant words
// ("forced", "SDH"). Only the last word counts, so "The.English.Patient.srt" is not
// English. Two- and three-letter codes only count after another word, so a film called
// "It" stays a title. "hi" is read as hearing impaired, as subtitle files use it.
export function languageFromFileName(fileName: string): FileLanguage {
  const stem = fileName.replace(/\.[^.\\/]+$/, "");
  // Each word with what came right before it, which tells "pt-BR" from "fr.de".
  const parts = [...stem.matchAll(/[\p{L}\p{N}]+/gu)];
  const words = parts.map((m) => m[0]);
  const joinedByHyphen = (i: number) => {
    const prev = parts[i - 1];
    return stem.slice(prev.index + prev[0].length, parts[i].index) === "-";
  };
  let variant: string | null = null;
  for (let i = words.length - 1; i >= 0; i--) {
    const word = words[i].toLowerCase();
    if (VARIANTS[word]) {
      variant ??= VARIANTS[word];
      continue;
    }
    if (FILLER.has(word)) continue;
    const isCode = /^[a-z]{2,3}$/.test(word);
    // A tag with a region or script ("pt-BR", "zh-Hant") is read whole, so
    // "Movie.pt-BR.srt" is Brazilian Portuguese rather than Breton. Only a hyphen joins
    // them: in "Movie.fr.de.srt" the last tag, German, stands on its own.
    if (
      /^[a-z]{2,4}$/.test(word) &&
      i > 1 &&
      joinedByHyphen(i) &&
      /^[a-z]{2,3}$/i.test(words[i - 1])
    ) {
      const base = words[i - 1].toLowerCase();
      const full = languageName(base) ? languageName(`${base}-${words[i]}`) : null;
      if (full) return { language: full, variant };
    }
    const language = isCode ? (i > 0 ? languageName(word) : null) : nameForWord(word);
    return { language, variant };
  }
  return { language: null, variant };
}

// A file name made readable, for files without a language: "notes_final.srt" -> "notes final".
export function readableFileName(fileName: string): string {
  const stem = fileName.replace(/\.[^.\\/]+$/, "");
  return stem.replace(/[._]+/g, " ").replace(/\s+/g, " ").trim() || fileName;
}
