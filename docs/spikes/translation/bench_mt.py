"""Subtitle translation spike: speed, memory and output of OPUS-MT models on real
Whisper cues from the live subtitle cache.

Setup (outside the repo; any Python 3.10+):
    python -m venv %TEMP%\\mtb
    %TEMP%\\mtb\\Scripts\\pip install torch --index-url https://download.pytorch.org/whl/cpu
    %TEMP%\\mtb\\Scripts\\pip install ctranslate2 "transformers<5" sentencepiece sacremoses psutil sacrebleu

Usage:
    python bench_mt.py --cache <live-cache json> --out results/
    python bench_mt.py --one <model id> --cache ... --out ...   (internal: one model per process)

Each model runs in its own process so memory numbers are not mixed. Models are converted
to CTranslate2 int8 once and kept in --models-dir.
"""

import argparse
import json
import os
import re
import statistics
import subprocess
import sys
import time

MODELS = {
    "Helsinki-NLP/opus-mt_tiny_eng-deu": "de",
    "Helsinki-NLP/opus-mt-en-de": "de",
    "Helsinki-NLP/opus-mt_tiny_eng-fra": "fr",
    "Helsinki-NLP/opus-mt-en-fr": "fr",
    "Helsinki-NLP/opus-mt-tc-big-en-fr": "fr",
}
# Treated as the quality reference for its language when scoring the others.
REFERENCE = {"fr": "Helsinki-NLP/opus-mt-tc-big-en-fr"}
THREADS = 4  # leaves most cores for Whisper and video decoding


def load_cues(path):
    data = json.load(open(path, encoding="utf-8"))
    return [{"start": c["start"], "end": c["end"], "text": c["text"].replace("\n", " ")} for c in data["cues"]]


def sentences_from(cues):
    """Joins cue texts and splits them into sentences, as a translator would see them."""
    text = " ".join(c["text"] for c in cues)
    return [s for s in re.split(r"(?<=[.?!])\s+", text) if s.strip()]


def convert(model_id, models_dir):
    out = os.path.join(models_dir, model_id.replace("/", "__"))
    if not os.path.exists(os.path.join(out, "model.bin")):
        import ctranslate2

        ctranslate2.converters.TransformersConverter(model_id).convert(out, quantization="int8", force=True)
    return out


def dir_size_mb(path):
    return sum(os.path.getsize(os.path.join(r, f)) for r, _, fs in os.walk(path) for f in fs) / 1e6


def run_one(model_id, cues, models_dir):
    import ctranslate2
    import psutil
    import transformers

    proc = psutil.Process()
    ct2_dir = convert(model_id, models_dir)
    base_rss = proc.memory_info().rss

    t0 = time.perf_counter()
    tokenizer = transformers.AutoTokenizer.from_pretrained(model_id)
    translator = ctranslate2.Translator(ct2_dir, device="cpu", inter_threads=1, intra_threads=THREADS)
    load_s = time.perf_counter() - t0

    def translate(texts, batch):
        tokens = [tokenizer.convert_ids_to_tokens(tokenizer.encode(t)) for t in texts]
        results = translator.translate_batch(tokens, max_batch_size=batch, beam_size=2)
        return [tokenizer.decode(tokenizer.convert_tokens_to_ids(r.hypotheses[0]), skip_special_tokens=True) for r in results]

    translate(["Warm up."], 1)

    # Cue by cue, one at a time: the worst case for live mode.
    per_cue_ms, cue_out = [], []
    for c in cues:
        t = time.perf_counter()
        cue_out.append(translate([c["text"]], 1)[0])
        per_cue_ms.append((time.perf_counter() - t) * 1000)

    # Whole sentences, batched per ~20 s chunk worth of text: what live mode would do.
    sents = sentences_from(cues)
    t = time.perf_counter()
    sent_out = translate(sents, 16)
    sent_total_s = time.perf_counter() - t

    words = sum(len(s.split()) for s in sents)
    return {
        "model": model_id,
        "lang": MODELS[model_id],
        "size_mb_int8": round(dir_size_mb(ct2_dir), 1),
        "load_s": round(load_s, 2),
        "rss_mb": round((proc.memory_info().rss - base_rss) / 1e6, 1),
        "cue_ms_mean": round(statistics.mean(per_cue_ms), 1),
        "cue_ms_p95": round(sorted(per_cue_ms)[int(len(per_cue_ms) * 0.95)], 1),
        "sentences": len(sents),
        "sentences_total_s": round(sent_total_s, 2),
        "words_per_s": round(words / sent_total_s),
        "cue_out": cue_out,
        "sent_in": sents,
        "sent_out": sent_out,
    }


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--cache", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--models-dir", default=os.path.join(os.environ.get("TEMP", "."), "mtb", "models"))
    ap.add_argument("--one")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    cues = load_cues(args.cache)

    if args.one:
        res = run_one(args.one, cues, args.models_dir)
        name = args.one.replace("/", "__")
        json.dump(res, open(os.path.join(args.out, f"{name}.json"), "w", encoding="utf-8"), ensure_ascii=False, indent=1)
        return

    results = {}
    for model_id in MODELS:
        print(f"== {model_id}", flush=True)
        subprocess.run([sys.executable, __file__, "--one", model_id, "--cache", args.cache, "--out", args.out,
                        "--models-dir", args.models_dir], check=True)
        results[model_id] = json.load(open(os.path.join(args.out, model_id.replace("/", "__") + ".json"), encoding="utf-8"))

    # Agreement with the reference model (chrF), a proxy for quality where no human
    # reference translation exists.
    import sacrebleu

    for res in results.values():
        ref_id = REFERENCE.get(res["lang"])
        if ref_id and ref_id != res["model"]:
            ref = results[ref_id]["sent_out"]
            res["chrf_vs_reference"] = round(sacrebleu.corpus_chrf(res["sent_out"], [ref]).score, 1)

    keys = ["size_mb_int8", "load_s", "rss_mb", "cue_ms_mean", "cue_ms_p95", "words_per_s", "chrf_vs_reference"]
    print("\n" + "model".ljust(38) + "".join(k.rjust(18) for k in keys))
    for res in results.values():
        print(res["model"].split("/")[1].ljust(38) + "".join(str(res.get(k, "-")).rjust(18) for k in keys))
    json.dump({k: {kk: v for kk, v in r.items() if kk not in ("cue_out", "sent_in", "sent_out")} for k, r in results.items()},
              open(os.path.join(args.out, "summary.json"), "w"), indent=1)

    # Side-by-side sample for human judgement.
    first = next(iter(results.values()))
    picks = list(range(0, len(first["sent_in"]), max(1, len(first["sent_in"]) // 15)))[:15]
    lines = ["# Translation samples (LingDig talk, Whisper small-q5_1)", ""]
    for i in picks:
        lines.append(f"**EN:** {first['sent_in'][i]}")
        for res in results.values():
            lines.append(f"- `{res['model'].split('/')[1]}`: {res['sent_out'][i]}")
        lines.append("")
    open(os.path.join(args.out, "samples.md"), "w", encoding="utf-8").write("\n".join(lines))
    print(f"\nSamples written to {os.path.join(args.out, 'samples.md')}")


if __name__ == "__main__":
    main()
