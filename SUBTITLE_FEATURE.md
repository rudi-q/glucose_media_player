# Subtitle Feature Implementation

## Overview

The subtitle feature allows users to load external subtitle files (.srt, .vtt, .ass, .ssa, .sub) and display them synchronized with video playback in the glucose media player.

## Features

### File Format Support
- **SRT** (SubRip) - Most common subtitle format
- **VTT** (WebVTT) - Web Video Text Tracks
- **ASS** (Advanced SubStation Alpha) - Advanced styling support
- **SSA** (SubStation Alpha) - Subtitle format with styling
- **SUB** - MicroDVD subtitle format

### User Interface

#### Load Subtitles Button
- Located in the video controls bar
- Icon: Subtitle/caption symbol
- Tooltip: "Load subtitles"
- Opens a native file dialog filtered to subtitle formats

#### Toggle Subtitles Button
- Only appears after a subtitle file has been loaded
- Icon: Chat bubble (with slash when disabled)
- Tooltip: "Toggle subtitles (C/S)"
- Purple highlight when subtitles are active
- Click to show/hide subtitles

### Keyboard Shortcuts
- **C** or **S** - Toggle subtitles on/off (when loaded)

### Behavior
- Subtitles are automatically reset when loading a new video
- Subtitles default to enabled when first loaded
- Subtitle visibility state persists during playback
- Native browser subtitle rendering with custom styling

## Technical Implementation

### Backend (Rust)

**File**: `src-tauri/src/lib.rs`

Added new Tauri command:
```rust
#[tauri::command]
async fn open_subtitle_dialog(app: tauri::AppHandle) -> Result<Option<String>, String>
```

This command:
- Opens a native file picker dialog
- Filters to subtitle file extensions
- Returns the selected file path to the frontend

### Frontend (Svelte)

**File**: `src/routes/+page.svelte`

#### State Variables
- `subtitleSrc: string | null` - Path to loaded subtitle file
- `subtitlesEnabled: boolean` - Visibility toggle state
- `trackElement: HTMLTrackElement` - Reference to track element

#### Functions
- `openSubtitleDialog()` - Opens file picker and loads subtitle
- `loadSubtitle(path)` - Converts file path and sets up subtitle track
- `toggleSubtitles()` - Toggles subtitle visibility on/off

#### HTML Structure
```svelte
<video crossorigin="anonymous">
  {#if subtitleSrc}
    <track
      bind:this={trackElement}
      kind="subtitles"
      src={subtitleSrc}
      srclang="en"
      label="Subtitles"
      default={subtitlesEnabled}
    />
  {/if}
</video>
```

#### Styling
Custom CSS for subtitle appearance:
```css
:global(video::cue) {
  background-color: rgba(0, 0, 0, 0.8);
  color: #ffffff;
  font-size: 1.2em;
  font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.9);
}
```

## Usage Instructions

### Loading Subtitles

1. **Play a Video**
   - Open a video file using any method (drag-drop, file dialog, etc.)

2. **Load Subtitle File**
   - Click the subtitle button (caption icon) in the controls
   - Select your .srt or other subtitle file from the file picker
   - Subtitles will automatically appear on the video

3. **Toggle Visibility**
   - Click the toggle button (chat bubble icon) that appears after loading
   - Or press **C** or **S** keys on your keyboard
   - Purple highlight indicates subtitles are active

### Tips
- The subtitle file should be synchronized with your video
- Most video files have matching .srt files available online
- Subtitles persist throughout playback and can be toggled anytime
- Loading a new video will reset the subtitle selection

## Browser Compatibility

The subtitle feature uses the HTML5 `<track>` element, which is supported in:
- ✅ Chrome/Edge (WebKit)
- ✅ Firefox
- ✅ Safari
- ✅ All modern browsers

Note: The Tauri app uses WebKit as its webview engine, ensuring consistent subtitle support across all platforms.

## Future Enhancements

Potential improvements for future versions:
- [ ] Auto-detect and load matching subtitle files (same name as video)
- [ ] Subtitle styling customization (font size, color, position)
- [ ] Multiple subtitle track support (language selection)
- [ ] Subtitle offset/delay adjustment
- [ ] Embedded subtitle track extraction
- [ ] Subtitle search and download from online databases
