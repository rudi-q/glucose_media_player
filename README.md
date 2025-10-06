# glucose 🎬

A brutally minimalist, cinematic video player built with Svelte + Tauri.

## Features

- **Brutal Minimalism** - Clean, premium UI that stays out of your way
- **Drag & Drop** - Simply drop video files to play
- **File Associations** - Open videos directly with glucose as the default app
- **Keyboard Controls** - Full keyboard navigation
- **Universal Format Support** - Supports all video formats your browser can play (MP4, MKV, AVI, MOV, WebM, etc.)

## Development

```bash
# Install dependencies
pnpm install

# Run in development mode
pnpm tauri:dev

# Check Rust code
pnpm tauri:check

# Check Svelte/TypeScript code
pnpm check

# Build for production
pnpm tauri:build
```

## Keyboard Shortcuts

- `Space` or `K` - Play/Pause
- `F` - Fullscreen
- `M` - Mute/Unmute
- `←` - Skip back 5 seconds
- `→` - Skip forward 5 seconds
- `↑` - Volume up
- `↓` - Volume down

## Usage

**Three ways to open videos:**

1. **Drag & Drop** - Drag a video file into the window
2. **File Dialog** - Click "Open Video" button or the folder icon
3. **File Association** - Set glucose as default app for video files (after building)

## Tech Stack

- **Frontend** - Svelte 5 + SvelteKit
- **Backend** - Rust + Tauri v2
- **Styling** - Pure CSS (brutalist/minimal aesthetic)

## License

MIT

# Tauri + SvelteKit + TypeScript

This template should help get you started developing with Tauri, SvelteKit and TypeScript in Vite.

## Recommended IDE Setup

[VS Code](https://code.visualstudio.com/) + [Svelte](https://marketplace.visualstudio.com/items?itemName=svelte.svelte-vscode) + [Tauri](https://marketplace.visualstudio.com/items?itemName=tauri-apps.tauri-vscode) + [rust-analyzer](https://marketplace.visualstudio.com/items?itemName=rust-lang.rust-analyzer).
