<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";

  let videoElement = $state<HTMLVideoElement>();
  let backgroundVideo = $state<HTMLVideoElement>();
  let previewVideo = $state<HTMLVideoElement>();
  let previewCanvas = $state<HTMLCanvasElement>();
  let videoSrc = $state<string | null>(null);
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state(1);
  let showControls = $state(false);
  let hideControlsTimeout: number;
  let isDragging = $state(false);
  let isScrubbing = $state(false);
  let wasPlayingBeforeScrub = $state(false);
  let isFullscreen = $state(false);
  let isCinematicMode = $state(true); // Default to cinematic mode
  let showPreview = $state(false);
  let previewTime = $state(0);
  let previewPosition = $state(0);
  let shouldLoadGallery = $state(true); // Control whether to load gallery
  
  interface VideoFile {
    path: string;
    name: string;
    size: number;
    modified: number;
  }
  
  let recentVideos = $state<VideoFile[]>([]);
  let loadingRecent = $state(true);
  let thumbnailCache = $state<Map<string, string>>(new Map());
  
  let audioDevices = $state<MediaDeviceInfo[]>([]);
  let selectedAudioDevice = $state<string>('default');
  let showAudioMenu = $state(false);
  let selectedVideoIndex = $state(0);
  let showCloseButton = $state(false);
  let hideCloseButtonTimeout: number;
  let showVolumeMenu = $state(false);
  let subtitleSrc = $state<string | null>(null);
  let subtitlesEnabled = $state(true);
  let trackElement = $state<HTMLTrackElement>();
  let isGeneratingSubtitles = $state(false);
  let generationProgress = $state(0);
  let generationMessage = $state("");
  let showModelSelector = $state(false);
  let currentVideoPath = $state<string | null>(null);

  onMount(() => {
    // Listen for file open events from Rust
    listen<string>("open-file", (event) => {
      loadVideo(event.payload);
      shouldLoadGallery = false; // Skip gallery when file is opened via association
      // Mark file as processed
      invoke("mark_file_processed").catch(console.error);
    });

    // Listen for drag and drop events
    listen<string[]>("tauri://drag-drop", (event) => {
      if (event.payload && event.payload.length > 0) {
        loadVideo(event.payload[0]);
      }
    });
    
    // Listen for subtitle generation progress
    listen<{stage: string, progress: number, message: string}>("subtitle-generation-progress", (event) => {
      generationProgress = event.payload.progress;
      generationMessage = event.payload.message;
      
      if (event.payload.stage === "complete") {
        setTimeout(() => {
          isGeneratingSubtitles = false;
          generationProgress = 0;
          generationMessage = "";
        }, 2000);
      } else if (event.payload.stage === "error") {
        isGeneratingSubtitles = false;
      }
    });

    // Keyboard shortcuts
    document.addEventListener("keydown", handleKeyPress);
    document.addEventListener("click", handleClickOutside);
    
    // Load audio devices
    loadAudioDevices();
    
    // Load recent videos asynchronously (only if no file was opened)
    (async () => {
      // Small delay to check if a file is being opened via file association
      await new Promise(resolve => setTimeout(resolve, 100));
      
      if (shouldLoadGallery && !videoSrc) {
        try {
          const videos = await invoke<VideoFile[]>("get_recent_videos");
          recentVideos = videos;
        } catch (err) {
          console.error("Failed to load recent videos:", err);
        } finally {
          loadingRecent = false;
        }
      } else {
        loadingRecent = false;
      }
    })();

    // Notify backend that frontend is ready
    invoke("frontend_ready").catch(console.error);

    return () => {
      document.removeEventListener("keydown", handleKeyPress);
      document.removeEventListener("click", handleClickOutside);
    };
  });

  function handleKeyPress(e: KeyboardEvent) {
    // Close app
    if (e.key === "Escape") {
      e.preventDefault();
      closeApp();
      return;
    }

    // Home/Back
    if (e.key === "Backspace" && videoSrc) {
      e.preventDefault();
      goHome();
      return;
    }

    // Video playback controls
    if (videoSrc) {
      switch (e.key) {
        case " ":
        case "k":
          e.preventDefault();
          togglePlay();
          break;
        case "f":
          toggleCinematicMode();
          break;
        case "m":
          toggleMute();
          break;
        case "c":
        case "s":
          e.preventDefault();
          toggleSubtitles();
          break;
        case "ArrowLeft":
          e.preventDefault();
          skip(-5);
          break;
        case "ArrowRight":
          e.preventDefault();
          skip(5);
          break;
        case "ArrowUp":
          e.preventDefault();
          adjustVolume(0.1);
          break;
        case "ArrowDown":
          e.preventDefault();
          adjustVolume(-0.1);
          break;
      }
      return;
    }

    // Recent videos navigation
    if (!videoSrc && recentVideos.length > 0) {
      switch (e.key) {
        case "ArrowLeft":
          e.preventDefault();
          selectedVideoIndex = Math.max(0, selectedVideoIndex - 1);
          break;
        case "ArrowRight":
          e.preventDefault();
          selectedVideoIndex = Math.min(recentVideos.length - 1, selectedVideoIndex + 1);
          break;
        case "ArrowUp":
          e.preventDefault();
          // Move up one row (assuming 4 columns grid)
          selectedVideoIndex = Math.max(0, selectedVideoIndex - 4);
          break;
        case "ArrowDown":
          e.preventDefault();
          // Move down one row
          selectedVideoIndex = Math.min(recentVideos.length - 1, selectedVideoIndex + 4);
          break;
        case "Enter":
        case " ":
          e.preventDefault();
          if (recentVideos[selectedVideoIndex]) {
            loadVideo(recentVideos[selectedVideoIndex].path);
          }
          break;
      }
    }
  }

  async function openFileDialog() {
    const result = await invoke<string | null>("open_file_dialog");
    if (result) {
      loadVideo(result);
    }
  }

  async function openSubtitleDialog() {
    const result = await invoke<string | null>("open_subtitle_dialog");
    if (result) {
      await loadSubtitle(result);
    }
  }

  async function loadVideo(path: string) {
    const src = convertFileSrc(path);
    videoSrc = src;
    currentVideoPath = path;  // Store original path for subtitle generation
    
    // Reset subtitles when loading new video
    // Revoke blob URL to prevent memory leak
    if (subtitleSrc && subtitleSrc.startsWith('blob:')) {
      try {
        URL.revokeObjectURL(subtitleSrc);
      } catch (err) {
        console.error('Failed to revoke subtitle blob URL:', err);
      }
    }
    subtitleSrc = null;
    
    if (videoElement) {
      videoElement.load();
    }
    
    // Auto-detect and load subtitle file
    try {
      const subtitlePath = await invoke<string | null>('find_subtitle_for_video', { videoPath: path });
      if (subtitlePath) {
        console.log('Auto-loading subtitle:', subtitlePath);
        await loadSubtitle(subtitlePath);
      }
    } catch (err) {
      console.log('No subtitle found or error:', err);
    }
  }

  async function loadSubtitle(path: string) {
    try {
      if (import.meta.env.DEV) {
        console.log('=== LOADING SUBTITLE ===');
        console.log('Path:', path);
      }
      
      // Read the subtitle file content
      const { readTextFile } = await import('@tauri-apps/plugin-fs');
      const content = await readTextFile(path);
      
      if (import.meta.env.DEV) {
        console.log('Subtitle content loaded, length:', content.length);
        console.log('First 200 chars:', content.substring(0, 200));
      }
      
      // Determine subtitle format and handle accordingly
      let vttContent: string;
      const ext = path.toLowerCase().split('.').pop() || '';
      
      if (ext === 'vtt' || content.startsWith('WEBVTT')) {
        // Already in WebVTT format
        if (import.meta.env.DEV) console.log('Subtitle is WebVTT format');
        vttContent = content;
      } else if (ext === 'srt') {
        // Convert SRT to WebVTT
        if (import.meta.env.DEV) console.log('Converting SRT to WebVTT');
        vttContent = convertSrtToVtt(content);
        if (import.meta.env.DEV) console.log('WebVTT first 200 chars:', vttContent.substring(0, 200));
      } else if (ext === 'ass' || ext === 'ssa' || ext === 'sub') {
        // Reject unsupported formats gracefully
        console.error(`Unsupported subtitle format: ${ext}`);
        alert(`Sorry, ${ext.toUpperCase()} subtitle format is not yet supported.\n\nPlease convert your subtitles to SRT or VTT format.\n\nSupported formats: SRT, VTT`);
        
        // Clean up previous subtitle if any
        if (subtitleSrc && subtitleSrc.startsWith('blob:')) {
          URL.revokeObjectURL(subtitleSrc);
        }
        subtitleSrc = null;
        subtitlesEnabled = false;
        return;
      } else {
        // Unknown format - try to detect by content
        if (content.includes('[Script Info]') || content.includes('Dialogue:')) {
          console.error('Detected ASS/SSA format without proper extension');
          alert('This appears to be an ASS/SSA subtitle file, which is not yet supported.\n\nPlease convert to SRT or VTT format.');
        } else if (/^\{\d+\}\{\d+\}/.test(content)) {
          console.error('Detected MicroDVD format');
          alert('This appears to be a MicroDVD subtitle file, which is not yet supported.\n\nPlease convert to SRT or VTT format.');
        } else {
          console.error('Unknown subtitle format');
          alert('Unsupported subtitle file format.\n\nSupported formats: SRT, VTT');
        }
        
        // Clean up previous subtitle if any
        if (subtitleSrc && subtitleSrc.startsWith('blob:')) {
          URL.revokeObjectURL(subtitleSrc);
        }
        subtitleSrc = null;
        subtitlesEnabled = false;
        return;
      }
      
      // Create a blob URL from the content
      const blob = new Blob([vttContent], { type: 'text/vtt;charset=utf-8' });
      const blobUrl = URL.createObjectURL(blob);
      
      // Revoke previous blob URL if exists
      if (subtitleSrc && subtitleSrc.startsWith('blob:')) {
        URL.revokeObjectURL(subtitleSrc);
      }
      
      subtitleSrc = blobUrl;
      subtitlesEnabled = true;
      
      if (import.meta.env.DEV) {
        console.log('Subtitle blob URL created:', blobUrl);
        console.log('=== SUBTITLE LOADING COMPLETE ===');
      }
    } catch (err) {
      console.error('Failed to load subtitle:', err);
      alert('Failed to load subtitle file: ' + err);
    }
  }
  
  function convertSrtToVtt(srt: string): string {
    // Convert SRT to WebVTT format
    let vtt = 'WEBVTT\n\n';
    
    // Clean up the SRT content
    // Remove BOM if present
    let cleanSrt = srt.replace(/^\uFEFF/, '');
    
    // Replace SRT timestamps (00:00:00,000) with VTT timestamps (00:00:00.000)
    // SRT format: 00:00:20,000 --> 00:00:24,400
    // VTT format: 00:00:20.000 --> 00:00:24.400
    cleanSrt = cleanSrt.replace(/(\d{2}:\d{2}:\d{2}),(\d{3})/g, '$1.$2');
    
    vtt += cleanSrt;
    
    return vtt;
  }

  function toggleSubtitles() {
    if (!trackElement?.track || !subtitleSrc) return;
    
    subtitlesEnabled = !subtitlesEnabled;
    trackElement.track.mode = subtitlesEnabled ? 'showing' : 'hidden';
    
    if (import.meta.env.DEV) {
      console.log('Subtitles toggled:', subtitlesEnabled, 'mode:', trackElement.track.mode);
    }
  }

  function goHome() {
    videoSrc = null;
    if (videoElement) {
      videoElement.pause();
    }
    if (backgroundVideo) {
      backgroundVideo.pause();
    }
    isPlaying = false;
  }

  async function closeApp() {
    console.log('closeApp called');
    try {
      // Try using the process plugin first (recommended for Tauri 2)
      console.log('Attempting to use process plugin...');
      const { exit } = await import('@tauri-apps/plugin-process');
      console.log('Process plugin imported, calling exit(0)...');
      await exit(0);
    } catch (err) {
      console.error('Failed to exit app with process plugin:', err);
      // Fallback to window API
      try {
        console.log('Attempting fallback to window API...');
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const window = getCurrentWindow();
        console.log('Window obtained:', window);
        await window.close();
        console.log('Window close called');
      } catch (fallbackErr) {
        console.error('Fallback close also failed:', fallbackErr);
        // Last resort: try using invoke to call a backend close command
        try {
          console.log('Trying invoke as last resort...');
          await invoke('exit_app');
        } catch (invokeErr) {
          console.error('Invoke exit_app also failed:', invokeErr);
        }
      }
    }
  }

  function togglePlay() {
    if (!videoElement) return;
    if (videoElement.paused) {
      videoElement.play();
      if (backgroundVideo) backgroundVideo.play();
      isPlaying = true;
    } else {
      videoElement.pause();
      if (backgroundVideo) backgroundVideo.pause();
      isPlaying = false;
    }
  }

  function skip(seconds: number) {
    if (!videoElement) return;
    videoElement.currentTime += seconds;
  }

  function toggleMute() {
    if (!videoElement) return;
    videoElement.muted = !videoElement.muted;
  }

  function adjustVolume(delta: number) {
    if (!videoElement) return;
    const newVolume = Math.max(0, Math.min(1, volume + delta));
    volume = newVolume;
    videoElement.volume = newVolume;
  }

  function handleTrackLoad() {
    // Guard: return early if no track element or subtitles are disabled
    if (!subtitlesEnabled || !trackElement || !trackElement.track) {
      if (import.meta.env.DEV) {
        console.log('Track load handler skipped:', { 
          subtitlesEnabled, 
          hasTrackElement: !!trackElement,
          hasTrack: !!trackElement?.track 
        });
      }
      return;
    }
    
    // Respect the subtitlesEnabled state when setting track mode
    trackElement.track.mode = subtitlesEnabled ? 'showing' : 'hidden';
    
    if (import.meta.env.DEV) {
      console.log('Track loaded successfully, mode set to:', trackElement.track.mode);
      console.log('Track cues:', trackElement.track.cues?.length || 0);
      console.log('Track activeCues:', trackElement.track.activeCues?.length || 0);
      
      // Add cuechange listener to monitor when cues become active
      trackElement.track.addEventListener('cuechange', () => {
        if (!trackElement) return;
        console.log('Cue changed, active cues:', trackElement.track.activeCues?.length || 0);
        if (trackElement.track.activeCues && trackElement.track.activeCues.length > 0) {
          const cue = trackElement.track.activeCues[0] as VTTCue;
          console.log('Current cue text:', cue.text);
        }
      });
    }
  }

  function seek(e: MouseEvent) {
    if (!videoElement) return;
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const percent = (e.clientX - rect.left) / rect.width;
    videoElement.currentTime = percent * duration;
  }

  function startScrubbing(e: MouseEvent) {
    if (!videoElement) return;
    isScrubbing = true;
    wasPlayingBeforeScrub = !videoElement.paused;
    if (wasPlayingBeforeScrub) {
      videoElement.pause();
    }
    
    const progressBar = e.currentTarget as HTMLElement;
    
    const updateProgress = (clientX: number) => {
      const rect = progressBar.getBoundingClientRect();
      const percent = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width));
      const newTime = percent * duration;
      
      // Use fastSeek for instant frame updates if available
      if ('fastSeek' in videoElement!) {
        (videoElement as any).fastSeek(newTime);
      } else {
        videoElement!.currentTime = newTime;
      }
    };
    
    updateProgress(e.clientX);
    
    const handleMouseMove = (e: MouseEvent) => {
      if (isScrubbing) {
        updateProgress(e.clientX);
      }
    };
    
    const handleMouseUp = () => {
      isScrubbing = false;
      if (wasPlayingBeforeScrub) {
        videoElement!.play();
      }
      document.removeEventListener('mousemove', handleMouseMove);
      document.removeEventListener('mouseup', handleMouseUp);
    };
    
    document.addEventListener('mousemove', handleMouseMove);
    document.addEventListener('mouseup', handleMouseUp);
  }

  function toggleCinematicMode() {
    isCinematicMode = !isCinematicMode;
  }

  function toggleFullscreen() {
    if (!document.fullscreenElement) {
      document.documentElement.requestFullscreen();
      isFullscreen = true;
    } else {
      document.exitFullscreen();
      isFullscreen = false;
    }
  }

  function handleMainContainerMouseMove() {
    // Show close button on mouse movement
    showCloseButton = true;
    
    // Clear existing timeout
    clearTimeout(hideCloseButtonTimeout);
    
    // Hide close button after 1 second of inactivity
    hideCloseButtonTimeout = setTimeout(() => {
      showCloseButton = false;
    }, 1000);
  }

  function handleControlsEnter() {
    showControls = true;
    clearTimeout(hideControlsTimeout);
  }

  function handleControlsLeave() {
    hideControlsTimeout = setTimeout(() => {
      showControls = false;
    }, 500);
  }

  function handleClickOutside(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (showAudioMenu && !target.closest('.audio-device-selector')) {
      showAudioMenu = false;
    }
    if (showVolumeMenu && !target.closest('.volume-control')) {
      showVolumeMenu = false;
    }
    if (showModelSelector && !target.closest('.ai-subtitle-generator')) {
      showModelSelector = false;
    }
  }

  function handleTimeUpdate() {
    if (!videoElement) return;
    currentTime = videoElement.currentTime;
    // Sync background video
    if (backgroundVideo && Math.abs(backgroundVideo.currentTime - videoElement.currentTime) > 0.1) {
      backgroundVideo.currentTime = videoElement.currentTime;
    }
  }

  function handleLoadedMetadata() {
    if (!videoElement) return;
    duration = videoElement.duration;
    
    // Auto-play when video loads
    videoElement.play().catch(err => {
      console.log('Auto-play prevented:', err);
    });
    // Start background video
    if (backgroundVideo) {
      backgroundVideo.play().catch(() => {});
    }
    isPlaying = true;
    
    // Show controls briefly when video loads
    showControls = true;
    hideControlsTimeout = setTimeout(() => {
      showControls = false;
    }, 3000);
  }

  function formatTime(seconds: number): string {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    
    if (h > 0) {
      return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    }
    return `${m}:${s.toString().padStart(2, '0')}`;
  }

  function handleDragOver(e: DragEvent) {
    e.preventDefault();
    isDragging = true;
  }

  function handleDragLeave() {
    isDragging = false;
  }

  function handleDrop(e: DragEvent) {
    e.preventDefault();
    isDragging = false;
  }

  function handleProgressHover(e: MouseEvent) {
    if (!videoElement || !previewVideo || !previewCanvas || isScrubbing) return;
    
    const progressBar = e.currentTarget as HTMLElement;
    const rect = progressBar.getBoundingClientRect();
    const percent = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    
    previewTime = percent * duration;
    previewPosition = e.clientX - rect.left;
    showPreview = true;
    
    // Update preview video time
    previewVideo.currentTime = previewTime;
  }

  function handleProgressLeave() {
    showPreview = false;
  }

  function drawPreview() {
    if (!previewVideo || !previewCanvas) return;
    
    const ctx = previewCanvas.getContext('2d');
    if (!ctx) return;
    
    // Draw video frame to canvas
    ctx.drawImage(previewVideo, 0, 0, previewCanvas.width, previewCanvas.height);
  }

  async function loadAudioDevices() {
    try {
      const devices = await navigator.mediaDevices.enumerateDevices();
      const outputDevices = devices.filter(device => device.kind === 'audiooutput');
      audioDevices = outputDevices;
      console.log('Audio output devices found:', outputDevices);
    } catch (err) {
      console.error('Failed to load audio devices:', err);
    }
  }

  async function changeAudioOutput(deviceId: string) {
    if (!videoElement) return;
    
    try {
      // @ts-ignore - setSinkId is not in TS types but supported in browsers
      if (typeof videoElement.setSinkId !== 'undefined') {
        await videoElement.setSinkId(deviceId);
        selectedAudioDevice = deviceId;
        showAudioMenu = false;
        console.log('Audio output changed to:', deviceId);
      }
    } catch (err) {
      console.error('Failed to change audio output:', err);
    }
  }

  function toggleAudioMenu() {
    showAudioMenu = !showAudioMenu;
  }

  function toggleVolumeMenu() {
    showVolumeMenu = !showVolumeMenu;
  }

  async function generateThumbnail(videoPath: string): Promise<string> {
    // Check cache first
    if (thumbnailCache.has(videoPath)) {
      return thumbnailCache.get(videoPath)!;
    }

    return new Promise((resolve) => {
      const video = document.createElement('video');
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');
      
      if (!ctx) {
        resolve('');
        return;
      }

      video.muted = true;
      video.preload = 'metadata';
      video.crossOrigin = 'anonymous'; // Allow canvas export
      
      video.onloadedmetadata = () => {
        // Seek to 1 second or 10% of video
        video.currentTime = Math.min(1, video.duration * 0.1);
      };
      
      video.onseeked = () => {
        try {
          // Calculate canvas dimensions based on video aspect ratio
          const targetWidth = 320;
          const aspectRatio = video.videoWidth / video.videoHeight;
          
          canvas.width = targetWidth;
          canvas.height = Math.round(targetWidth / aspectRatio);
          
          ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
          const thumbnail = canvas.toDataURL('image/jpeg', 0.7);
          thumbnailCache.set(videoPath, thumbnail);
          resolve(thumbnail);
        } catch (err) {
          // If canvas is tainted, just resolve empty
          if (import.meta.env.DEV) {
            console.log('Thumbnail generation skipped (CORS):', videoPath);
          }
          resolve('');
        }
      };
      
      video.onerror = () => resolve('');
      
      video.src = convertFileSrc(videoPath);
    });
  }
  
  function toggleModelSelector() {
    showModelSelector = !showModelSelector;
  }
  
  async function startSubtitleGeneration(modelSize: string) {
    if (!currentVideoPath) {
      alert('No video loaded');
      return;
    }
    
    showModelSelector = false;
    isGeneratingSubtitles = true;
    generationProgress = 0;
    generationMessage = 'Starting subtitle generation...';
    
    try {
      const subtitlePath = await invoke<string>('generate_subtitles', {
        videoPath: currentVideoPath,
        modelSize: modelSize
      });
      
      // Auto-load the generated subtitle
      await loadSubtitle(subtitlePath);
    } catch (err) {
      console.error('Failed to generate subtitles:', err);
      alert(`Subtitle generation failed: ${err}`);
      isGeneratingSubtitles = false;
      generationProgress = 0;
      generationMessage = '';
    }
  }
</script>

<main 
  class="player-container"
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
  onmousemove={handleMainContainerMouseMove}
>
  <button class="close-button" class:visible={showCloseButton} onclick={closeApp} title="Close (Esc)">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <line x1="18" y1="6" x2="6" y2="18"></line>
      <line x1="6" y1="6" x2="18" y2="18"></line>
    </svg>
  </button>
  {#if !videoSrc}
    <div class="empty-state" class:dragging={isDragging}>
      <div class="library-container">
        <div class="library-header">
          <img src="/logo-dark.svg" alt="glucose" class="logo" />
          <button class="open-button" onclick={openFileDialog}>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"></path>
            </svg>
            Open Video
          </button>
        </div>
        
        {#if loadingRecent}
          <div class="loading">Scanning for videos...</div>
        {:else if recentVideos.length === 0}
          <div class="empty-content">
            <svg width="64" height="64" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
              <polygon points="5 3 19 12 5 21 5 3"></polygon>
            </svg>
            <p>No recent videos found</p>
            <p class="hint">Drop a video file or click Open Video above</p>
          </div>
        {:else}
          <div class="recent-section">
            <h2>Recent Videos</h2>
            <div class="video-grid">
              {#each recentVideos as video, index}
                <button 
                  class="video-card" 
                  class:selected={selectedVideoIndex === index}
                  onclick={() => loadVideo(video.path)}
                >
                  <div class="video-thumbnail">
                    {#await generateThumbnail(video.path)}
                      <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                        <polygon points="5 3 19 12 5 21 5 3"></polygon>
                      </svg>
                    {:then thumbnail}
                      {#if thumbnail}
                        <img src={thumbnail} alt={video.name} class="thumbnail-img" />
                      {:else}
                        <svg width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
                          <polygon points="5 3 19 12 5 21 5 3"></polygon>
                        </svg>
                      {/if}
                    {/await}
                    <div class="play-overlay">
                      <svg width="32" height="32" viewBox="0 0 24 24" fill="white" stroke="none">
                        <polygon points="5 3 19 12 5 21 5 3"></polygon>
                      </svg>
                    </div>
                  </div>
                  <div class="video-info">
                    <div class="video-name" title={video.name}>{video.name}</div>
                    <div class="video-meta">
                      {(video.size / (1024 * 1024)).toFixed(1)} MB
                    </div>
                  </div>
                </button>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    </div>
  {:else}
    <div class="video-container" class:cinematic={isCinematicMode} class:fullscreen={!isCinematicMode}>
      {#if isCinematicMode}
        <!-- Blurred background video for cinematic mode -->
        <!-- svelte-ignore a11y_media_has_caption -->
        <video
          bind:this={backgroundVideo}
          class="background-video"
          src={videoSrc}
          muted
          aria-hidden="true"
        ></video>
      {/if}

      <!-- Main video -->
      <!-- svelte-ignore a11y_media_has_caption -->
      <video
        bind:this={videoElement}
        class="main-video"
        class:cinematic-video={isCinematicMode}
        class:fullscreen-video={!isCinematicMode}
        src={videoSrc}
        ontimeupdate={handleTimeUpdate}
        onloadedmetadata={handleLoadedMetadata}
        onclick={togglePlay}
        crossorigin="anonymous"
      >
        {#if subtitleSrc}
          <track
            bind:this={trackElement}
            kind="subtitles"
            src={subtitleSrc}
            srclang="en"
            label="Subtitles"
            default
            onload={handleTrackLoad}
          />
        {/if}
      </video>
    </div>
    
    <!-- AI Subtitle Generation Progress Overlay -->
    {#if isGeneratingSubtitles}
      <div class="generation-overlay">
        <div class="generation-modal">
          <div class="generation-icon">
            <svg class="spinner" width="48" height="48" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"></circle>
            </svg>
          </div>
          <h3>Generating AI Subtitles</h3>
          <div class="progress-container">
            <div class="progress-track">
              <div class="progress-fill" style="width: {generationProgress}%"></div>
            </div>
            <div class="progress-percentage">{Math.round(generationProgress)}%</div>
          </div>
          <p class="generation-message">{generationMessage}</p>
        </div>
      </div>
    {/if}

    <!-- Hidden preview video for generating thumbnails -->
    <!-- svelte-ignore a11y_media_has_caption -->
    <video
      bind:this={previewVideo}
      src={videoSrc}
      class="preview-video"
      muted
      onseeked={drawPreview}
    ></video>

    <div 
      class="controls-zone" 
      role="region"
      aria-label="Video controls"
      onmouseenter={handleControlsEnter}
      onmouseleave={handleControlsLeave}
    >
    <div 
      class="controls" 
      class:visible={showControls} 
      class:cinematic-controls={isCinematicMode} 
      class:overlay-controls={!isCinematicMode}
    >
      <div 
        class="progress-bar" 
        class:scrubbing={isScrubbing}
        onmousedown={startScrubbing}
        onmousemove={handleProgressHover}
        onmouseleave={handleProgressLeave}
        role="slider" 
        aria-label="Video progress"
        aria-valuemin={0}
        aria-valuemax={duration}
        aria-valuenow={currentTime}
        tabindex="0"
      >
        {#if showPreview}
          <div class="preview-tooltip" style="left: {previewPosition}px">
            <canvas bind:this={previewCanvas} width="160" height="90" class="preview-canvas"></canvas>
            <div class="preview-time">{formatTime(previewTime)}</div>
          </div>
        {/if}
        <div class="progress-filled" style="width: {(currentTime / duration) * 100}%">
          <div class="progress-handle"></div>
        </div>
      </div>

      <div class="controls-row">
        <div class="controls-left">
          <button class="control-button" onclick={goHome} title="Home">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 9l9-7 9 7v11a2 2 0 01-2 2H5a2 2 0 01-2-2z"></path>
              <polyline points="9 22 9 12 15 12 15 22"></polyline>
            </svg>
          </button>
          <div class="time">
            {formatTime(currentTime)} / {formatTime(duration)}
          </div>
        </div>

        <div class="controls-center">
          <button class="control-button" onclick={togglePlay}>
            {#if isPlaying}
              <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                <rect x="6" y="4" width="4" height="16"></rect>
                <rect x="14" y="4" width="4" height="16"></rect>
              </svg>
            {:else}
              <svg width="24" height="24" viewBox="0 0 24 24" fill="currentColor">
                <polygon points="5 3 19 12 5 21 5 3"></polygon>
              </svg>
            {/if}
          </button>
        </div>

        <div class="controls-right">
          <div class="volume-control">
            <button class="control-button" onclick={toggleVolumeMenu} title="Volume">
              {#if videoElement?.muted || volume === 0}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M11 5L6 9H2v6h4l5 4V5z"></path>
                  <line x1="23" y1="9" x2="17" y2="15"></line>
                  <line x1="17" y1="9" x2="23" y2="15"></line>
                </svg>
              {:else if volume < 0.5}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M11 5L6 9H2v6h4l5 4V5z"></path>
                  <path d="M15.54 8.46a5 5 0 010 7.07"></path>
                </svg>
              {:else}
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M11 5L6 9H2v6h4l5 4V5z"></path>
                  <path d="M19.07 4.93a10 10 0 010 14.14M15.54 8.46a5 5 0 010 7.07"></path>
                </svg>
              {/if}
            </button>
            {#if showVolumeMenu}
              <div class="volume-menu">
                <input 
                  type="range" 
                  class="volume-slider-vertical"
                  min="0" 
                  max="1" 
                  step="0.01" 
                  aria-label="Volume"
                  aria-orientation="vertical"
                  bind:value={volume}
                  oninput={(e) => { if (videoElement) { videoElement.volume = (e.target as HTMLInputElement).valueAsNumber; if (videoElement.muted) videoElement.muted = false; } }}
                />
                <button class="mute-toggle" onclick={toggleMute} class:muted={videoElement?.muted}>
                  {#if videoElement?.muted}
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M11 5L6 9H2v6h4l5 4V5z"></path>
                      <line x1="23" y1="9" x2="17" y2="15"></line>
                      <line x1="17" y1="9" x2="23" y2="15"></line>
                    </svg>
                  {:else}
                    <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                      <path d="M11 5L6 9H2v6h4l5 4V5z"></path>
                    </svg>
                  {/if}
                </button>
              </div>
            {/if}
          </div>
          
          {#if audioDevices.length > 0}
            <div class="audio-device-selector">
              <button class="control-button" onclick={toggleAudioMenu} title="Audio output device">
                <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <circle cx="12" cy="12" r="2"></circle>
                  <path d="M12 2v4m0 12v4M4.93 4.93l2.83 2.83m8.48 8.48l2.83 2.83M2 12h4m12 0h4M4.93 19.07l2.83-2.83m8.48-8.48l2.83-2.83"></path>
                </svg>
              </button>
              {#if showAudioMenu}
                <div class="audio-menu">
                  {#each audioDevices as device}
                    <button 
                      class="audio-option" 
                      class:selected={selectedAudioDevice === device.deviceId}
                      onclick={() => changeAudioOutput(device.deviceId)}
                    >
                      {device.label || `Device ${device.deviceId.slice(0, 8)}`}
                    </button>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
          <button class="control-button" onclick={openSubtitleDialog} title="Load subtitles">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <rect x="2" y="7" width="20" height="10" rx="2" ry="2"></rect>
              <line x1="6" y1="11" x2="6.01" y2="11"></line>
              <line x1="10" y1="11" x2="14" y2="11"></line>
              <line x1="6" y1="15" x2="10" y2="15"></line>
              <line x1="14" y1="15" x2="18" y2="15"></line>
            </svg>
          </button>
          
          <!-- AI Subtitle Generation Button -->
          <div class="ai-subtitle-generator">
            <button 
              class="control-button" 
              class:generating={isGeneratingSubtitles}
              onclick={toggleModelSelector} 
              disabled={isGeneratingSubtitles}
              title="Generate AI Subtitles"
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M12 2a2 2 0 012 2c0 .74-.4 1.39-1 1.73V7h1a7 7 0 014 12.93V21a1 1 0 01-1 1H7a1 1 0 01-1-1v-1.07A7 7 0 0110 7h1V5.73c-.6-.34-1-.99-1-1.73a2 2 0 012-2z"></path>
                <circle cx="12" cy="12" r="3"></circle>
              </svg>
            </button>
            {#if showModelSelector && !isGeneratingSubtitles}
              <div class="model-selector">
                <div class="model-header">Select AI Model</div>
                <button class="model-option" onclick={() => startSubtitleGeneration('tiny')}>
                  <span class="model-name">Tiny</span>
                  <span class="model-desc">~75MB • Fastest</span>
                </button>
                <button class="model-option" onclick={() => startSubtitleGeneration('base')}>
                  <span class="model-name">Base</span>
                  <span class="model-desc">~142MB • Fast</span>
                </button>
                <button class="model-option" onclick={() => startSubtitleGeneration('small')}>
                  <span class="model-name">Small</span>
                  <span class="model-desc">~466MB • Balanced</span>
                </button>
                <button class="model-option" onclick={() => startSubtitleGeneration('medium')}>
                  <span class="model-name">Medium</span>
                  <span class="model-desc">~1.5GB • Accurate</span>
                </button>
                <button class="model-option" onclick={() => startSubtitleGeneration('large')}>
                  <span class="model-name">Large</span>
                  <span class="model-desc">~3GB • Best</span>
                </button>
              </div>
            {/if}
          </div>
          
          {#if subtitleSrc}
            <button 
              class="control-button" 
              class:subtitle-active={subtitlesEnabled}
              onclick={toggleSubtitles} 
              title="Toggle subtitles (C/S)"
            >
              <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z"></path>
                {#if !subtitlesEnabled}
                  <line x1="3" y1="3" x2="21" y2="21"></line>
                {/if}
              </svg>
            </button>
          {/if}
          <button class="control-button" onclick={openFileDialog} title="Open file (O)">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M3 7v10a2 2 0 002 2h14a2 2 0 002-2V9a2 2 0 00-2-2h-6l-2-2H5a2 2 0 00-2 2z"></path>
            </svg>
          </button>
          <button class="control-button" onclick={toggleCinematicMode} title="Toggle view mode (F)">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M8 3H5a2 2 0 00-2 2v3m18 0V5a2 2 0 00-2-2h-3m0 18h3a2 2 0 002-2v-3M3 16v3a2 2 0 002 2h3"></path>
            </svg>
          </button>
        </div>
      </div>
    </div>
    </div>
  {/if}
</main>

<style>
  :global(*) {
    margin: 0;
    padding: 0;
    box-sizing: border-box;
  }

  :global(body) {
    margin: 0;
    padding: 0;
    overflow: hidden;
    background: transparent;
    color: #fff;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif;
  }

  .player-container {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    cursor: default;
    overflow: hidden;
  }

  .close-button {
    position: fixed;
    top: 1.5rem;
    right: 1.5rem;
    width: 36px;
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.5);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 50%;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    transition: all 0.2s ease;
    z-index: 100;
    opacity: 0;
    pointer-events: none;
  }

  .close-button.visible {
    opacity: 1;
    pointer-events: all;
  }

  .close-button:hover {
    background: rgba(255, 0, 0, 0.8);
    border-color: rgba(255, 255, 255, 0.3);
    color: #fff;
    transform: scale(1.1);
  }

  .player-container:has(.empty-state) {
    /* Gallery screen - more opaque */
    background: rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(40px);
    -webkit-backdrop-filter: blur(40px);
  }

  .player-container:has(.video-container) {
    /* Video playback screen - more transparent in cinematic mode */
    background: rgba(0, 0, 0, 0.85);
    backdrop-filter: blur(40px);
    -webkit-backdrop-filter: blur(40px);
  }

  .player-container:has(.video-container.fullscreen) {
    /* Fullscreen mode - solid black, no blur */
    background: rgba(0, 0, 0, 1);
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  .empty-state {
    flex: 1;
    display: flex;
    background: transparent;
    transition: background 0.2s ease;
    overflow-y: auto;
  }

  .empty-state.dragging {
    background: #0a0a0a;
  }

  .library-container {
    width: 100%;
    max-width: 1400px;
    margin: 0 auto;
    padding: 3rem 2rem;
  }

  .library-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 3rem;
  }

  .library-header .logo {
    height: 48px;
    width: auto;
    opacity: 0.95;
  }

  .loading {
    text-align: center;
    padding: 4rem 0;
    color: rgba(255, 255, 255, 0.5);
    font-size: 0.95rem;
  }

  .empty-content {
    text-align: center;
    color: rgba(255, 255, 255, 0.7);
    padding: 4rem 0;
  }

  .empty-content svg {
    margin: 0 auto 1.5rem;
    opacity: 0.6;
  }

  .empty-content p {
    font-size: 0.95rem;
    margin-bottom: 0.5rem;
    opacity: 0.7;
  }

  .empty-content .hint {
    opacity: 0.4;
    font-size: 0.875rem;
  }

  .recent-section h2 {
    font-size: 1.25rem;
    font-weight: 400;
    margin-bottom: 1.5rem;
    color: rgba(255, 255, 255, 0.9);
  }

  .video-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
    gap: 1.5rem;
  }

  .video-card {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 0;
    cursor: pointer;
    transition: all 0.2s ease;
    text-align: left;
    overflow: hidden;
  }

  .video-card:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.15);
    transform: translateY(-2px);
  }

  .video-card.selected {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.3);
    box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.2);
  }

  .video-thumbnail {
    aspect-ratio: 16 / 9;
    background: rgba(255, 255, 255, 0.02);
    display: flex;
    align-items: center;
    justify-content: center;
    border-bottom: 1px solid rgba(255, 255, 255, 0.08);
    position: relative;
    overflow: hidden;
  }

  .video-thumbnail svg {
    opacity: 0.3;
  }

  .thumbnail-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: rgba(0, 0, 0, 0.3);
  }

  .play-overlay {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    opacity: 0;
    transition: opacity 0.2s ease;
    pointer-events: none;
    background: rgba(0, 0, 0, 0.5);
    border-radius: 50%;
    width: 56px;
    height: 56px;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .video-card:hover .play-overlay {
    opacity: 1;
  }

  .video-info {
    padding: 1rem;
  }

  .video-name {
    font-size: 0.875rem;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.9);
    margin-bottom: 0.5rem;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .video-meta {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.5);
  }

  .open-button {
    background: #fff;
    color: #000;
    border: none;
    padding: 0.75rem 1.5rem;
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    letter-spacing: 0.01em;
    border-radius: 6px;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .open-button:hover {
    background: rgba(255, 255, 255, 0.9);
    transform: translateY(-1px);
  }

  .open-button:active {
    transform: translateY(0);
  }

  .video-container {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  .video-container.cinematic {
    /* Cinematic mode - video centered with space below for controls */
    flex: 1;
  }

  .video-container.fullscreen {
    /* Fullscreen mode - video takes all space */
    position: absolute;
    inset: 0;
  }

  .background-video {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    min-width: 110%;
    min-height: 110%;
    width: auto;
    height: auto;
    object-fit: cover;
    filter: blur(100px) brightness(0.5);
    opacity: 0.08;
    z-index: 0;
    pointer-events: none;
  }

  .main-video {
    cursor: pointer;
    z-index: 1;
  }

  .cinematic-video {
    max-width: 85%;
    max-height: 70vh;
    width: auto;
    height: auto;
    object-fit: contain;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.5);
    border-radius: 6px;
  }

  .fullscreen-video {
    width: 100%;
    height: 100%;
    object-fit: contain;
  }

  .preview-video {
    position: absolute;
    width: 1px;
    height: 1px;
    opacity: 0;
    pointer-events: none;
    z-index: -1;
  }

  .controls-zone {
    position: relative;
  }

  .video-container.fullscreen ~ .controls-zone {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 10;
    min-height: 120px;
    pointer-events: all;
  }

  .controls {
    transition: opacity 0.25s ease;
  }

  .cinematic-controls {
    /* Controls below video in cinematic mode */
    position: relative;
    padding: 1.5rem 2rem 2rem;
    background: transparent;
    opacity: 0;
    pointer-events: none;
    z-index: 2;
    transition: opacity 0.25s ease;
  }

  .cinematic-controls.visible {
    opacity: 1;
    pointer-events: all;
  }

  .overlay-controls {
    /* Controls overlay on video in fullscreen mode */
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.85) 0%, rgba(0, 0, 0, 0.6) 60%, transparent 100%);
    padding: 2rem 1.5rem 1.5rem;
    opacity: 0;
    pointer-events: none;
  }

  .overlay-controls.visible {
    opacity: 1;
    pointer-events: all;
  }

  .progress-bar {
    width: 100%;
    height: 4px;
    background: transparent;
    cursor: pointer;
    margin-bottom: 1rem;
    position: relative;
    transition: height 0.15s ease, background 0.2s ease;
    border-radius: 2px;
    overflow: visible;
  }

  .progress-bar:hover,
  .progress-bar.scrubbing {
    height: 6px;
    background: rgba(255, 255, 255, 0.2);
  }

  .progress-filled {
    height: 100%;
    background: #fff;
    transition: width 0.1s linear;
    border-radius: 2px;
    position: relative;
  }

  .progress-handle {
    position: absolute;
    right: -6px;
    top: 50%;
    transform: translateY(-50%);
    width: 12px;
    height: 12px;
    background: #fff;
    border-radius: 50%;
    opacity: 0;
    transition: opacity 0.15s ease;
  }

  .progress-bar:hover .progress-handle,
  .progress-bar.scrubbing .progress-handle {
    opacity: 1;
  }

  .preview-tooltip {
    position: absolute;
    bottom: 100%;
    transform: translateX(-50%);
    margin-bottom: 12px;
    pointer-events: none;
    z-index: 10;
  }

  .preview-canvas {
    display: block;
    border: 2px solid #fff;
    border-radius: 4px;
    background: #000;
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.8);
  }

  .preview-time {
    text-align: center;
    margin-top: 6px;
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    background: rgba(0, 0, 0, 0.9);
    padding: 4px 8px;
    border-radius: 4px;
    color: #fff;
    font-weight: 500;
  }

  .controls-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .controls-left,
  .controls-center,
  .controls-right {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    flex: 1;
  }

  .controls-left {
    justify-content: flex-start;
  }

  .controls-center {
    justify-content: center;
  }

  .controls-right {
    justify-content: flex-end;
  }

  .control-button {
    background: none;
    border: none;
    color: #fff;
    cursor: pointer;
    padding: 0.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: opacity 0.15s ease;
    opacity: 0.9;
  }

  .control-button:hover {
    opacity: 1;
  }

  .control-button.subtitle-active {
    opacity: 1;
    color: #C065B6;
  }

  .time {
    font-size: 0.875rem;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.01em;
    opacity: 0.9;
  }

  .volume-control {
    position: relative;
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .volume-menu {
    position: absolute;
    bottom: 100%;
    left: 50%;
    transform: translateX(-50%);
    margin-bottom: 0.5rem;
    background: rgba(0, 0, 0, 0.95);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 1rem 0.75rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    z-index: 100;
  }

  .volume-slider-vertical {
    width: 4px;
    height: 100px;
    -webkit-appearance: slider-vertical;
    appearance: slider-vertical;
    background: rgba(255, 255, 255, 0.2);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
    writing-mode: bt-lr; /* IE */
    -webkit-appearance: slider-vertical; /* WebKit */
  }

  .volume-slider-vertical::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    cursor: pointer;
    transition: transform 0.15s ease;
  }

  .volume-slider-vertical::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    cursor: pointer;
    border: none;
    transition: transform 0.15s ease;
  }

  .volume-slider-vertical::-webkit-slider-thumb:hover {
    transform: scale(1.2);
  }

  .volume-slider-vertical::-moz-range-thumb:hover {
    transform: scale(1.2);
  }

  .mute-toggle {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 0.5rem;
    color: rgba(255, 255, 255, 0.9);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: all 0.15s ease;
  }

  .mute-toggle:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .mute-toggle.muted {
    background: rgba(255, 0, 0, 0.2);
    border-color: rgba(255, 0, 0, 0.3);
    color: #ff5555;
  }

  .audio-device-selector {
    position: relative;
  }

  .audio-menu {
    position: absolute;
    bottom: 100%;
    right: 0;
    margin-bottom: 0.5rem;
    background: rgba(0, 0, 0, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 0.5rem 0;
    min-width: 200px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    z-index: 100;
  }

  .audio-option {
    width: 100%;
    padding: 0.625rem 1rem;
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.9);
    text-align: left;
    cursor: pointer;
    font-size: 0.875rem;
    transition: background 0.15s ease;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .audio-option:hover {
    background: rgba(255, 255, 255, 0.1);
  }

  .audio-option.selected {
    background: rgba(255, 255, 255, 0.15);
    color: #fff;
    font-weight: 500;
  }

  /* Subtitle styling */
  /* !important required to override browser default subtitle styles */
  :global(video::cue) {
    background-color: rgba(0, 0, 0, 0.8) !important;
    color: #ffffff !important;
    font-size: 1.5em !important;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif !important;
    text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.9) !important;
    line-height: 1.4 !important;
    padding: 0.2em 0.5em !important;
  }
  
  /* Position subtitle container at bottom */
  /* !important required for cross-browser subtitle positioning override */
  :global(video::-webkit-media-text-track-container) {
    position: absolute !important;
    bottom: 0 !important;
    left: 0 !important;
    right: 0 !important;
    display: flex !important;
    flex-direction: column !important;
    justify-content: flex-end !important;
    padding-bottom: 8vh !important;
    pointer-events: none !important;
  }
  
  :global(video::-webkit-media-text-track-display) {
    font-size: 24px;
    padding-top: 2vh !important;
    text-align: center !important;
    width: 100% !important;
  }
  
  /* Firefox subtitle positioning */
  :global(video::cue-region) {
    position: absolute !important;
    bottom: 86vh !important;
    left: 0 !important;
    right: 0 !important;
  }
  
  /* AI Subtitle Generation Styles */
  .ai-subtitle-generator {
    position: relative;
  }
  
  .control-button.generating {
    color: #C065B6;
    opacity: 1;
    animation: pulse 1.5s ease-in-out infinite;
  }
  
  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50% { opacity: 0.5; }
  }
  
  .model-selector {
    position: absolute;
    bottom: 100%;
    right: 0;
    margin-bottom: 0.5rem;
    background: rgba(0, 0, 0, 0.95);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 0.75rem 0;
    min-width: 220px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    z-index: 100;
  }
  
  .model-header {
    padding: 0.5rem 1rem;
    font-size: 0.75rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.6);
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    margin-bottom: 0.5rem;
  }
  
  .model-option {
    width: 100%;
    padding: 0.75rem 1rem;
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.9);
    text-align: left;
    cursor: pointer;
    font-size: 0.875rem;
    transition: all 0.15s ease;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .model-option:hover {
    background: rgba(255, 255, 255, 0.1);
  }
  
  .model-name {
    font-weight: 600;
    color: #fff;
  }
  
  .model-desc {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.6);
  }
  
  .generation-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.85);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    animation: fadeIn 0.3s ease;
  }
  
  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  
  .generation-modal {
    background: rgba(20, 20, 20, 0.95);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    padding: 2.5rem;
    min-width: 400px;
    max-width: 500px;
    text-align: center;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
    animation: slideUp 0.3s ease;
  }
  
  @keyframes slideUp {
    from {
      transform: translateY(20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }
  
  .generation-icon {
    margin-bottom: 1.5rem;
    display: flex;
    justify-content: center;
  }
  
  .spinner {
    animation: spin 2s linear infinite;
    color: #C065B6;
  }
  
  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
  
  .generation-modal h3 {
    font-size: 1.5rem;
    font-weight: 600;
    margin-bottom: 1.5rem;
    color: #fff;
  }
  
  .progress-container {
    margin-bottom: 1.5rem;
  }
  
  .progress-track {
    width: 100%;
    height: 8px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    overflow: hidden;
    margin-bottom: 0.75rem;
  }
  
  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #C065B6, #8C77FF);
    border-radius: 4px;
    transition: width 0.3s ease;
    box-shadow: 0 0 10px rgba(192, 101, 182, 0.5);
  }
  
  .progress-percentage {
    font-size: 1.25rem;
    font-weight: 600;
    color: #C065B6;
    font-variant-numeric: tabular-nums;
  }
  
  .generation-message {
    font-size: 0.875rem;
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.5;
    margin: 0;
  }
</style>
