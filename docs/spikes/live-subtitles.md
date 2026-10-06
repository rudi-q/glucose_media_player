# Spike: Real-time subtitle generation

Branch: `spike/live-subtitles`

## Status and how to resume

_Last updated 2026-10-06._

**Where things stand:** live mode and the live cache have been tried in the app and work so far. Step 1 is partly done (gate passes on a high-end machine only). A first in-app version covering steps 3-5 is built and passes `pnpm check` and `pnpm tauri:check`, but has **not yet been tried in the running app**. Seek and audio-track restarts and backpressure (from step 7) are included; the seek-bar strip and status chip (step 6), the playback impact check (step 8) and "Save as SRT" (step 9b) are not. The live cache (step 9a) is built and type-checks but is also untested in the app. The `whisper-rs` abort bug that caused random -6 errors is fixed.

**What exists:**

- This plan, with the step 1 results under Findings.
- `src-tauri/examples/live_bench.rs`, the benchmark harness (standalone Cargo example, does not touch the app).
- `src-tauri/src/live_subtitles.rs`, the live worker. Commands `start_live_subtitles`, `stop_live_subtitles(unload)` and `update_live_subtitles_playhead`; events `live-subtitle-chunk` (cues plus covered range) and `live-subtitle-status`. Picks the first installed model from small-q5_1, small, base, tiny. 20 s windows with the cut moved to the quietest 50 ms frame in the last 3 s, 8-thread cap, `no_context` on, pauses at 180 s ahead of the playhead.
- `src-tauri/src/live_cache.rs`, commands `load_live_cache` and `save_live_cache`: one JSON file per video, audio track and language, with key check, model-rank check, atomic writes and 50 MB eviction.
- `src/lib/subtitle/liveSubtitles.ts`, event and cache types plus covered-range and cue-merge helpers.
- `SubtitleOverlay.svelte` takes optional `liveCues` (replaces the loaded track) and `placeholder` ("Generating subtitles..." when the playhead is in an uncovered range).
- Player page: "Live (real-time)" at the top of the Select AI Model menu; while on, a "Live subtitles · model" entry in the subtitle menu turns it off and restores the previous track. Restarts on seeks outside covered ranges and on audio track changes; skips ahead when it runs into an already covered range; turns off on video change and unmount.
- Models in `~/.whisper/models`: `ggml-tiny.bin`, `ggml-base.bin`, `ggml-small.bin`, `ggml-small-q5_1.bin`, `ggml-large-v3-turbo-q5_0.bin`, and the VAD model `ggml-silero-v5.1.2.bin`.

**Committed:** on `spike/live-subtitles` (2026-10-06). This doc is matched by the `*.md` rule in `.gitignore`, so it was added with `git add -f`; later edits to it are tracked normally.

**Running the harness:**

```powershell
$env:CMAKE_GENERATOR="Ninja"
cargo build --release --example live_bench --manifest-path=src-tauri/Cargo.toml
./src-tauri/target/release/examples/live_bench.exe "<media path>" --lang en
```

Defaults: models tiny, base, small-q5_1 and small; threads 4 and 8; windows 5 s and 20 s; start 60 s in; 3 windows each. The input needs at least `start + 20 s x 3` of audio, so pass `--start 0` for short files. Options are listed at the top of the harness file. whisper.cpp prints a lot of log lines to stderr; filter lines starting with `whisper_`, `ggml_`, `load_backend` and `register_` to read the table.

The looped test clip from the first run was a temporary file. To recreate it:

```powershell
ffmpeg -stream_loop 11 -i "C:\Users\rudra\Downloads\rudi english.m4a" -t 60 -ac 1 -ar 16000 looped60.wav
```

**Next actions, in order:**

1. Try live mode in `pnpm tauri dev` on a video with dialogue: turn it on, play, seek forward and back, switch audio track, turn it off. Note time to first cue, gaps and wrong text.
2. Test the live cache: watch part of a video in live mode, turn it off and on (cues should come back instantly), reopen the video, switch audio tracks and back. Cache files are in `%LOCALAPPDATA%\glucose\live-cache`.
3. Fix what testing turns up, then step 6 (seek-bar strip, status chip), step 8 (playback impact at the 8-thread cap) and step 9b ("Save as SRT").
4. Benchmark a few minutes of a real video with natural dialogue (`--start 60` or later to skip intros), and record the actual text of `rudi english.m4a` for the step 1 comparison.
5. Before calling the gate passed for real: run the harness on a mid-range laptop (4-8 cores). This is the biggest open risk.

**Decisions so far:**

- Look-ahead chunked transcription, not streaming.
- No short first window: use full 15-30 s windows from the start (Whisper pads to 30 s anyway).
- Thread cap 8 as the working default.
- `small-q5_1` as the provisional default model (quality not yet confirmed).
- Chunk cuts use a simple energy minimum, not Silero VAD, for the first version. VAD is the planned upgrade if boundaries cause dropped or split words.
- The batch generator is unchanged: it still loads its own model per run, and live mode keeps a separate cached model. Sharing one context is deferred.
- Settings cannot download `small-q5_1` yet, so on machines without it live mode falls back to `small`.
- Cues follow the Netflix Timed Text Style Guide for English in both live and batch generation (`src-tauri/src/subtitle_format.rs`, unit-tested): Whisper cuts segments at 84 characters on word boundaries (token timestamps on), then cues are split to at most 2 lines of 42 characters and 7 s, broken into balanced lines preferring punctuation, and retimed to at least 5/6 s and at most 20 CPS by extending into silence, never into the next cue (2-frame gap) or past the transcribed audio. Live cache format bumped to 2 so older unformatted caches are ignored.
- **Known gaps (measured 2026-10-06 on 106 live cues from a 4.5 min talk, `small-q5_1`):** 9 cues over 42 chars per line (max 45), 17 under 5/6 s (min 0.01 s), 33 over 20 CPS, 11 with less than a 2-frame gap; none over 2 lines or 7 s. Causes: Whisper's own 84-char `max_len` cut leaves orphan fragments ("the", "out", "trained to") and mid-phrase splits; text that cannot fit 2x42 is not split further; cues are conformed per chunk, so there is no gap at chunk boundaries; fast verbatim speech genuinely exceeds 20 CPS. Planned fix: segment cues ourselves from Whisper word timestamps (sentence, then clause, then length), merge orphans, split when two lines cannot fit, keep a gap at chunk ends, treat 20 CPS as a soft target, and add a checker that measures real cue data.
- Live results persist in a hidden per-video cache, not an auto-saved SRT. SRT is an explicit export. See "Saving and the live cache".
- Don't use `whisper-rs` 0.16's `set_abort_callback_safe` with a closure that captures anything. It casts the boxed closure back to the wrong type, reads unrelated memory, and aborts at random (encoder error -6, "Transcription failed: Generic whisper error"). A repro failed 20/20 runs with the cancel flag false. The live worker uses the raw `set_abort_callback` with a pointer to its cancel flag instead. The batch path's closure captures nothing, so it is unaffected.

## Goal

Offer an optional "live subtitles" mode that starts showing generated subtitles shortly after playback begins, instead of requiring the user to wait for a full batch transcription and SRT file.

The spike answers one question: **can Whisper stay ahead of the playhead on typical hardware, with acceptable quality and latency, without hurting playback?**

## Background

Current pipeline (`src-tauri/src/lib.rs`):

1. FFmpeg extracts the full audio as a 16 kHz mono WAV.
2. The Whisper model is loaded (`WhisperContext::new_with_params`) on every call.
3. `state.full()` runs over the entire file (`transcribe_audio_with_whisper`, ~line 1584) with the user-selected language (`set_language`, ~line 1617).
4. Segments are written to an SRT (~line 1355).

Cancellation uses a single global flag, `SUBTITLE_CANCEL: AtomicBool` (~line 15). Translation is disabled (`set_translate(false)`).

Model reloading per run and the global cancel flag are both unsuitable for incremental use as is.

## Approach: look-ahead chunked transcription tied to the playhead

The video is a file on disk, so true streaming ASR is unnecessary. Transcribe ahead of the playhead in chunks and keep a running cue list.

- FFmpeg starts at the current playback position (`-ss <pos>`) and pipes 16 kHz mono PCM of the **currently selected audio track** to stdout. One long-lived process per play session, restarted on seek or audio track change.
- Windows are about 15-30 s, cut at VAD-detected pauses. A shorter first window was considered but does not help; see Findings.
- Each window is transcribed by Whisper. Segment timestamps are offset by the window start so cues land on the global timeline.
- Language: use the language selected in settings. If set to auto, detect once on the first chunk and lock it for the session, so per-chunk detection cannot flip languages mid-video.
- Cues are appended to one in-memory list (`{start, end, text}[]`), the same shape as a parsed SRT, and rendered by the existing `SubtitleOverlay.svelte`.
- Chunks are an internal detail. No per-chunk files are written.
- Cues and covered ranges are persisted to a hidden cache so a later session (same video and audio track) resumes instead of starting over. A normal `.srt` is only written on an explicit "Save as SRT". See "Saving and the live cache".

Sliding-window streaming (whisper.cpp `stream` style, LocalAgreement-2 commits) is out of scope for local files. It is only worth revisiting for live sources such as microphone or system audio.

## Covered ranges (the "buffer")

Track which time ranges have been processed, separate from the cues, because silence produces no cues but still counts as covered.

- Data: `ranges: [start, end][]`, merged when adjacent or overlapping.
- Seek bar: draw covered ranges as a strip like the video buffer, in a distinct color.
- Status chip: "Subtitles ready to 2:20", with a spinner while the worker is active.
- Gap handling: if the playhead is in an uncovered range, the overlay shows "Generating subtitles..." instead of nothing.
- Lead time: `coveredEnd - currentTime`. If it trends toward zero, the worker cannot keep up; warn or fall back to a smaller model.
- Backpressure: pause the worker once the lead exceeds a threshold (about 2-3 min) and resume when the playhead catches up.
- Seek: inside a covered range, do nothing. Outside one, cancel the current job and restart at the new position. Cached cues are reused when seeking back.
- Audio track change: save the old track's cache, load the new track's cache (if any), and continue from the playhead. Cues never mix across tracks.

## Interaction with existing subtitles

- While live mode is on, it replaces the active subtitle track (loaded `.srt`/`.vtt`/`.ass` or embedded track). Turning it off restores the previous selection.
- Live mode and batch generation must not cancel each other (see cancellation below).

## Saving and the live cache

Decision (2026-10-06): live results persist in a hidden cache, and SRT is an explicit export. An auto-saved SRT was rejected because:

- Most live sessions cover part of a video. An SRT cannot tell "covered, nobody spoke" from "not transcribed yet", so a later session could not resume correctly. The cache stores covered ranges next to the cues.
- The batch generator already writes `<video>.srt` next to the video. An auto-saved live SRT would collide with it or overwrite a user's own file.
- An SRT next to the video is auto-loaded on the next open, which would silently promote possibly lower-quality live output (e.g. from a `tiny` fallback) to "the" subtitles.

### 9a. Live cache

- **Location:** `<local app data>/glucose/live-cache/<key>.json`, outside the user's folders.
- **Key:** a stable 64-bit FNV-1a hash of video path, file size, file modification time, audio stream index and language. The same fields are stored in the file and compared on load, so a hash collision or a re-encoded file is a miss, not stale cues.
- **Model is not part of the key.** The file records which model produced it. On load, a cache made by a lower-ranked model than the one live mode would use now is ignored, and the new session's results replace it.
- **Contents:** format version, the key fields, model, covered ranges and cues.
- **Writes:** the frontend owns the cue list, so it calls `save_live_cache` at most every 10 s while there are unsaved chunks, and flushes on turning live mode off, audio track change, video change and closing the player. Rust writes to a temp file and renames it, so a crash cannot leave a half-written cache.
- **Reads:** turning live mode on loads the matching cache first. The worker then starts at the end of the covered range containing the playhead (or at the playhead if it is uncovered), and does not start at all if everything from there to the end is covered.
- **Eviction:** after each write, remove the least recently written files until the folder is under 50 MB. A two-hour film is a few hundred KB.

### 9b. Save as SRT

- A "Save as SRT" entry in the subtitle menu while live mode is on, writing through the existing SRT writer to a location the user picks.
- On partial coverage, offer to save anyway with a warning that some parts are missing. Finishing the gaps first is a possible later addition.

## Components

### Rust (`src-tauri`)

- **Model lifecycle:** `WhisperContext` held in Tauri managed state, loaded on first use and reused across chunks. Unloaded when live mode is turned off or the player window closes, so the model (about 500 MB for `small`) is not held in memory indefinitely.
- **Commands:** `start_live_subtitles(path, start_secs, audio_track, model, language)` and `stop_live_subtitles()`.
- **Worker thread:** FFmpeg PCM pipe, VAD chunker, Whisper per chunk.
- **Cancellation:** a per-session cancel token for the live worker, not the global `SUBTITLE_CANCEL`, so cancel-on-seek never aborts a batch generation running at the same time.
- **Threading:** cap Whisper's thread count (`FullParams::set_n_threads`) so decoding and rendering keep enough CPU. The cap should be configurable during the spike.
- **Events:** `subtitle-cue` (batch of cues with global timestamps) and `subtitle-progress` (covered range, worker state, real-time factor).
- **VAD:** evaluate whisper.cpp's built-in VAD exposed by `whisper-rs`; fall back to Silero via ONNX or a simple energy gate if unavailable.

### Frontend

- New store (e.g. `src/lib/subtitle/liveSubtitleStore.ts`) holding `cues[]`, `ranges[]` and worker state, fed by the Tauri events.
- `SubtitleOverlay.svelte` reads from the live store when live mode is active.
- Player page (`src/routes/player/[videoPath]/+page.svelte`): toggle, status chip, seek-bar covered strip, seek handler and audio track change handler that call start/stop.
- Settings: model choice, GPU on/off, thread cap (spike only).

## Open questions to settle during the spike

1. ~~Does `whisper-rs` 0.16 expose whisper.cpp's VAD?~~ Yes: `FullParams::enable_vad`, `set_vad_model_path`, `set_vad_params`, and a standalone `WhisperVadContext` that returns speech segments. It needs a separate Silero model file (`ggml-silero-v5.1.2.bin`). Confirmed working in our Windows build (see Findings).
2. Real-time factor for `base`, `small`, `small-q5_1` (and optionally `distil-small.en`) on CPU, and on GPU if a feature flag is practical. Partly answered on a high-end CPU (see Findings); still needed on a mid-range machine and with real dialogue.
3. Chunk size vs. quality: boundary artifacts and dropped words at chunk edges.
4. Previous-chunk prompting (`initial_prompt` with the last chunk's text): does it improve continuity, or trigger Whisper's known repetition loops? Treat as an experiment, off by default.
5. Startup latency to the first cue, and whether pre-buffering before playback is worth it. Expected about 4 s with `small-q5_1` at 8 threads (see Findings); to confirm in the app.
6. Hallucination rate on silence and music with VAD on vs. off.
7. Playback impact: dropped frames or stutter while Whisper runs, at different thread caps.
8. FFmpeg piped output: seek accuracy with `-ss` before `-i` on variable-bitrate and MKV files, and restart time.
9. Language lock: is detecting once on the first chunk reliable when the video opens with music or silence?

## Plan

1. **Baseline measurements.** Script or test harness that times Whisper on 5 s and 20 s windows for each candidate model, at a few thread caps. Record the real-time factor.
2. **Go/no-go gate.** If no model reaches about 2x real time with a reasonable thread cap on a mid-range machine, stop and reassess (smaller models, GPU, or alternative engines) before building anything else.
3. **Persistent context.** Move model loading into managed state with load/unload; confirm the batch path still works with it.
4. **Chunked worker.** FFmpeg PCM pipe for the selected audio track, per-session cancel token, fixed-size windows, then VAD cuts. Emit cues and progress events.
5. **Frontend wiring.** Live store, overlay hookup, toggle in the player, replacing the active subtitle track while on.
6. **Covered ranges UI.** Seek-bar strip, status chip, gap message.
7. **Seek, track change and backpressure.** Cancel and restart on seek or track change, pause when far ahead, resume on catch-up.
8. **Playback impact check.** Measure dropped frames with live mode on vs. off; tune the thread cap.
9. **Persistence.** (a) Live cache so sessions resume across toggles and reopens; (b) explicit "Save as SRT" export with a partial-coverage warning.
10. **Write up findings.** Real-time factors, quality notes, playback impact, recommended defaults, and a go/no-go recommendation.

## Findings

### Step 1: baseline measurements (2026-10-05)

Harness: `src-tauri/examples/live_bench.rs` (release build). It decodes audio through an FFmpeg PCM pipe, runs the Silero VAD, and times `state.full()` per window for each model, thread count and window size.

Setup:

- Machine: 20 logical cores (high-end; not the mid-range target).
- Input: a 5.5 s English speech clip, looped to 60 s at 16 kHz mono. Looped single-sentence speech is denser and more repetitive than real dialogue, so treat the numbers as indicative.
- 3 windows per configuration, greedy decoding, language `en`.

Real-time factor on 20 s windows (minimum across windows; higher is faster):

| Model | 4 threads | 8 threads | 12 threads |
|---|---|---|---|
| tiny | 5.4x | 6.0x | 5.1x |
| base | 4.2x | 4.7x | 4.0x |
| small-q5_1 | 3.1x | 5.3x | 5.2x |
| small | 3.3x | 4.9x | 4.6x |

Wall time per window on `small` / `small-q5_1` at 8 threads: about 3.7-4.1 s for both 5 s and 20 s windows.

Transcript of the same 5 s sample:

- tiny: "At the Tahih car, cold after walking home in the weather."
- base: "At the Tahiqat cold after walking home in the winter."
- small-q5_1: "I thought he caught a cold after walking home in the winter."
- small: "I had to tie a card cold after walking home in the winter."

Conclusions:

1. **Go/no-go gate passes on this machine.** Every model clears 2x real time on 20 s windows, `small-q5_1` at 8 threads reaches 5.3x.
2. **Short first windows do not help.** Whisper pads every input to 30 s internally, so a 5 s window costs about as much as a 20 s one. Since the file is on disk, the first 20 s of audio is available immediately, so the first cue should appear in about 4 s with a full-size first window. The short-first-window idea is dropped.
3. **8 threads is the knee.** 4 to 8 threads gives about 1.7x on `small`; 12 threads adds nothing. Default thread cap for further work: 8 (to be revisited in the playback impact check).
4. **Provisional default model: `small-q5_1`.** Same speed as `small` and 190 MB on disk vs 488 MB. Its transcript looked the most plausible, but the clip's actual text is not recorded and one sentence is noise, so the quality ranking between `small` and `small-q5_1` is unsettled.
5. **`tiny` and `base` are not good enough on quality** for this sample, and their 20 s timings varied a lot between windows (averages 2-4x higher than the minimums). Keep them only as fallbacks for slow machines.
6. **VAD works on Windows.** Silero VAD loaded and found the single speech segment (0.96-3.77 s) in about 100 ms including model load. Open question 1 is closed.

Still to do for step 1:

- Re-run on a mid-range laptop before claiming the gate passes for the target hardware.
- Re-run with a longer real video with natural dialogue.

## Success criteria

- On a mid-range machine, `base` or `small` stays at least 2x faster than real time and keeps a lead of 30 s or more after startup.
- First cues appear within about 5-10 s of starting playback.
- Seeking to an uncovered position produces subtitles within a few seconds.
- No visible playback stutter with live mode on at the default thread cap.
- Cue quality is comparable to the existing batch output.
- Running live mode does not interfere with a batch generation in progress, and vice versa.

## Out of scope

- Translation (including Whisper's translate-to-English mode).
- Live audio sources (microphone, system loopback, network streams).
- Alternative engines (Vosk, sherpa-onnx, Moonshine), unless the go/no-go gate fails.
- Polishing the UI beyond what is needed to validate the approach.

## Risks

- Low-end Windows CPUs may not reach real time even with `base`; mitigation is the go/no-go gate, the lead-time warning and model fallback.
- Whisper competing with video decoding for CPU; mitigation is the thread cap and the playback impact check.
- GPU builds add platform-specific build complexity to CI.
- Chunk-boundary errors may be visible; mitigation is VAD cuts. Previous-chunk prompting is a possible fix but can cause repetition loops.
- Language misdetection on the first chunk locks the wrong language for the session; mitigation is preferring the user-selected language and re-detecting if the first chunk has little speech.
- Longer-running worker threads increase the importance of correct cancellation and cleanup on window close.
