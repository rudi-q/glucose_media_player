<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount, getContext } from "svelte";
  import { goto } from "$app/navigation";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { 
    X, 
    Settings, 
    Play, 
    Pause, 
    Home, 
    VolumeX, 
    Volume1, 
    Volume2, 
    Captions, 
    CaptionsOff, 
    Maximize, 
    Loader2
  } from "lucide-svelte";
  import { appSettings, setupStore, type SetupStatus } from "$lib/stores/appStore";
  import { watchProgressStore, type WatchProgress } from "$lib/stores/watchProgressStore";
  import type { VideoInfo } from "$lib/types/video";
  import { loadSubtitleFile } from "$lib/utils/subtitles";
  import { formatTime, formatEstimatedTime, formatTimeForScreenReader } from "$lib/utils/time";
  
  let { data } = $props();
  
  // Video elements
  let videoElement = $state<HTMLVideoElement>();
  let backgroundVideo = $state<HTMLVideoElement>();
  let previewVideo = $state<HTMLVideoElement>();
  let previewCanvas = $state<HTMLCanvasElement>();
  let trackElement = $state<HTMLTrackElement>();
  
  // Playback state
  let videoSrc = $state<string | null>(null);
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state($appSettings.volume);
  let isMuted = $state($appSettings.isMuted);
  
  // UI state
  let showControls = $state(false);
  let hideControlsTimeout: ReturnType<typeof setTimeout>;
  let showCloseButton = $state(false);
  let hideCloseButtonTimeout: ReturnType<typeof setTimeout>;
  type ViewMode = 'cinematic' | 'fullscreen' | 'pip';
  let viewMode = $state<ViewMode>('cinematic');
  
  // Scrubbing/seeking state
  let isScrubbing = $state(false);
  let wasPlayingBeforeScrub = $state(false);
  let showPreview = $state(false);
  let previewTime = $state(0);
  let previewPosition = $state(0);
  let previewRaf: number | null = null;
  
  // Subtitle state
  let subtitleSrc = $state<string | null>(null);
  let subtitlesEnabled = $state(true);
  let subtitleFileName = $state<string | null>(null);
  let showSubtitleMenu = $state(false);
  let isGeneratingSubtitles = $state(false);
  let generationProgress = $state(0);
  let generationMessage = $state("");
  let showModelSelector = $state(false);
  
  // Context menu state
  let showContextMenu = $state(false);
  let contextMenuPosition = $state({ x: 0, y: 0 });
  let showConvertSubmenu = $state(false);
  
  // Audio/Volume state
  let showVolumeMenu = $state(false);
  let showAudioMenu = $state(false);
  let audioDevices = $state<MediaDeviceInfo[]>([]);
  let selectedAudioDevice = $state($appSettings.selectedAudioDevice);
  
  // Conversion state
  let isConverting = $state(false);
  let conversionProgress = $state(0);
  let conversionMessage = $state("");
  let currentVideoInfo = $state<VideoInfo | null>(null);
  let normalizedFormat = $derived(currentVideoInfo?.format?.toUpperCase() ?? '');
  
  // Progress tracking
  let currentVideoPath = $state<string | null>(null);
  let progressSaveInterval: ReturnType<typeof setInterval>;
  
  // Get context from layout
  const showSettings = getContext<() => void>('showSettings');
  const showSetupDialog = getContext<() => void>('showSetupDialog');
  const getSetupStatus = getContext<() => SetupStatus | null>('setupStatus');
  
  let setupStatus = $state(getSetupStatus());
  
  onMount(() => {
    let disposed = false;
    const unsubs: UnlistenFn[] = [];
    
    // Load the video
    (async () => {
      if (data.videoPath) {
        const src = convertFileSrc(data.videoPath);
        videoSrc = src;
        currentVideoPath = data.videoPath;
        
        // Auto-detect and load subtitle file
        try {
          const subtitlePath = await invoke<string | null>('find_subtitle_for_video', { videoPath: data.videoPath });
          if (subtitlePath) {
            console.log('Auto-loading subtitle:', subtitlePath);
            await loadSubtitle(subtitlePath);
          }
        } catch (err) {
          console.log('No subtitle found or error:', err);
        }
      }
    })();
    
    // Register Tauri event listeners
    (async () => {
      const results = await Promise.allSettled([
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
        }),
        // Listen for conversion progress
        listen<{stage: string, progress: number, message: string}>("conversion-progress", (event) => {
          conversionProgress = event.payload.progress;
          conversionMessage = event.payload.message;
          
          if (event.payload.stage === "complete") {
            setTimeout(() => {
              isConverting = false;
              conversionProgress = 0;
              conversionMessage = "";
            }, 2000);
          } else if (event.payload.stage === "error") {
            isConverting = false;
          }
        }),
      ]);
      
      for (const r of results) {
        if (r.status === "fulfilled") {
          const un = r.value;
          if (disposed) {
            try { un(); } catch (e) { console.error("Unlisten failed", e); }
          } else {
            unsubs.push(un);
          }
        } else {
          console.error("Failed to register Tauri listener:", r.reason);
        }
      }
    })();
    
    // Load audio devices
    loadAudioDevices();
    
    // Keyboard shortcuts
    document.addEventListener("keydown", handleKeyPress);
    document.addEventListener("click", handleClickOutside);
    
    return () => {
      disposed = true;
      // Clear progress save interval
      if (progressSaveInterval) {
        clearInterval(progressSaveInterval);
      }
      // Save progress one last time before unmount
      if (videoElement && currentVideoPath && duration > 0) {
        saveWatchProgress();
      }
      // Clean up subtitle blob URL
      if (subtitleSrc && subtitleSrc.startsWith('blob:')) {
        try {
          URL.revokeObjectURL(subtitleSrc);
        } catch (err) {
          console.error('Failed to revoke subtitle blob URL:', err);
        }
      }
      // Unregister event listeners
      for (const un of unsubs) {
        try { un(); } catch (e) { console.error("Unlisten failed", e); }
      }
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
    if (e.key === "Backspace") {
      e.preventDefault();
      goHome();
      return;
    }

    // Number keys 0-9 for scrubbing
    if (e.key >= '0' && e.key <= '9') {
      e.preventDefault();
      const percentage = parseInt(e.key) * 0.1;
      scrubToPercentage(percentage);
      return;
    }
    
    switch (e.key) {
      case " ":
      case "k":
        e.preventDefault();
        togglePlay();
        break;
      case "f":
        toggleViewMode();
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
  }
  
  async function saveWatchProgress() {
    if (!currentVideoPath || !videoElement || duration <= 0) return;
    
    const currentTime = videoElement.currentTime;
    const videoDuration = duration;
    
    try {
      await invoke('save_watch_progress', {
        videoPath: currentVideoPath,
        currentTime: currentTime,
        duration: videoDuration
      });
      
      // Update local state
      const timestamp = Date.now() / 1000;
      watchProgressStore.setProgress(currentVideoPath, {
        path: currentVideoPath,
        current_time: currentTime,
        duration: videoDuration,
        last_watched: timestamp
      });
    } catch (err) {
      console.error('Failed to save watch progress:', err);
    }
  }
  
  async function loadSubtitle(path: string) {
    const result = await loadSubtitleFile(path);
    if (result) {
      // Revoke previous blob URL if exists
      if (subtitleSrc && subtitleSrc.startsWith('blob:')) {
        URL.revokeObjectURL(subtitleSrc);
      }
      
      subtitleSrc = result.blobUrl;
      subtitleFileName = result.fileName;
      subtitlesEnabled = true;
    }
  }
  
  async function openSubtitleDialog() {
    const result = await invoke<string | null>("open_subtitle_dialog");
    if (result) {
      await loadSubtitle(result);
    }
  }
  
  function toggleSubtitles() {
    if (!trackElement?.track || !subtitleSrc) return;
    
    subtitlesEnabled = !subtitlesEnabled;
    trackElement.track.mode = subtitlesEnabled ? 'showing' : 'hidden';
  }
  
  function handleTrackLoad() {
    if (!subtitlesEnabled || !trackElement || !trackElement.track) return;
    trackElement.track.mode = subtitlesEnabled ? 'showing' : 'hidden';
  }
  
  async function goHome() {
    // Save progress before going home
    if (currentVideoPath && videoElement && duration > 0) {
      await saveWatchProgress();
    }
    
    // Clear progress save interval
    if (progressSaveInterval) {
      clearInterval(progressSaveInterval);
    }
    
    await goto('/');
  }
  
  async function closeApp() {
    try {
      const { exit } = await import('@tauri-apps/plugin-process');
      await exit(0);
    } catch (err) {
      console.error('Failed to exit app:', err);
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const window = getCurrentWindow();
        await window.close();
      } catch (fallbackErr) {
        console.error('Fallback close also failed:', fallbackErr);
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
  
  function scrubToPercentage(percentage: number) {
    if (!videoElement || !duration) return;
    const targetTime = duration * percentage;
    
    if ('fastSeek' in videoElement && typeof (videoElement as any).fastSeek === 'function') {
      (videoElement as any).fastSeek(targetTime);
    } else {
      videoElement.currentTime = targetTime;
    }
  }
  
  function toggleMute() {
    if (!videoElement) return;
    isMuted = !isMuted;
    videoElement.muted = isMuted;
    appSettings.updateMuted(isMuted);
  }
  
  function adjustVolume(delta: number) {
    if (!videoElement) return;
    const newVolume = Math.max(0, Math.min(1, volume + delta));
    volume = newVolume;
    videoElement.volume = newVolume;
    appSettings.updateVolume(newVolume);
    if (isMuted) {
      isMuted = false;
      videoElement.muted = false;
      appSettings.updateMuted(false);
    }
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
  
  async function toggleViewMode() {
    const modes: ViewMode[] = ['cinematic', 'fullscreen', 'pip'];
    const currentIndex = modes.indexOf(viewMode);
    const nextMode = modes[(currentIndex + 1) % modes.length];
    
    // Handle PiP transitions
    if (viewMode === 'pip') {
      // Exiting PiP mode
      try {
        // Restore transparent background
        document.body.style.background = 'transparent';
        
        // Clear inline video styles
        if (videoElement) {
          videoElement.style.cssText = '';
        }
        
        await invoke('exit_pip_mode');
      } catch (err) {
        console.error('Failed to exit PiP mode:', err);
      }
    }
    
    if (nextMode === 'pip') {
      // Entering PiP mode
      try {
        await invoke('enter_pip_mode');
        
        // Change mode immediately
        viewMode = nextMode;
        
        // Force solid background for PiP (transparency causes black screen)
        document.body.style.background = '#000';
        
        // Wait a moment for window resize, then trigger video reflow
        await new Promise(resolve => setTimeout(resolve, 150));
        
        if (videoElement) {
          const wasPlaying = !videoElement.paused;
          
          // Force PiP video sizing via inline styles
          videoElement.style.cssText = `
            width: 100% !important;
            height: 100% !important;
            max-width: 100% !important;
            max-height: 100% !important;
            object-fit: contain !important;
            position: absolute !important;
            top: 0 !important;
            left: 0 !important;
            visibility: visible !important;
            opacity: 1 !important;
            display: block !important;
          `;
          void videoElement.offsetHeight; // Force reflow
          
          // Restore playback state
          if (wasPlaying) {
            videoElement.play().catch(() => {});
          }
        }
        
        return; // Exit early since we already set viewMode
      } catch (err) {
        console.error('Failed to enter PiP mode:', err);
        return; // Don't change mode if entering PiP failed
      }
    }
    
    viewMode = nextMode;
  }
  
  function handleMainContainerMouseMove() {
    showCloseButton = true;
    clearTimeout(hideCloseButtonTimeout);
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
    if (showVolumeMenu && !target.closest('.volume-control')) {
      showVolumeMenu = false;
    }
    if (showSubtitleMenu && !target.closest('.subtitle-control')) {
      showSubtitleMenu = false;
    }
    if (showModelSelector && !(target.closest('.ai-subtitle-generator') || target.closest('.subtitle-control') || target.closest('.model-selector'))) {
      showModelSelector = false;
    }
    if (showContextMenu && !target.closest('.context-menu')) {
      showContextMenu = false;
    }
  }
  
  async function handleContextMenu(e: MouseEvent) {
    e.preventDefault();
    contextMenuPosition = { x: e.clientX, y: e.clientY };
    showContextMenu = true;
    showConvertSubmenu = false;
    
    // Load video info for conversion estimates
    if (currentVideoPath) {
      try {
        currentVideoInfo = await invoke<VideoInfo>('get_video_info', { videoPath: currentVideoPath });
      } catch (err) {
        console.error('Failed to get video info:', err);
      }
    }
  }
  
  function estimateConvertedSize(format: string): string {
    if (!currentVideoInfo) return "~? MB";
    
    let ratio = 1.0;
    switch (format) {
      case 'mp4': ratio = 0.85; break;
      case 'webm': ratio = 0.70; break;
      case 'mkv': ratio = 0.90; break;
    }
    
    const estimatedSize = currentVideoInfo.sizeMb * ratio;
    return `~${estimatedSize.toFixed(0)} MB`;
  }
  
  async function startConversion(format: string) {
    if (!currentVideoPath) return;
    
    showContextMenu = false;
    isConverting = true;
    conversionProgress = 0;
    conversionMessage = `Starting conversion to ${format.toUpperCase()}...`;
    
    try {
      const outputPath = await invoke<string>('convert_video', {
        videoPath: currentVideoPath,
        targetFormat: format
      });
      
      console.log('Video converted successfully:', outputPath);
    } catch (err) {
      console.error('Failed to convert video:', err);
      alert(`Conversion failed: ${err}`);
      isConverting = false;
      conversionProgress = 0;
      conversionMessage = '';
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
    
    // Try to restore watch progress
    if (currentVideoPath) {
      invoke<WatchProgress | null>('get_watch_progress', { videoPath: currentVideoPath })
        .then(progress => {
          if (progress && videoElement && progress.duration > 0) {
            const progressPercent = progress.current_time / progress.duration;
            if (progressPercent > 0.05 && progressPercent < 0.95) {
              videoElement.currentTime = progress.current_time;
            }
          }
        })
        .catch(err => console.error('Failed to load watch progress:', err));
    }
    
    // Set up interval to save progress every 5 seconds
    if (progressSaveInterval) {
      clearInterval(progressSaveInterval);
    }
    progressSaveInterval = setInterval(() => {
      if (videoElement && currentVideoPath && duration > 0) {
        saveWatchProgress();
      }
    }, 5000);
    
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
  
  function handleProgressHover(e: MouseEvent) {
    if (!videoElement || !previewVideo || !previewCanvas || isScrubbing) return;
    
    const progressBar = e.currentTarget as HTMLElement;
    const rect = progressBar.getBoundingClientRect();
    const percent = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    
    previewTime = percent * duration;
    previewPosition = e.clientX - rect.left;
    showPreview = true;
    
    // Throttle preview seeks with requestAnimationFrame
    if (previewRaf !== null) cancelAnimationFrame(previewRaf);
    previewRaf = requestAnimationFrame(() => {
      if (previewVideo) previewVideo.currentTime = previewTime;
      previewRaf = null;
    });
  }
  
  function handleProgressLeave() {
    showPreview = false;
    // Cancel any pending preview seek
    if (previewRaf !== null) {
      cancelAnimationFrame(previewRaf);
      previewRaf = null;
    }
  }
  
  function handleProgressKeydown(e: KeyboardEvent) {
    if (!videoElement || !duration) return;
    
    let handled = false;
    let newTime = currentTime;
    const step = duration * 0.01; // 1% of duration
    
    switch (e.key) {
      case 'ArrowLeft':
        newTime = Math.max(0, currentTime - step);
        handled = true;
        break;
      case 'ArrowRight':
        newTime = Math.min(duration, currentTime + step);
        handled = true;
        break;
      case 'Home':
        newTime = 0;
        handled = true;
        break;
      case 'End':
        newTime = duration;
        handled = true;
        break;
    }
    
    if (handled) {
      e.preventDefault();
      videoElement.currentTime = newTime;
      currentTime = newTime;
    }
  }
  
  function drawPreview() {
    if (!previewVideo || !previewCanvas) return;
    
    const ctx = previewCanvas.getContext('2d');
    if (!ctx) return;
    
    ctx.drawImage(previewVideo, 0, 0, previewCanvas.width, previewCanvas.height);
  }
  
  async function loadAudioDevices() {
    try {
      const devices = await navigator.mediaDevices.enumerateDevices();
      const outputDevices = devices.filter(device => device.kind === 'audiooutput');
      audioDevices = outputDevices;
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
        appSettings.updateAudioDevice(deviceId);
        showAudioMenu = false;
      }
    } catch (err) {
      console.error('Failed to change audio output:', err);
    }
  }
  
  function toggleVolumeMenu() {
    showVolumeMenu = !showVolumeMenu;
  }
  
  function toggleSubtitleMenu() {
    showSubtitleMenu = !showSubtitleMenu;
    if (showSubtitleMenu) {
      showModelSelector = false;
    }
  }
  
  function openAIFromUnifiedMenu() {
    showSubtitleMenu = false;
    setTimeout(() => {
      showModelSelector = true;
    }, 0);
  }
  
  async function startSubtitleGeneration(modelSize: string) {
    if (!currentVideoPath) {
      alert('No video loaded');
      return;
    }
    
    // Check if setup is complete
    const status = setupStatus || getSetupStatus();
    if (!status || !status.setup_completed || status.models_installed.length === 0) {
      showSetupDialog();
      return;
    }
    
    showModelSelector = false;
    isGeneratingSubtitles = true;
    generationProgress = 0;
    generationMessage = 'Starting subtitle generation...';
    
    try {
      // Get current subtitle language from store at call time
      const currentSettings = $appSettings;
      const subtitlePath = await invoke<string>('generate_subtitles', {
        videoPath: currentVideoPath,
        modelSize: modelSize,
        language: currentSettings.subtitleLanguage
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
  
  function getEstimatedTranscriptionTime(modelKey: string): string {
    if (!duration) return 'Unknown';
    
    const coefficients: Record<string, { min: number; max: number }> = {
      'tiny': { min: 0.15, max: 0.25 },
      'small': { min: 0.6, max: 0.8 },
      'large-v3-turbo': { min: 0.9, max: 1.2 }
    };
    
    const coef = coefficients[modelKey];
    if (!coef) return 'Unknown';
    
    const avgCoef = (coef.min + coef.max) / 2;
    const estimatedSeconds = duration * avgCoef;
    
    return formatEstimatedTime(estimatedSeconds);
  }
</script>

<main 
  class="player-container video-player"
  onmousemove={handleMainContainerMouseMove}
>
  {#if viewMode !== 'pip'}
    <button class="close-button" class:visible={showCloseButton} onclick={closeApp} title="Close (Esc)">
      <X size={16} />
    </button>
  {/if}
  
  <div class="video-container" class:cinematic={viewMode === 'cinematic'} class:fullscreen={viewMode === 'fullscreen'} class:pip={viewMode === 'pip'}>
    {#if viewMode === 'cinematic'}
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
      class:cinematic-video={viewMode === 'cinematic'}
      class:fullscreen-video={viewMode === 'fullscreen'}
      class:pip-video={viewMode === 'pip'}
      src={videoSrc}
      ontimeupdate={handleTimeUpdate}
      onloadedmetadata={handleLoadedMetadata}
      onclick={togglePlay}
      oncontextmenu={handleContextMenu}
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
          <Loader2 size={48} strokeWidth={2} class="spinner" />
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
    class:cinematic-controls={viewMode === 'cinematic'} 
    class:overlay-controls={viewMode === 'fullscreen'}
    class:pip-controls={viewMode === 'pip'}
  >
    <div 
      class="progress-bar" 
      class:scrubbing={isScrubbing}
      onmousedown={startScrubbing}
      onmousemove={handleProgressHover}
      onmouseleave={handleProgressLeave}
      onkeydown={handleProgressKeydown}
      role="slider" 
      aria-label="Video progress"
      aria-valuemin={0}
      aria-valuemax={duration}
      aria-valuenow={currentTime}
      aria-valuetext={formatTimeForScreenReader(currentTime)}
      tabindex="0"
    >
      {#if showPreview}
        <div class="preview-tooltip" style="left: {previewPosition}px">
          <canvas bind:this={previewCanvas} width="160" height="90" class="preview-canvas"></canvas>
          <div class="preview-time">{formatTime(previewTime)}</div>
        </div>
      {/if}
      <div class="progress-filled" style="width: {duration ? Math.min(100, Math.max(0, (currentTime / duration) * 100)) : 0}%">
        <div class="progress-handle"></div>
      </div>
    </div>

    <div class="controls-row">
      <div class="controls-left">
        <button class="control-button" onclick={goHome} title="Home">
          <Home size={20} />
        </button>
        <div class="time">
          {formatTime(currentTime)} / {formatTime(duration)}
        </div>
      </div>

      <div class="controls-center">
        <button class="control-button" onclick={togglePlay}>
          {#if isPlaying}
            <Pause size={24} fill="currentColor" />
          {:else}
            <Play size={24} fill="currentColor" />
          {/if}
        </button>
      </div>

      <div class="controls-right">
        <div class="volume-control">
          <button class="control-button" onclick={toggleVolumeMenu} title="Volume">
            {#if isMuted}
              <VolumeX size={20} />
            {:else if volume < 0.5}
              <Volume1 size={20} />
            {:else}
              <Volume2 size={20} />
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
                oninput={(e) => { 
                  if (videoElement) { 
                    const newVolume = (e.target as HTMLInputElement).valueAsNumber;
                    videoElement.volume = newVolume; 
                    appSettings.updateVolume(newVolume);
                    if (isMuted) { 
                      isMuted = false; 
                      videoElement.muted = false; 
                      appSettings.updateMuted(false);
                    } 
                  } 
                }}
              />
              <button class="mute-toggle" onclick={toggleMute} class:muted={isMuted}>
                {#if isMuted}
                  <VolumeX size={16} />
                {:else}
                  <Volume2 size={16} />
                {/if}
              </button>
            </div>
          {/if}
        </div>
        
        <!-- Consolidated Subtitles Menu -->
        <div class="subtitle-control">
          <button 
            class="control-button" 
            class:subtitle-active={subtitleSrc && subtitlesEnabled}
            class:generating={isGeneratingSubtitles}
            title="Subtitles"
            onclick={() => showSubtitleMenu = !showSubtitleMenu}
            disabled={isGeneratingSubtitles}
          >
            {#if subtitleSrc && subtitlesEnabled}
              <Captions size={20} />
            {:else}
              <CaptionsOff size={20} />
            {/if}
          </button>

          {#if showSubtitleMenu && !isGeneratingSubtitles}
            <div class="subtitle-menu">
              <div class="model-header">Subtitles</div>
              <button class="model-option" onclick={() => { showSubtitleMenu = false; openSubtitleDialog(); }}>
                <span class="model-name">Import subtitle from device</span>
                <span class="model-desc">Open .srt, .vtt or compatible file</span>
              </button>
              <button class="model-option" onclick={openAIFromUnifiedMenu}>
                <span class="model-name">Generate with AI</span>
                <span class="model-desc">Auto-generate using Whisper AI</span>
              </button>
              {#if subtitleFileName}
                <div class="subtitle-menu-divider"></div>
                <button class="model-option" onclick={toggleSubtitles}>
                  <span class="model-name">{subtitleFileName}</span>
                  <span class="model-desc">{subtitlesEnabled ? 'Hide' : 'Show'} subtitles</span>
                </button>
              {/if}
            </div>
          {/if}
        </div>
        
        <!-- Model selector anchored to unified subtitle control -->
        {#if showModelSelector && !isGeneratingSubtitles}
          <div class="model-selector">
            <div class="model-header">Select AI Model</div>
            {#if setupStatus && setupStatus.models_installed.length > 0}
              {#each setupStatus.models_installed as model}
                {#if model === 'tiny'}
                  <button class="model-option" onclick={() => startSubtitleGeneration('tiny')}>
                    <span class="model-name">Tiny</span>
                    <span class="model-desc">{getEstimatedTranscriptionTime('tiny')} • Fastest</span>
                  </button>
                {:else if model === 'small'}
                  <button class="model-option" onclick={() => startSubtitleGeneration('small')}>
                    <span class="model-name">Small</span>
                    <span class="model-desc">{getEstimatedTranscriptionTime('small')} • Balanced</span>
                  </button>
                {:else if model === 'large-v3-turbo'}
                  <button class="model-option" onclick={() => startSubtitleGeneration('large-v3-turbo')}>
                    <span class="model-name">Large V3 Turbo</span>
                    <span class="model-desc">{getEstimatedTranscriptionTime('large-v3-turbo')} • Most Accurate</span>
                  </button>
                {/if}
              {/each}
            {:else}
              <div class="no-models-message">
                No AI models installed. Open Settings to download models.
              </div>
            {/if}
          </div>
        {/if}
        
        <button class="control-button" onclick={toggleViewMode} title="Toggle view mode (F)">
          <Maximize size={20} />
        </button>
      </div>
    </div>
  </div>
  </div>
  
  <!-- Custom Context Menu -->
  {#if showContextMenu}
    <div 
      class="context-menu" 
      style="left: {contextMenuPosition.x}px; top: {contextMenuPosition.y}px;"
    >
      <button class="context-menu-item" onclick={() => { togglePlay(); showContextMenu = false; }}>
        {#if isPlaying}
          <Pause size={16} />
          <span>Pause</span>
        {:else}
          <Play size={16} />
          <span>Play</span>
        {/if}
      </button>
      <button class="context-menu-item" onclick={() => { toggleMute(); showContextMenu = false; }}>
        {#if isMuted}
          <Volume2 size={16} />
          <span>Unmute</span>
        {:else}
          <VolumeX size={16} />
          <span>Mute</span>
        {/if}
      </button>
      <div class="context-menu-divider"></div>
      <button class="context-menu-item" onclick={() => { toggleViewMode(); showContextMenu = false; }}>
        <Maximize size={16} />
        <span>
          {#if viewMode === 'cinematic'}
            Fullscreen Mode
          {:else if viewMode === 'fullscreen'}
            PiP Mode
          {:else}
            Cinematic Mode
          {/if}
        </span>
      </button>
      {#if subtitleSrc}
        <button class="context-menu-item" onclick={() => { toggleSubtitles(); showContextMenu = false; }}>
          {#if subtitlesEnabled}
            <CaptionsOff size={16} />
            <span>Hide Subtitles</span>
          {:else}
            <Captions size={16} />
            <span>Show Subtitles</span>
          {/if}
        </button>
      {/if}
      <div class="context-menu-divider"></div>
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <div 
        class="context-menu-item-wrapper" 
        onmouseenter={() => showConvertSubmenu = true}
        onmouseleave={() => showConvertSubmenu = false}
        role="menuitem"
        tabindex="0"
      >
        <div class="context-menu-item">
          <Settings size={16} />
          <span>Convert Video To</span>
          <span style="margin-left: auto; font-size: 0.75rem;">›</span>
        </div>
        
        {#if showConvertSubmenu}
          <div class="context-submenu">
            {#if currentVideoInfo && normalizedFormat !== 'MP4'}
              <button class="context-menu-item" onclick={() => startConversion('mp4')}>
                <span>MP4 {estimateConvertedSize('mp4')}</span>
              </button>
            {/if}
            {#if currentVideoInfo && normalizedFormat !== 'WEBM'}
              <button class="context-menu-item" onclick={() => startConversion('webm')}>
                <span>WebM {estimateConvertedSize('webm')}</span>
              </button>
            {/if}
            {#if currentVideoInfo && normalizedFormat !== 'MKV'}
              <button class="context-menu-item" onclick={() => startConversion('mkv')}>
                <span>MKV {estimateConvertedSize('mkv')}</span>
              </button>
            {/if}
          </div>
        {/if}
      </div>
      <div class="context-menu-divider"></div>
      <button class="context-menu-item" onclick={() => { goHome(); showContextMenu = false; }}>
        <Home size={16} />
        <span>Back to Home</span>
      </button>
    </div>
  {/if}
  
  <!-- Video Conversion Progress Overlay -->
  {#if isConverting}
    <div class="generation-overlay">
      <div class="generation-modal">
        <div class="generation-icon">
          <Loader2 size={48} strokeWidth={2} class="spinner" />
        </div>
        <h3>Converting Video</h3>
        <div class="progress-container">
          <div class="progress-track">
            <div class="progress-fill" style="width: {conversionProgress}%"></div>
          </div>
          <div class="progress-percentage">{Math.round(conversionProgress)}%</div>
        </div>
        <p class="generation-message">{conversionMessage}</p>
      </div>
    </div>
  {/if}
</main>

<style>
  .player-container.video-player {
    background: rgba(0, 0, 0, 0.85);
    backdrop-filter: blur(40px);
    -webkit-backdrop-filter: blur(40px);
  }

  .player-container:has(.video-container.fullscreen) {
    background: rgba(0, 0, 0, 1);
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }

  .player-container:has(.video-container.pip) {
    background: #000 !important;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
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
    flex: 1;
  }

  .video-container.fullscreen {
    position: absolute;
    inset: 0;
  }

  .video-container.pip {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
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

  .pip-video {
    width: 100% !important;
    height: 100% !important;
    max-width: 100% !important;
    max-height: 100% !important;
    object-fit: contain !important;
    position: absolute !important;
    top: 0 !important;
    left: 0 !important;
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

  .video-container.pip ~ .controls-zone {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 10;
    min-height: 60px;
    pointer-events: all;
  }

  .controls {
    transition: opacity 0.25s ease;
  }

  .cinematic-controls {
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

  .pip-controls {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    background: linear-gradient(to top, rgba(0, 0, 0, 0.9) 0%, rgba(0, 0, 0, 0.7) 70%, transparent 100%);
    padding: 0.5rem;
    opacity: 0;
    pointer-events: none;
  }

  .pip-controls.visible {
    opacity: 1;
    pointer-events: all;
  }

  /* Hide some controls in PiP mode */
  .pip-controls .controls-left .control-button:not(:first-child),
  .pip-controls .controls-right > *:not(.control-button:last-child) {
    display: none;
  }

  /* Hide close button in PiP mode (handled via conditional rendering) */
  /* Hide subtitle menus and volume menus in PiP mode */
  .pip-controls .subtitle-menu,
  .pip-controls .model-selector,
  .pip-controls .volume-menu {
    display: none !important;
  }

  .pip-controls .time {
    font-size: 0.65rem;
  }

  .pip-controls .control-button {
    padding: 0.25rem;
  }

  .pip-controls .progress-bar {
    height: 2px;
    margin-bottom: 0.5rem;
  }

  .pip-controls .progress-bar:hover {
    height: 3px;
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
    writing-mode: bt-lr;
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

  /* Subtitle styling */
  :global(video::cue) {
    background-color: rgba(0, 0, 0, 0.8) !important;
    color: #ffffff !important;
    font-size: 1.5em !important;
    font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", system-ui, sans-serif !important;
    text-shadow: 2px 2px 4px rgba(0, 0, 0, 0.9) !important;
    line-height: 1.4 !important;
    padding: 0.2em 0.5em !important;
  }
  
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
  
  :global(video::cue-region) {
    position: absolute !important;
    bottom: 86vh !important;
    left: 0 !important;
    right: 0 !important;
  }
  
  .subtitle-control {
    position: relative;
  }
  
  .subtitle-menu {
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
    min-width: 260px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    z-index: 100;
  }
  
  .subtitle-menu-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.05);
    margin: 0.5rem 0;
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
  
  .no-models-message {
    padding: 1rem;
    font-size: 0.875rem;
    color: rgba(255, 255, 255, 0.7);
    text-align: center;
    line-height: 1.5;
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
  
  :global(.spinner) {
    animation: spin 2s linear infinite;
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
  
  /* Context Menu */
  .context-menu {
    position: fixed;
    background: rgba(0, 0, 0, 0.95);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 0.5rem 0;
    min-width: 200px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    z-index: 1000;
  }
  
  .context-menu-item {
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
    align-items: center;
    gap: 0.75rem;
  }
  
  .context-menu-item:hover {
    background: rgba(255, 255, 255, 0.1);
  }
  
  .context-menu-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.1);
    margin: 0.5rem 0;
  }
  
  .context-menu-item-wrapper {
    position: relative;
  }
  
  .context-submenu {
    position: absolute;
    left: 100%;
    top: 0;
    margin-left: 0.5rem;
    background: rgba(0, 0, 0, 0.95);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 0.5rem 0;
    min-width: 180px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    z-index: 1001;
  }
</style>
