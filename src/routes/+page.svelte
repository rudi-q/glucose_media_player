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

  onMount(() => {
    // Listen for file open events from Rust
    listen<string>("open-file", (event) => {
      loadVideo(event.payload);
    });

    // Listen for drag and drop events
    listen<string[]>("tauri://drag-drop", (event) => {
      if (event.payload && event.payload.length > 0) {
        loadVideo(event.payload[0]);
      }
    });

    // Keyboard shortcuts
    document.addEventListener("keydown", handleKeyPress);
    document.addEventListener("click", handleClickOutside);
    
    // Load audio devices
    loadAudioDevices();
    
    // Load recent videos asynchronously
    (async () => {
      try {
        const videos = await invoke<VideoFile[]>("get_recent_videos");
        recentVideos = videos;
      } catch (err) {
        console.error("Failed to load recent videos:", err);
      } finally {
        loadingRecent = false;
      }
    })();

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

  function loadVideo(path: string) {
    const src = convertFileSrc(path);
    videoSrc = src;
    if (videoElement) {
      videoElement.load();
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
    const { getCurrentWindow } = await import('@tauri-apps/api/window');
    await getCurrentWindow().close();
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

  function handleMouseMove() {
    // This is now only called from video container, not controls
    // Don't automatically show controls on video area
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
    if (showAudioMenu) {
      const target = e.target as HTMLElement;
      if (!target.closest('.audio-device-selector')) {
        showAudioMenu = false;
      }
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
      // Request microphone permission to get device labels
      await navigator.mediaDevices.getUserMedia({ audio: true }).then(stream => {
        stream.getTracks().forEach(track => track.stop());
      }).catch(() => {});
      
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
      
      video.onloadedmetadata = () => {
        // Seek to 1 second or 10% of video
        video.currentTime = Math.min(1, video.duration * 0.1);
      };
      
      video.onseeked = () => {
        canvas.width = 320;
        canvas.height = 180;
        ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
        const thumbnail = canvas.toDataURL('image/jpeg', 0.7);
        thumbnailCache.set(videoPath, thumbnail);
        resolve(thumbnail);
      };
      
      video.onerror = () => resolve('');
      
      video.src = convertFileSrc(videoPath);
    });
  }
</script>

<main 
  class="player-container"
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
  onmouseenter={() => showCloseButton = true}
  onmouseleave={() => showCloseButton = false}
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
          <h1>glucose</h1>
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
      ></video>
    </div>

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
            <button class="control-button" onclick={toggleMute} title="Mute (M)">
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
            <input 
              type="range" 
              class="volume-slider"
              min="0" 
              max="1" 
              step="0.01" 
              bind:value={volume}
              oninput={(e) => { if (videoElement) videoElement.volume = (e.target as HTMLInputElement).valueAsNumber; }}
              title="Volume: {Math.round(volume * 100)}%"
            />
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

  .library-header h1 {
    font-size: 2.5rem;
    font-weight: 200;
    letter-spacing: -0.02em;
    color: #fff;
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
    object-fit: cover;
    transition: transform 0.3s ease;
  }

  .video-card:hover .thumbnail-img {
    transform: scale(1.05);
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
    background: rgba(255, 255, 255, 0.2);
    cursor: pointer;
    margin-bottom: 1rem;
    position: relative;
    transition: height 0.15s ease;
    border-radius: 2px;
    overflow: visible;
  }

  .progress-bar:hover,
  .progress-bar.scrubbing {
    height: 6px;
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

  .time {
    font-size: 0.875rem;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.01em;
    opacity: 0.9;
  }

  .volume-control {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .volume-slider {
    width: 80px;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: rgba(255, 255, 255, 0.2);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }

  .volume-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    cursor: pointer;
    transition: transform 0.15s ease;
  }

  .volume-slider::-moz-range-thumb {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: #fff;
    cursor: pointer;
    border: none;
    transition: transform 0.15s ease;
  }

  .volume-slider::-webkit-slider-thumb:hover {
    transform: scale(1.2);
  }

  .volume-slider::-moz-range-thumb:hover {
    transform: scale(1.2);
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
</style>
