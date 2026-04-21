<div align="center">
  <a href="https://glucose.media"><img src="static/logo-dark.svg" alt="Glucose Media Player" width="400"></a>

_A Sleek & Lightweight VLC Alternative with On-Device AI Subtitles_


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

### 🎬 Cinematic Mode
Enjoy your media with a beautifully blurred background and centered content for truly immersive viewing.

### 🖥️ Picture-in-Picture Mode
Pop the player into a compact, always-on-top window that snaps to the bottom-right corner of your screen — perfect for multitasking.

### 🖼️ Universal Media Support
- **Videos**: MP4, MKV, AVI, MOV, WebM, WMV, FLV, M4V, MPG, MPEG, OGV
- **Audio**: MP3, FLAC, WAV, AAC, OGG, Opus, M4A, AIFF, WMA
- **Subtitles**: SRT, VTT, ASS, SSA, SUB

### 🎵 Audio Player with Visualizer
A dedicated audio mode with a real-time frequency visualizer powered by the Web Audio API — full playback controls, volume management, and progress saving included.

### 🤖 On-Device AI Subtitles
Generate subtitles for any video without an internet connection using [Whisper](https://github.com/ggerganov/whisper.cpp) running entirely on your machine. Three model sizes are available to balance speed and accuracy:

| Model | Size | Best for |
|-------|------|----------|
| Tiny | ~75 MB | Fast results, shorter clips |
| Small | ~466 MB | Good accuracy, most videos |
| Large v3 Turbo (q5_0) | ~574 MB | Best accuracy, longer content |

Models are downloaded once and stored locally. No data ever leaves your device.

### 📝 Subtitle Support
- Automatically loads external subtitle files (`.srt`, `.vtt`, `.ass`, `.ssa`, `.sub`) from the same folder as the video
- Detects and extracts subtitle tracks embedded directly in MKV and MP4 files
- Manually load any subtitle file from anywhere on disk
- Toggle subtitles on/off during playback

### 📂 Smart Gallery
Automatically scans and displays your recent media in a beautiful grid layout, with per-file watch progress indicators so you can pick up where you left off.

### ⏱️ Watch Progress & Resume
Playback position is saved automatically for both video and audio files. Reopen any file and resume from exactly where you stopped.

### 🎯 Minimal by Design
No clutter, no distractions. Just your content and elegant controls that appear when you need them.

### ⚡ Blazingly Fast
Built with Rust and Tauri for native performance with a tiny footprint.

### 🎮 Keyboard-First
Complete keyboard navigation for power users who value efficiency.

### 🎨 Modern Interface
- Frameless, transparent window design
- Smooth animations and transitions
- Audio output device selection
- Volume control with visual feedback
- Timeline scrubbing with video preview thumbnails
- Fullscreen, cinematic, and picture-in-picture viewing modes

---

## 📥 Installation

### Pre-built Binaries

Download the latest release from the [Releases page](https://github.com/rudi-q/glucose_media_player/releases):

- **Windows**: `glucose_3.1.0_x64_en-US.msi` or `.exe`

### Build from Source

#### Prerequisites

- [Node.js](https://nodejs.org/) (v18 or later)
- [pnpm](https://pnpm.io/) (recommended) or npm
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)

#### Building

```bash
# Clone the repository
git clone https://github.com/rudi-q/glucose_media_player.git
cd glucose

# Install dependencies
pnpm install

# Run in development mode
pnpm tauri:dev

# Build for production
pnpm tauri:build
```

The built application will be available in `src-tauri/target/release/bundle/`.

---

## 🚀 Usage

### Opening Media Files

glucose offers several convenient ways to open your media:

1. **Drag & Drop** — Drag a video or audio file into the glucose window
2. **File Dialog** — Click the "Open" button in the gallery screen
3. **File Association** — Set glucose as your default media player and open files directly from File Explorer
4. **Recent Gallery** — Browse and play recently accessed files from the home screen

### Keyboard Shortcuts

#### Playback Controls
| Key | Action |
|-----|--------|
| `Space` or `K` | Play/Pause |
| `←` | Skip backward 5 seconds |
| `→` | Skip forward 5 seconds |
| `↑` | Increase volume |
| `↓` | Decrease volume |
| `M` | Mute/Unmute |
| `C` or `S` | Toggle subtitles on/off (video) |
| `F` | Cycle view modes (cinematic → fullscreen → PiP) |
| `P` | Toggle Picture-in-Picture mode |
| `0` – `9` | Jump to 0%–90% of the file |

#### Navigation
| Key | Action |
|-----|--------|
| `Backspace` | Return to gallery |
| `Escape` | Close application |
| `Arrow Keys` | Navigate gallery (home screen) |
| `Enter` | Open selected file from gallery |

---

### Available Scripts

```bash
# Development
pnpm dev              # Start Vite dev server
pnpm tauri:dev        # Start Tauri in dev mode with hot reload

# Code Quality
pnpm check            # Type-check Svelte/TypeScript code
pnpm check:watch      # Type-check in watch mode
pnpm tauri:check      # Check Rust code (cargo check)

# Building
pnpm build            # Build frontend only
pnpm tauri:build      # Build complete application
pnpm preview          # Preview production build locally
```

## 📄 License

European Union Public Licence 1.2 (EUPL-1.2)

This Source Code Form is subject to the terms of the European Union Public Licence, v. 1.2. 

For a copy of the EUPL, You can obtain one at https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12 or see the [LICENSE](LICENSE) file. 

A Finnish translation is also available in [LICENSE.FI](LICENSE.FI).

Copyright (c) 2026 [DoublOne Studios Limited](https://doubl.one).

Pursuant to Article 15 of the EUPL, this Licence is governed by the laws of the Republic of Finland.

Refer to our terms of use at [glucose.media/terms](https://glucose.media/terms) for more information.

---

## 🙏 Acknowledgments

- Built with [Tauri](https://tauri.app/) and [Svelte](https://svelte.dev/)
- AI subtitles powered by [whisper-rs](https://github.com/tazz4843/whisper-rs) and [whisper.cpp](https://github.com/ggerganov/whisper.cpp)
- Inspired by minimalist design principles
- Thanks to all contributors and users

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/rudi-q/glucose_media_player/issues)
- **Discussions**: [GitHub Discussions](https://github.com/rudi-q/glucose_media_player/discussions)

---

<div align="center">
  <p>Made by the maker of <a href="https://github.com/rudi-q/leed_pdf_viewer">LeedPDF</a></p>
  <p>
    <a href="https://github.com/rudi-q/glucose_media_player">GitHub</a> •
    <a href="https://github.com/rudi-q/glucose_media_player/issues">Report Bug</a> •
    <a href="https://github.com/rudi-q/glucose_media_player/discussions">Request Feature</a>
  </p>
</div>
