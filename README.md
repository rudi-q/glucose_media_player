<div align="center">
  <img src="static/logo-dark.svg" alt="glucose" width="400" />
  
  <h3>A brutally minimalist, cinematic media viewer</h3>
  
  <p>
    <strong>glucose</strong> is a modern media viewer that strips away complexity to deliver a pure, immersive viewing experience.
  </p>

  <p>
    <img src="https://img.shields.io/badge/License-MPL%202.0-FFAB97?style=flat" alt="License: MPL 2.0" />
    <img src="https://img.shields.io/badge/version-0.2.0-C065B6?style=flat" alt="Version" />

[//]: # (    <img src="https://img.shields.io/github/stars/yourusername/glucose?color=8C77FF&style=flat&logo=github" alt="GitHub Stars" />)
[//]: # (    <img src="https://img.shields.io/github/downloads/yourusername/glucose/total?label=Downloads&logo=github&color=FF6362&style=flat" alt="Downloads" />)
  </p>
  
  <p>
    <img src="https://img.shields.io/badge/SvelteKit-FF3E00?style=flat&logo=svelte&logoColor=white" alt="SvelteKit" />
    <img src="https://img.shields.io/badge/TypeScript-007ACC?style=flat&logo=typescript&logoColor=white" alt="TypeScript" />
    <img src="https://img.shields.io/badge/Tauri-24C8D8?style=flat&logo=tauri&logoColor=white" alt="Tauri" />
    <img src="https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white" alt="Rust" />
    <img src="https://img.shields.io/badge/Vite-646CFF?style=flat&logo=vite&logoColor=white" alt="Vite" />
  </p>
</div>

---

## ✨ Features

### 🎬 Cinematic Mode
Enjoy your media with a beautifully blurred background and centered content for truly immersive viewing.

### 🖼️ Universal Media Support
- **Videos**: MP4, MKV, AVI, MOV, WebM, WMV, FLV, M4V
- **Subtitles**: SRT, VTT, ASS, SSA, SUB
- **Images**: JPG, JPEG, PNG, GIF, WebP, BMP, SVG, AVIF, HEIC, HEIF

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
- **macOS**: `glucose_0.2.0_universal.dmg`
- **Linux**: `glucose_0.2.0_amd64.deb` or `glucose_0.2.0_amd64.AppImage`

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
git clone https://github.com/rudi-q/glucose-media-player.git
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

## 🛠️ Development

### Project Structure

```
glucose/
├── src/                    # SvelteKit frontend
│   ├── routes/
│   │   └── +page.svelte   # Main application component
│   └── app.html           # HTML template
├── src-tauri/             # Rust backend
│   ├── src/
│   │   └── lib.rs         # Tauri commands and logic
│   ├── Cargo.toml         # Rust dependencies
│   └── tauri.conf.json    # Tauri configuration
├── static/                # Static assets
└── package.json           # Node.js dependencies
```

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

### Tech Stack

#### Frontend
- **[Svelte 5](https://svelte.dev/)** — Reactive UI framework with runes
- **[SvelteKit](https://kit.svelte.dev/)** — Application framework
- **[TypeScript](https://www.typescriptlang.org/)** — Type-safe JavaScript
- **[Vite](https://vitejs.dev/)** — Lightning-fast build tool
- **Pure CSS** — Custom brutalist/minimal styling

#### Backend
- **[Rust](https://www.rust-lang.org/)** — Systems programming language
- **[Tauri v2](https://tauri.app/)** — Native desktop framework
- **Plugins**:
  - `tauri-plugin-opener` — Opening files and URLs
  - `tauri-plugin-dialog` — Native file dialogs
  - `tauri-plugin-process` — Process management

### Contributing

Contributing guidelines coming soon.

---

## 🎨 Design Philosophy

glucose embraces **brutal minimalism**:

- **Content First**: The media is the star. UI elements fade away when not needed.
- **Performance**: Native performance with minimal resource usage.
- **Transparency**: Literally — frameless, transparent windows that blend with your desktop.
- **Efficiency**: Keyboard shortcuts for everything. Mouse is optional.
- **Beauty in Simplicity**: No unnecessary features. Every element serves a purpose.

---

## 🐛 Known Issues

- Image playback controls (play/pause, timeline) are inherited from video mode but not applicable to static images
- Recent gallery only displays videos, not images
- Some HEIC/HEIF images may not display depending on browser codec support

---

## 📋 Roadmap

- [ ] Playlist support for sequential playback
- [ ] Image-specific controls and transitions
- [ ] Image gallery with thumbnail view
- [ ] Slideshow mode for images
- [x] Subtitle support for videos (SRT, VTT, ASS, SSA, SUB)
- [ ] Custom keyboard shortcut configuration
- [ ] Multiple window support
- [ ] Folder watching and auto-refresh
- [ ] Video playback speed controls
- [ ] Chapter markers support

---

## 📄 License

Mozilla Public License 2.0 (MPL-2.0)

This Source Code Form is subject to the terms of the Mozilla Public License, v. 2.0. If a copy of the MPL was not distributed with this file, You can obtain one at https://mozilla.org/MPL/2.0/.

Copyright (c) 2025 glucose

---

## 🙏 Acknowledgments

- Built with [Tauri](https://tauri.app/) and [Svelte](https://svelte.dev/)
- Inspired by minimalist design principles
- Thanks to all contributors and users

---

## 📞 Support

- **Issues**: [GitHub Issues](https://github.com/rudi-q/glucose-media-player/issues)
- **Discussions**: [GitHub Discussions](https://github.com/rudi-q/glucose-media-player/discussions)

---

<div align="center">
  <p>Made by the maker of <a href="https://github.com/rudi-q/leed_pdf_viewer">LeedPDF</a></p>
  <p>
    <a href="https://github.com/yourusername/glucose">GitHub</a> •
    <a href="https://github.com/yourusername/glucose/issues">Report Bug</a> •
    <a href="https://github.com/yourusername/glucose/discussions">Request Feature</a>
  </p>
</div>
