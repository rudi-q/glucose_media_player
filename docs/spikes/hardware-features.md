# Spike: Features tab (hardware check)

Branch: `spike/hardware-features` (from `spike/live-subtitles`, with `spike/smart-subtitle-position` and `fix/live-subtitles-review` merged in until those PRs are merged).

## Status and how to resume

_Last updated 2026-10-18._

**Where things stand:** the check, the verdicts, the Features tab, the automatic first check and the warnings next to each feature's setting are built and committed. The automatic first check has been seen running in a dev build (in the background, no window freeze). The tab has been seen by eye once, before the limits were relaxed. Thresholds have only been tried on one machine.

**Why:** Glucose now has several heavy features (live subtitles, live translation, face detection for smart subtitle position, AI subtitles with the large model). On a weak PC they can be slow or fail, which makes the whole app look bad. The check measures what this PC can do and says so per feature, instead of promising everything.

## What exists

- `src-tauri/src/hardware_check.rs`, command `run_hardware_check`, on a background thread:
  - Processor name (registry), logical thread count, total memory (`GlobalMemoryStatusEx`) and the graphics card with the most dedicated memory (DXGI). Windows only; other systems report these as unknown.
  - Live subtitle speed: Whisper with the model live mode would use (`live_subtitles::preferred_model`), live mode's settings and thread cap, on 20 s of speech (one live chunk). Runs twice and keeps the faster run. Model loading is not counted, since live mode loads it once per session.
  - Face detection: median of five detections on a 320x180 frame (`face_detect::time_detection`).
- `src-tauri/assets/hardware_check/`: the speech sample (raw 16 kHz mono 16-bit PCM, 640 KB) and `SOURCE.txt`. The sample is 20 s of a public-domain LibriVox recording.
- `src/lib/utils/featureAdvice.ts`: verdicts, storage (`localStorage` key `glucose_hardware_check`) and the warning text used next to settings.
- Settings > Features: the machine summary, one card per feature with verdict and reason, "Check again", the last check time, and a note when the first check turned something off.
- First check: runs once by itself after onboarding, 5 s after the library is showing (never while a video plays, so it cannot make playback stutter). Features rated "Not recommended" are turned off: auto-start of live subtitles, live translation, smart position. They can be turned back on. "Check again" never changes settings.
- Warnings next to each feature's own setting when its verdict is "May be slow" or "Not recommended": the auto-start and translation settings, Settings > Player > Subtitle Position, the subtitle style panel (under Smart), and the player's AI model menu (Live and Large).
- After a speech model is downloaded, the check runs again if the last one had no model to test.

## Verdicts

| Feature | Works well | May be slow | Not recommended |
|---|---|---|---|
| Live subtitles | 1.5x real time or faster | 1.0x to 1.5x | below 1.0x |
| Live translation | follows live subtitles (translation itself takes about 75 ms per 20 s chunk) | | |
| Smart subtitle position | face check up to 100 ms | up to 200 ms (the longest a subtitle waits) | over 200 ms |
| AI subtitles, large model | 8 GB memory or more | 4 to 8 GB | under 4 GB |

The first limits were stricter (live 3x / 1.5x, faces 60 / 150 ms) and rated live subtitles "Not recommended" on a machine where they work well, translation included. They were relaxed because live mode only needs to beat real time to keep up; what a lower speed costs is the wait for the first subtitles after a seek (about 20 s / speed).

## Findings

- On an i9-13900H laptop (20 threads, 16 GB, Intel Iris Xe), plugged in, with a browser, an IDE and another dev server running: Whisper `small-q5_1` measured 1.2x in a dev build and 1.2x to 2.0x in a release build; face detection 31 to 80 ms. Earlier, with the machine idle, face detection measured 22 ms.
- The result depends heavily on what else is running. The tab says so and suggests closing heavy apps and checking again. Busy-PC detection is not built.
- Dev and release builds measured almost the same Whisper speed, so whisper.cpp is not unoptimised in dev builds (an earlier guess that it was turned out wrong).

## Open items

- Try the limits on a weak machine (4 to 8 threads, 8 GB or less).
- The large-model verdict is an estimate from memory only; that model is never actually run, since loading it takes too long.
- macOS and Linux: processor, memory and graphics card are not read yet (the speed tests work).
- Possibly detect a busy PC during the check and say so, or retry later.
