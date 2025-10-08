# AI-Generated Subtitles Feature

## Overview

This feature adds the ability to automatically generate subtitles for videos using OpenAI's Whisper speech-to-text model. The implementation uses `whisper-rs` for local, offline subtitle generation.

## ✨ Features

- **AI-Powered Transcription**: Automatically transcribe video audio to subtitles
- **Multiple Model Sizes**: Choose from Tiny, Base, Small, Medium, or Large models (speed vs accuracy trade-off)
- **Offline Processing**: Everything runs locally - no internet required after model download
- **Auto-Load**: Generated subtitles are automatically loaded into the player
- **Progress Tracking**: Real-time progress indicators during generation
- **SRT Format**: Generates standard SRT subtitle files with accurate timestamps

## 📋 Prerequisites

### Windows

1. **Install LLVM/Clang** (required for whisper-rs build):
   ```powershell
   # Option 1: Using winget
   winget install LLVM.LLVM

   # Option 2: Download installer
   # Visit https://releases.llvm.org/download.html
   # Download LLVM-<version>-win64.exe and install
   ```

2. **Install FFmpeg**:
   ```powershell
   # Option 1: Using winget
   winget install Gyan.FFmpeg

   # Option 2: Using Chocolatey
   choco install ffmpeg

   # Option 3: Manual installation
   # Download from https://ffmpeg.org/download.html
   # Extract and add to PATH
   ```

3. **Set Environment Variables** (after LLVM install):
   ```powershell
   # Add to PATH
   $env:Path += ";C:\Program Files\LLVM\bin"
   
   # Or set permanently in System Environment Variables:
   # LIBCLANG_PATH = C:\Program Files\LLVM\bin
   ```

### macOS

1. **Install LLVM**:
   ```bash
   brew install llvm
   ```

2. **Install FFmpeg**:
   ```bash
   brew install ffmpeg
   ```

### Linux

1. **Install LLVM/Clang**:
   ```bash
   # Ubuntu/Debian
   sudo apt-get install llvm-dev libclang-dev clang

   # Fedora
   sudo dnf install llvm-devel clang-devel

   # Arch
   sudo pacman -S llvm clang
   ```

2. **Install FFmpeg**:
   ```bash
   # Ubuntu/Debian
   sudo apt-get install ffmpeg

   # Fedora
   sudo dnf install ffmpeg

   # Arch
   sudo pacman -S ffmpeg
   ```

## 📥 Whisper Model Setup

Before using the AI subtitle generation, you need to download Whisper models:

1. **Create models directory**:
   ```bash
   # Windows
   mkdir %USERPROFILE%\.whisper\models

   # macOS/Linux
   mkdir -p ~/.whisper/models
   ```

2. **Download models** from [Whisper.cpp Hugging Face](https://huggingface.co/ggerganov/whisper.cpp/tree/main):

   | Model | Size | Speed | Accuracy | Download |
   |-------|------|-------|----------|----------|
   | Tiny | 75 MB | Fastest | Good | `ggml-tiny.bin` |
   | Base | 142 MB | Fast | Better | `ggml-base.bin` |
   | Small | 466 MB | Moderate | Very Good | `ggml-small.bin` |
   | Medium | 1.5 GB | Slow | Excellent | `ggml-medium.bin` |
   | Large | 3 GB | Slowest | Best | `ggml-large-v3.bin` |

3. **Place downloaded models** in:
   - Windows: `C:\Users\<YourName>\.whisper\models\`
   - macOS/Linux: `~/.whisper/models/`

### Quick Download (using curl/wget):

```bash
cd ~/.whisper/models

# Tiny model (recommended for testing)
curl -LO https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin

# Base model (balanced)
curl -LO https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin

# Small model (good accuracy)
curl -LO https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin
```

## 🚀 Usage

1. **Load a Video**: Open a video file in glucose
2. **Generate Subtitles**: Click the AI brain icon (💡) in the controls
3. **Select Model**: Choose a Whisper model size based on your needs
4. **Wait for Processing**: 
   - Audio extraction (~5-10 seconds)
   - Model loading (~1-5 seconds)
   - Transcription (varies by video length and model)
5. **Subtitles Auto-Load**: Generated subtitles appear automatically
6. **Toggle On/Off**: Use the subtitle toggle button or press C/S keys

## ⚙️ How It Works

### Backend Process (Rust)

1. **Audio Extraction**: Uses FFmpeg to extract audio from video at 16kHz mono (Whisper's expected format)
2. **Model Loading**: Loads the selected Whisper model from `~/.whisper/models/`
3. **Transcription**: Processes audio through Whisper AI to generate text with timestamps
4. **SRT Generation**: Converts Whisper output to standard SRT format
5. **Auto-Save**: Saves `<videoname>.srt` alongside the video file

### Frontend (Svelte)

- Model selection dialog with size/speed info
- Real-time progress overlay with percentage
- Stage-based messaging (extracting, loading, transcribing)
- Automatic subtitle loading on completion

## 📁 File Structure

```
glucose/
├── src-tauri/
│   ├── Cargo.toml              # Added whisper-rs, tokio
│   └── src/
│       └── lib.rs              # Subtitle generation logic
└── src/
    └── routes/
        └── +page.svelte        # UI components & progress tracking
```

## 🔧 Build Instructions

After installing prerequisites:

```bash
# Install dependencies
pnpm install

# Build Rust backend
cd src-tauri
cargo build

# Run development mode
cd ..
pnpm tauri:dev

# Build for production
pnpm tauri:build
```

## 🐛 Troubleshooting

### Build Errors

**Error: "Unable to find libclang"**
```
Solution: Install LLVM and set LIBCLANG_PATH environment variable
Windows: set LIBCLANG_PATH=C:\Program Files\LLVM\bin
macOS/Linux: export LIBCLANG_PATH=/usr/lib/llvm-14/lib
```

**Error: "ffmpeg: command not found"**
```
Solution: Install FFmpeg and add to PATH
Test with: ffmpeg -version
```

### Runtime Errors

**Error: "Whisper model not found"**
```
Solution: Download the model file to ~/.whisper/models/
Check the model filename matches exactly (e.g., ggml-base.bin)
```

**Error: "Failed to execute ffmpeg"**
```
Solution: Ensure FFmpeg is in PATH
Windows: Run 'where ffmpeg' to verify
macOS/Linux: Run 'which ffmpeg' to verify
```

**Error: "No audio track found"**
```
Solution: Video may not have an audio track
Try a different video file
```

## 🎯 Performance Tips

- **First-time use**: Model loading takes a few seconds
- **Tiny/Base models**: Best for quick previews (~1-2x real-time)
- **Small model**: Good balance (~2-4x real-time)
- **Medium/Large models**: Best accuracy but slower (~4-10x real-time)
- **Video length**: A 10-minute video takes 5-10 minutes with Small model
- **Disk space**: Keep at least 1GB free for temporary audio files

## 🛣️ Future Enhancements

- [ ] Batch processing multiple videos
- [ ] Language selection (currently auto-detect)
- [ ] Translation to English option
- [ ] Custom model path configuration
- [ ] Progress cancellation
- [ ] Subtitle editing before saving
- [ ] Multiple subtitle track support
- [ ] Integration with online subtitle databases

## 📄 License

This feature is part of glucose and follows the same Mozilla Public License 2.0.

## 🙏 Credits

- [Whisper](https://github.com/openai/whisper) by OpenAI
- [whisper-rs](https://github.com/tazz4843/whisper-rs) Rust bindings
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp) C/C++ implementation
