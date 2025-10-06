# Features

## 🎬 Video Playback

- **Instant Frame Seeking** - When scrubbing, video pauses and shows exact frames immediately
- **Smooth Scrubbing** - Click and drag on the progress bar to scrub through the video
- **All Browser-Supported Formats** - MP4, MKV, AVI, MOV, WebM, and more

## 🎨 UI/UX

- **Brutal Minimalism** - Clean, cinematic interface that stays out of your way
- **Auto-hiding Controls** - Controls fade away after 2 seconds during playback
- **Hover Preview Thumbnails** - See video frames when hovering over the progress bar
- **Responsive Progress Bar** - Visual handle appears on hover and during scrubbing

## 📁 File Management

### Three Ways to Open Videos:
1. **Drag & Drop** - Drag video files directly into the window
2. **File Picker** - Native file dialog with video format filtering
3. **File Association** - Set glucose as default app for video files

### Recent Videos Library
- **Auto-Scan** - Automatically scans Videos, Downloads, Desktop, and Documents folders
- **Grid View** - Clean card-based layout showing recent videos
- **Quick Access** - Click any recent video to start playing immediately
- **Smart Sorting** - Most recently modified videos appear first
- **Top 20** - Shows up to 20 most recent videos

## ⌨️ Keyboard Shortcuts

### Playback
- `Space` or `K` - Play/Pause
- `←` - Skip backward 5 seconds
- `→` - Skip forward 5 seconds

### Audio
- `M` - Mute/Unmute
- `↑` - Volume up
- `↓` - Volume down

### Display
- `F` - Toggle fullscreen

## 🎯 Advanced Features

### Progress Bar Enhancements
- **Click to Seek** - Click anywhere on the progress bar to jump
- **Drag to Scrub** - Click and drag for smooth scrubbing
- **Hover Preview** - Canvas-based thumbnails show frame at hover position
- **Visual Feedback** - Progress bar grows on hover, shows scrubber handle

### Instant Seeking
- Uses `fastSeek()` API when available for instant frame updates
- Pauses playback during scrubbing
- Resumes automatically when scrubbing ends

### Video Discovery
- Scans common Windows directories automatically
- Shows file size and metadata
- Fast loading with async scanning
- Clean error handling if scan fails

## 🛠️ Technical

- **Frontend** - Svelte 5 with SvelteKit
- **Backend** - Rust with Tauri v2
- **File System** - Native Rust directory scanning with `dirs` crate
- **Canvas API** - Frame extraction for hover previews
- **Type-Safe** - Full TypeScript support
