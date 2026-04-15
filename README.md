<div align="center">
  <a href="https://glucose.media"><img src="static/logo-dark.svg" alt="Glucose Media Player" width="400"></a>

_A Sleek & Lightweight VLC Alternative with On-Device AI Subtitle<br>_


[![License: EUPL](https://img.shields.io/badge/License-EUPL%201.2-1a1a1a?style=for-the-badge&logo=opensourceinitiative&logoColor=white&labelColor=0a0a0a)](https://joinup.ec.europa.eu/collection/eupl/eupl-text-eupl-12)
[![Built with Tauri](https://img.shields.io/badge/Built%20with-Tauri-1a1a1a?style=for-the-badge&logo=tauri&logoColor=white&labelColor=0a0a0a)](https://tauri.app)
[![Svelte](https://img.shields.io/badge/Frontend-Svelte-1a1a1a?style=for-the-badge&logo=svelte&logoColor=white&labelColor=0a0a0a)](https://svelte.dev)
[![Rust](https://img.shields.io/badge/Backend-Rust-1a1a1a?style=for-the-badge&logo=rust&logoColor=white&labelColor=0a0a0a)](https://rust-lang.org)

[![Version](https://img.shields.io/github/v/release/rudi-q/glucose_media_player?style=for-the-badge&labelColor=0a0a0a&color=1a1a1a&label=Version)](https://github.com/rudi-q/glucose_media_player/releases)
[![Downloads](https://img.shields.io/github/downloads/rudi-q/glucose_media_player/total?style=for-the-badge&labelColor=0a0a0a&color=1a1a1a)](https://github.com/rudi-q/glucose_media_player/releases)
[![Platform](https://img.shields.io/badge/Platform-Windows%20-1a1a1a?style=for-the-badge&labelColor=0a0a0a)](https://github.com/rudi-q/glucose_media_player/releases)

</div>

---

## ✨ Features

### 🎬 Cinematic Mode
Enjoy your media with a beautifully blurred background and centered content for truly immersive viewing.

### 🖼️ Universal Media Support
- **Videos**: MP4, MKV, AVI, MOV, WebM, WMV, FLV, M4V
- **Subtitles**: SRT, VTT, ASS, SSA, SUB

### 🎯 Minimal by Design
No clutter, no distractions. Just your content and elegant controls that appear when you need them.

### ⚡ Blazingly Fast
Built with Rust and Tauri for native performance with a tiny footprint.

### 🎮 Keyboard-First
Complete keyboard navigation for power users who value efficiency.

### 📂 Smart Gallery
Automatically scans and displays your recent videos in a beautiful grid layout.

### 🎨 Modern Interface
- Frameless, transparent window design
- Smooth animations and transitions
- Audio output device selection
- Volume control with visual feedback
- Timeline scrubbing with video preview
- Fullscreen and cinematic viewing modes

---

## 📥 Installation

### Pre-built Binaries

Download the latest release for your platform:

- **Windows**: `glucose_0.2.0_x64_en-US.msi` or `.exe`

### Build from Source

#### Prerequisites

- [Node.js](https://nodejs.org/) (v18 or later)
- [pnpm](https://pnpm.io/) (recommended) or npm
- [Rust](https://www.rust-lang.org/tools/install) (latest stable)
- Platform-specific requirements:
  - **Windows**: [Microsoft Visual Studio C++ Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  - **macOS**: Xcode Command Line Tools
  - **Linux**: `libwebkit2gtk-4.1-dev`, `build-essential`, `curl`, `wget`, `file`, `libssl-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`

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

glucose offers three convenient ways to open your media:

1. **Drag & Drop** — Simply drag a video or image file into the glucose window
2. **File Dialog** — Click the "Open Video" button in the gallery screen
3. **File Association** — Set glucose as your default media player and open files directly from your file explorer
4. **Recent Gallery** — Browse and play recently accessed videos from the home screen

### Keyboard Shortcuts

#### Playback Controls (Video Mode)
| Key | Action |
|-----|--------|
| `Space` or `K` | Play/Pause |
| `←` | Skip backward 5 seconds |
| `→` | Skip forward 5 seconds |
| `↑` | Increase volume |
| `↓` | Decrease volume |
| `M` | Mute/Unmute |
| `C` or `S` | Toggle subtitles on/off |
| `F` | Toggle cinematic/fullscreen mode |

#### Navigation
| Key | Action |
|-----|--------|
| `Backspace` | Return to gallery |
| `Escape` | Close application |
| `Arrow Keys` | Navigate gallery (when in home screen) |
| `Enter` | Open selected video from gallery |

### Advanced Features

- **Subtitle Support**: 
  - Automatically loads .srt files in the same folder as the video
  - Manually load subtitle files from anywhere
  - Toggle subtitles on/off during playback (C/S keys)
- **Timeline Scrubbing**: Hover over the progress bar to see video preview thumbnails
- **Audio Device Selection**: Click the audio device icon to switch between output devices
- **Volume Popup**: Click the volume icon for a vertical volume slider
- **Cinematic Mode**: Enjoy videos with an elegant blurred background
- **Fullscreen Mode**: Press `F` to toggle fullscreen for maximum immersion

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

Refer to our terms of use at [https://glucose.media/terms](https://glucose.media/terms) for more information.

---

## 🙏 Acknowledgments

- Built with [Tauri](https://tauri.app/) and [Svelte](https://svelte.dev/)
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
    <a href="https://github.com/yourusername/glucose">GitHub</a> •
    <a href="https://github.com/yourusername/glucose/issues">Report Bug</a> •
    <a href="https://github.com/yourusername/glucose/discussions">Request Feature</a>
  </p>
</div>
