<div align="center">
  <a href="https://glucose.media"><img src="static/logo-dark.svg" alt="Glucose Media Player" width="400"></a>

_A powerfully modern, thoughtfully designed media playback experience._


[![License: EUPL](https://img.shields.io/badge/License-EUPL%201.2-1a1a1a?style=for-the-badge&logo=opensourceinitiative&logoColor=white&labelColor=0a0a0a)](https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-1a1a1a?style=for-the-badge&logo=tauri&logoColor=white&labelColor=0a0a0a)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Frontend-Svelte-1a1a1a?style=for-the-badge&logo=svelte&logoColor=white&labelColor=0a0a0a)](https://svelte.dev)
[![Rust](https://img.shields.io/badge/Backend-Rust-1a1a1a?style=for-the-badge&logo=rust&logoColor=white&labelColor=0a0a0a)](https://rust-lang.org)

[![Version](https://img.shields.io/github/v/release/rudi-q/glucose_media_player?style=for-the-badge&labelColor=0a0a0a&color=1a1a1a&label=Version)](https://github.com/rudi-q/glucose_media_player/releases)
[![Downloads](https://img.shields.io/github/downloads/rudi-q/glucose_media_player/total?style=for-the-badge&labelColor=0a0a0a&color=1a1a1a)](https://github.com/rudi-q/glucose_media_player/releases)
[![Platform](https://img.shields.io/badge/Platform-Windows-1a1a1a?style=for-the-badge&labelColor=0a0a0a)](https://github.com/rudi-q/glucose_media_player/releases)

</div>

---

## ✨ Features

### 🤖 On-Device AI & Subtitles

- **100% Private Transcription**: Generate highly accurate subtitles using Whisper running locally. No internet required.
- **Universal Format Support**: Auto-loads `.srt`, `.vtt`, `.ass`, and directly extracts subtitles embedded in MKV and MP4 files.

### ⏭️ Intelligent Playback

- **"Up Next" Overlay**: A Netflix-style countdown overlay that smoothly transitions you to the next episode.
- **Picture-in-Picture Mode**: A compact, draggable window that snaps to corners for easy multitasking.

### 📂 Smart Gallery & Custom Libraries

- **Intelligent Hover Previews**: Instantly preview video frames by hovering, seamlessly synced with your watch progress.
- **Time-Based Grouping**: Videos are automatically organized into sections like "Today" and "Yesterday".
- **Custom Libraries**: Configure exactly which folders Glucose should scan.

### 🔊 Pro Audio & Visuals

- **Dynamic Audio Visualizer**: A beautifully crafted, real-time visualizer for your music files.
- **Dual Audio Support**: Seamlessly switch between embedded audio tracks with real-time remuxing.
- **200% Volume Booster**: Web Audio API-powered boost for quiet files.

### 📽️ Premium Cinematic Experience

- **Cinematic Blur**: Enjoy your media with a beautifully blurred background for immersive viewing.
- **Glassmorphic UI**: A stunning design system featuring elegant **Instrument Serif** typography.
- **Fade-In & Fade-Out Transitions**: Smooth visual and playback volume transitions when pausing or playing.
- **Intelligent Controls**: Auto-hiding controls that stay out of your way.

### ⚡ Blazingly Fast & Private

- **Native Performance**: Built with Rust and Tauri for GPU-accelerated rendering and a tiny footprint.
- **Zero Telemetry**: Detects cloud-only file placeholders locally without uploading file contents, with no ads and no tracking.

---

## 📥 Installation

Download the latest release from the [Releases page](https://github.com/rudi-q/glucose_media_player/releases):

- **Windows**: Download the latest Windows installer

### Getting Started

Glucose offers several convenient ways to open your media:
1. **Drag & Drop** — Drag a video or audio file directly into the Glucose Media Player window
2. **Smart Gallery** — Browse and play recently accessed files or custom library folders from the home screen
3. **File Association** — Set Glucose Media Player as your default media player and open files directly from File Explorer

---

## ⌨️ Keyboard Shortcuts

| Key | Action |
|-----|--------|
| `Space` / `K` | Play/Pause |
| `←` / `→` | Skip backward/forward 5 seconds |
| `↑` / `↓` | Increase/Decrease volume |
| `M` | Mute/Unmute |
| `C` / `S` | Toggle subtitles on/off (video) |
| `F` | Toggle View Mode |
| `P` | Toggle Picture-in-Picture mode |
| `0` – `9` | Jump to 0%–90% of the file |
| `Backspace` | Return to gallery |
| `Escape` | Exit Current Mode / Back to Gallery |

---

## 🛠️ Contributing

Interested in contributing to the project or building from source? Please check our [CONTRIBUTING.md](CONTRIBUTING.md) guide for prerequisites, development setup, and coding standards.

---

## 📄 License

European Union Public Licence 1.2 (EUPL-1.2)

This Source Code Form is subject to the terms of the European Union Public Licence, v. 1.2. 

For a copy of the EUPL, You can obtain one at https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12 or see the [LICENSE](LICENSE) file. 

A Finnish translation is also available in [LICENSE.FI](LICENSE.FI).

Copyright (c) 2026 [DoublOne Studios Limited](https://doubl.one).

Pursuant to Article 15 of the EUPL, this Licence is governed by the laws of the Republic of Finland.

Refer to our terms of use at [glucose.media/terms](https://glucose.media/terms) for more information.

---

<div align="center">
  <p>Maintained by <a href="https://doubl.one">DoublOne Studios</a></p>
  <p>
    <a href="https://github.com/rudi-q/glucose_media_player/issues">Report Bug</a> •
    <a href="https://github.com/rudi-q/glucose_media_player/discussions">Request Feature</a>
  </p>
</div>
