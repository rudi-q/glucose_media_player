<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount } from "svelte";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { 
    X, 
    Settings, 
    FolderOpen, 
    Play, 
    Pause, 
    Home, 
    VolumeX, 
    Volume1, 
    Volume2, 
    Captions, 
    CaptionsOff, 
    Maximize, 
    Check,
    Loader2
  } from "lucide-svelte";

  let videoElement = $state<HTMLVideoElement>();
  let backgroundVideo = $state<HTMLVideoElement>();
  let previewVideo = $state<HTMLVideoElement>();
  let previewCanvas = $state<HTMLCanvasElement>();
  let videoSrc = $state<string | null>(null);
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);
  let volume = $state(1);
  let isMuted = $state(false);
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
    duration?: number;
  }
  
  interface WatchProgress {
    path: string;
    current_time: number;
    duration: number;
    last_watched: number;
  }
  
  let recentVideos = $state<VideoFile[]>([]);
  let loadingRecent = $state(true);
  let thumbnailCache = $state<Map<string, string>>(new Map());
  let watchProgressMap = $state<Map<string, WatchProgress>>(new Map());
  let progressSaveInterval: number;
  
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
  let showSubtitleMenu = $state(false);
  let currentVideoPath = $state<string | null>(null);
  let subtitleFileName = $state<string | null>(null);
  let showContextMenu = $state(false);
  let contextMenuPosition = $state({ x: 0, y: 0 });
  let showConvertSubmenu = $state(false);
  let showGalleryContextMenu = $state(false);
  let galleryContextMenuPosition = $state({ x: 0, y: 0 });
  let isConverting = $state(false);
  let conversionProgress = $state(0);
  let conversionMessage = $state("");
  
  interface VideoInfo {
    format: string;
    size_mb: number;
  }
  
  let currentVideoInfo = $state<VideoInfo | null>(null);
  
  // Setup state
  interface SetupStatus {
    ffmpeg_installed: boolean;
    models_installed: string[];
    setup_completed: boolean;
  }
  
  let showSetupDialog = $state(false);
  let setupStatus = $state<SetupStatus | null>(null);
  let selectedModelForSetup = $state<string>("tiny"); // "tiny" or "small"
  let isDownloading = $state(false);
  let downloadProgress = $state(0);
  let downloadMessage = $state("");
  let showSettings = $state(false);

onMount(() => {
    let disposed = false;
    const unsubs: UnlistenFn[] = [];

    // Register Tauri event listeners and store unlisten fns
    (async () => {
      const results = await Promise.allSettled([
        listen<string>("open-file", (event) => {
          loadVideo(event.payload);
          shouldLoadGallery = false; // Skip gallery when file is opened via association
          // Mark file as processed
          invoke("mark_file_processed").catch(console.error);
        }),
        listen<string[]>("tauri://drag-drop", (event) => {
          if (event.payload && event.payload.length > 0) {
            loadVideo(event.payload[0]);
          }
        }),
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
        // Listen for download progress
        listen<{downloaded: number, total: number, percentage: number, message: string}>("download-progress", (event) => {
          downloadProgress = event.payload.percentage;
          downloadMessage = event.payload.message;
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

    // Check setup status on first launch
    checkSetupStatus();

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
          
          // Load watch progress for all videos
          const progressData = await invoke<Record<string, WatchProgress>>("get_all_watch_progress");
          watchProgressMap = new Map(Object.entries(progressData));
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
      disposed = true;
      // Clear progress save interval
      if (progressSaveInterval) {
        clearInterval(progressSaveInterval);
      }
      // Save progress one last time before unmount
      if (videoElement && currentVideoPath && duration > 0) {
        saveWatchProgress();
      }
      // Unregister Tauri event listeners
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
    if (e.key === "Backspace" && videoSrc) {
      e.preventDefault();
      goHome();
      return;
    }

    // Video playback controls
    if (videoSrc) {
      // Number keys 0-9 for scrubbing to specific percentages
      if (e.key >= '0' && e.key <= '9') {
        e.preventDefault();
        const percentage = parseInt(e.key) * 0.1; // 0 = 0%, 1 = 10%, 2 = 20%, etc.
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
          scrollSelectedVideoIntoView();
          break;
        case "ArrowRight":
          e.preventDefault();
          selectedVideoIndex = Math.min(recentVideos.length - 1, selectedVideoIndex + 1);
          scrollSelectedVideoIntoView();
          break;
        case "ArrowUp":
          e.preventDefault();
          // Move up one row (assuming 4 columns grid)
          selectedVideoIndex = Math.max(0, selectedVideoIndex - 4);
          scrollSelectedVideoIntoView();
          break;
        case "ArrowDown":
          e.preventDefault();
          // Move down one row
          selectedVideoIndex = Math.min(recentVideos.length - 1, selectedVideoIndex + 4);
          scrollSelectedVideoIntoView();
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

  function scrollSelectedVideoIntoView() {
    // Wait a tick for the DOM to update
    setTimeout(() => {
      const selectedCard = document.querySelector('.video-card.selected');
      if (selectedCard) {
        selectedCard.scrollIntoView({
          behavior: 'smooth',
          block: 'center',
          inline: 'nearest'
        });
      }
    }, 0);
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

  async function saveWatchProgress() {
    if (!currentVideoPath || !videoElement || duration <= 0) return;
    
    try {
      await invoke('save_watch_progress', {
        videoPath: currentVideoPath,
        currentTime: videoElement.currentTime,
        duration: duration
      });
    } catch (err) {
      console.error('Failed to save watch progress:', err);
    }
  }

  async function loadVideo(path: string) {
    // Save progress of previous video before loading new one
    if (currentVideoPath && videoElement && duration > 0) {
      await saveWatchProgress();
    }
    
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
      // Record subtitle file name for UI display
      subtitleFileName = (path.split(/[/\\\\]/).pop() || 'Subtitles');
      
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
        subtitleFileName = null;
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
        subtitleFileName = null;
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

  async function goHome() {
    // Save progress before going home
    if (currentVideoPath && videoElement && duration > 0) {
      await saveWatchProgress();
    }
    
    // Clear progress save interval
    if (progressSaveInterval) {
      clearInterval(progressSaveInterval);
    }
    
    videoSrc = null;
    if (videoElement) {
      videoElement.pause();
    }
    if (backgroundVideo) {
      backgroundVideo.pause();
    }
    isPlaying = false;
    currentVideoPath = null;
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

  function scrubToPercentage(percentage: number) {
    if (!videoElement || !duration) return;
    const targetTime = duration * percentage;
    
    // Use fastSeek for instant frame updates if available
    if ('fastSeek' in videoElement && typeof (videoElement as any).fastSeek === 'function') {
      (videoElement as any).fastSeek(targetTime);
    } else if (videoElement) {
      videoElement.currentTime = targetTime;
    }
  }

  function toggleMute() {
    if (!videoElement) return;
    isMuted = !isMuted;
    videoElement.muted = isMuted;
  }

  function adjustVolume(delta: number) {
    if (!videoElement) return;
    const newVolume = Math.max(0, Math.min(1, volume + delta));
    volume = newVolume;
    videoElement.volume = newVolume;
    // Unmute if currently muted and adjusting volume
    if (isMuted) {
      isMuted = false;
      videoElement.muted = false;
    }
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
    if (showVolumeMenu && !target.closest('.volume-control')) {
      showVolumeMenu = false;
    }
    // Close unified subtitle menu only when clicking outside its container
    if (showSubtitleMenu && !target.closest('.subtitle-control')) {
      showSubtitleMenu = false;
    }
    // Keep model selector open when interaction happens on either AI button, unified subtitle control, or inside selector
    if (showModelSelector && !(target.closest('.ai-subtitle-generator') || target.closest('.subtitle-control') || target.closest('.model-selector'))) {
      showModelSelector = false;
    }
    // Close context menu when clicking outside
    if (showContextMenu && !target.closest('.context-menu')) {
      showContextMenu = false;
    }
    // Close gallery context menu when clicking outside
    if (showGalleryContextMenu && !target.closest('.context-menu')) {
      showGalleryContextMenu = false;
    }
  }
  
  function handleGalleryContextMenu(e: MouseEvent) {
    // Only show if clicking on gallery background, not on cards
    const target = e.target as HTMLElement;
    if (target.closest('.video-card')) return;
    
    e.preventDefault();
    galleryContextMenuPosition = { x: e.clientX, y: e.clientY };
    showGalleryContextMenu = true;
  }

  async function handleContextMenu(e: MouseEvent) {
    if (!videoSrc) return; // Only show context menu when video is playing
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
    
    const estimatedSize = currentVideoInfo.size_mb * ratio;
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
            // Only restore if video was watched for more than 5% and less than 95%
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

  function formatTime(seconds: number): string {
    const h = Math.floor(seconds / 3600);
    const m = Math.floor((seconds % 3600) / 60);
    const s = Math.floor(seconds % 60);
    
    if (h > 0) {
      return `${h}:${m.toString().padStart(2, '0')}:${s.toString().padStart(2, '0')}`;
    }
    return `${m}:${s.toString().padStart(2, '0')}`;
  }
  
  function formatDuration(seconds?: number): string {
    if (!seconds) return '';
    return formatTime(seconds);
  }
  
  function getRemainingTime(videoPath: string, videoDuration?: number): string {
    if (!videoDuration) return '';
    
    const progress = watchProgressMap.get(videoPath);
    if (!progress || !progress.duration) {
      // Not started yet, show full duration as "remaining"
      const mins = Math.ceil(videoDuration / 60);
      return `${mins} min${mins !== 1 ? 's' : ''} remaining`;
    }
    
    const remaining = videoDuration - progress.current_time;
    if (remaining <= 0) return 'Finished';
    
    const mins = Math.ceil(remaining / 60);
    return `${mins} min${mins !== 1 ? 's' : ''} remaining`;
  }
  
  function formatEstimatedTime(seconds: number): string {
    if (seconds < 60) {
      return `~${Math.round(seconds)}s`;
    } else if (seconds < 3600) {
      const mins = Math.round(seconds / 60);
      return `~${mins}m`;
    } else {
      const hours = Math.floor(seconds / 3600);
      const mins = Math.round((seconds % 3600) / 60);
      return mins > 0 ? `~${hours}h ${mins}m` : `~${hours}h`;
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
    console.log('toggleModelSelector called, current value:', showModelSelector);
    console.log('setupStatus:', setupStatus);
    showModelSelector = !showModelSelector;
    console.log('showModelSelector now:', showModelSelector);
  }
  
  function toggleSubtitleMenu() {
    showSubtitleMenu = !showSubtitleMenu;
    if (showSubtitleMenu) {
      showModelSelector = false;
    }
  }
  
  function openAISubtitleSelector() {
    console.log('Opening AI subtitle selector');
    console.log('Setup status:', setupStatus);
    showSubtitleMenu = false;
    showModelSelector = true;
    console.log('showModelSelector set to:', showModelSelector);
  }
  
  function openAIFromUnifiedMenu() {
    // Close the unified menu first, then open the model selector on next tick
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
    const status = await invoke<SetupStatus>('get_setup_status');
    if (!status.setup_completed || status.models_installed.length === 0) {
      // Show setup dialog
      showSetupDialog = true;
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
  
  async function checkSetupStatus() {
    try {
      const status = await invoke<SetupStatus>('get_setup_status');
      setupStatus = status;
      
      // Show setup dialog on first launch if not completed
      if (!status.setup_completed && status.models_installed.length === 0) {
        // Small delay to not interrupt app startup
        setTimeout(() => {
          showSetupDialog = true;
        }, 1500);
      }
    } catch (err) {
      console.error('Failed to check setup status:', err);
    }
  }
  
  function toggleSetupDialog() {
    showSetupDialog = !showSetupDialog;
  }
  
  async function runSetup() {
    if (!setupStatus) return;
    
    // Check if the selected model is already installed
    const isModelInstalled = setupStatus.models_installed.includes(selectedModelForSetup);
    
    if (isModelInstalled) {
      // Model already installed, just mark setup as complete
      try {
        await invoke('mark_setup_completed');
        await checkSetupStatus();
        showSetupDialog = false;
      } catch (err) {
        console.error('Failed to mark setup complete:', err);
      }
      return;
    }
    
    isDownloading = true;
    downloadProgress = 0;
    downloadMessage = "Starting download...";
    
    try {
      // Download the selected model
      await invoke('download_whisper_model', {
        modelSize: selectedModelForSetup
      });
      
      // Mark setup as completed
      await invoke('mark_setup_completed');
      
      // Refresh setup status
      await checkSetupStatus();
      
      isDownloading = false;
      downloadProgress = 100;
      downloadMessage = "Download complete!";
      
      setTimeout(() => {
        showSetupDialog = false;
        downloadProgress = 0;
        downloadMessage = "";
      }, 2000);
    } catch (err) {
      console.error('Setup failed:', err);
      alert(`Setup failed: ${err}`);
      isDownloading = false;
      downloadProgress = 0;
      downloadMessage = "";
    }
  }
  
  function skipSetup() {
    showSetupDialog = false;
    // Mark setup as "completed" so we don't show the dialog again
    invoke('mark_setup_completed').catch(console.error);
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
    <X size={16} />
  </button>
  
  {#if !videoSrc}
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="empty-state" class:dragging={isDragging} oncontextmenu={handleGalleryContextMenu}>
      <div class="library-container">
        <div class="library-header">
          <img src="/logo-dark.svg" alt="glucose" class="logo" />
          <div class="header-buttons">
            <button class="open-button" onclick={openFileDialog}>
              <FolderOpen size={18} />
              Open Video
            </button>
            <button class="open-button" onclick={() => showSettings = true}>
              <Settings size={18} />
              Settings
            </button>
          </div>
        </div>
        
        {#if loadingRecent}
          <div class="loading">Scanning for videos...</div>
        {:else if recentVideos.length === 0}
          <div class="empty-content">
            <Play size={64} strokeWidth={1.5} />
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
                      <Play size={48} strokeWidth={1.5} />
                    {:then thumbnail}
                      {#if thumbnail}
                        <img src={thumbnail} alt={video.name} class="thumbnail-img" />
                      {:else}
                        <Play size={48} strokeWidth={1.5} />
                      {/if}
                    {/await}
                    <div class="play-overlay">
                      <Play size={32} fill="white" stroke="none" />
                    </div>
                    {#if watchProgressMap.has(video.path)}
                      {@const progress = watchProgressMap.get(video.path)}
                      {@const progressPercent = progress && progress.duration > 0 ? (progress.current_time / progress.duration) * 100 : 0}
                      {#if progressPercent > 0 && progressPercent < 100}
                        <div class="video-progress-bar">
                          <div class="video-progress-fill" style="width: {progressPercent}%"></div>
                        </div>
                      {/if}
                    {/if}
                  </div>
                  <div class="video-info">
                    <div class="video-name" title={video.name}>{video.name}</div>
                    <div class="video-meta">
                      {#if video.duration}
                        <span class="video-duration">{formatDuration(video.duration)}</span>
                        <span class="video-separator">•</span>
                        <span class="video-remaining">{getRemainingTime(video.path, video.duration)}</span>
                      {:else}
                        <span>{(video.size / (1024 * 1024)).toFixed(1)} MB</span>
                      {/if}
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
                  oninput={(e) => { if (videoElement) { videoElement.volume = (e.target as HTMLInputElement).valueAsNumber; if (isMuted) { isMuted = false; videoElement.muted = false; } } }}
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
          
          <!-- Consolidated Subtitles Menu (additive) -->
          <div 
            class="subtitle-control"
          >
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
          
          <button class="control-button" onclick={toggleCinematicMode} title="Toggle view mode (F)">
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
        <button class="context-menu-item" onclick={() => { toggleCinematicMode(); showContextMenu = false; }}>
          <Maximize size={16} />
          <span>{isCinematicMode ? 'Fullscreen Mode' : 'Cinematic Mode'}</span>
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
              {#if currentVideoInfo && currentVideoInfo.format !== 'MP4'}
                <button class="context-menu-item" onclick={() => startConversion('mp4')}>
                  <span>MP4 {estimateConvertedSize('mp4')}</span>
                </button>
              {/if}
              {#if currentVideoInfo && currentVideoInfo.format !== 'WEBM'}
                <button class="context-menu-item" onclick={() => startConversion('webm')}>
                  <span>WebM {estimateConvertedSize('webm')}</span>
                </button>
              {/if}
              {#if currentVideoInfo && currentVideoInfo.format !== 'MKV'}
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
    
    <!-- Gallery Context Menu -->
    {#if showGalleryContextMenu}
      <div 
        class="context-menu" 
        style="left: {galleryContextMenuPosition.x}px; top: {galleryContextMenuPosition.y}px;"
      >
        <button class="context-menu-item" onclick={() => { openFileDialog(); showGalleryContextMenu = false; }}>
          <FolderOpen size={16} />
          <span>Open Video</span>
        </button>
        <button class="context-menu-item" onclick={() => { showSettings = true; showGalleryContextMenu = false; }}>
          <Settings size={16} />
          <span>Settings</span>
        </button>
      </div>
    {/if}
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
  
  <!-- Settings Overlay -->
  {#if showSettings}
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="settings-overlay" onclick={(e) => { if (e.target === e.currentTarget) showSettings = false; }}>
      <div class="settings-modal">
        <div class="settings-header">
          <h2>Settings</h2>
          <button class="settings-close" onclick={() => showSettings = false} title="Close">
            <X size={20} />
          </button>
        </div>
        
        <div class="settings-content">
          <!-- AI Settings Section -->
          <div class="settings-section">
            <h3>AI Settings</h3>
            
            {#if setupStatus}
              <div class="settings-group">
                <div class="settings-item">
                  <div class="settings-item-label">
                    <div class="settings-item-title">AI Subtitle Generation</div>
                    <div class="settings-item-desc">
                      Automatically generate subtitles from video audio using Whisper AI
                    </div>
                  </div>
                  <div class="settings-item-status">
                    {#if setupStatus.models_installed.length > 0}
                      <span class="status-badge active">Enabled</span>
                    {:else}
                      <span class="status-badge inactive">Not Set Up</span>
                    {/if}
                  </div>
                </div>
                
                <div class="settings-item">
                  <div class="settings-item-label">
                    <div class="settings-item-title">FFmpeg</div>
                    <div class="settings-item-desc">Required for audio extraction from videos</div>
                  </div>
                  <div class="settings-item-status">
                    {#if setupStatus.ffmpeg_installed}
                      <span class="status-badge active">✓ Installed</span>
                    {:else}
                      <span class="status-badge inactive">✗ Not Installed</span>
                    {/if}
                  </div>
                </div>
                
                <!-- svelte-ignore a11y_click_events_have_key_events -->
                <!-- svelte-ignore a11y_no_static_element_interactions -->
                <div class="settings-item" onclick={() => { showSettings = false; showSetupDialog = true; }} style="cursor: pointer;" role="button" tabindex="0">
                  <div class="settings-item-label">
                    <div class="settings-item-title">AI Models</div>
                    <div class="settings-item-desc">
                      {#if setupStatus.models_installed.length > 0}
                        {@const totalModels = 3}
                        {@const installedCount = setupStatus.models_installed.length}
                        {@const availableCount = totalModels - installedCount}
                        {@const recommendedModel = setupStatus.models_installed.includes('tiny') ? 'Tiny' : setupStatus.models_installed.includes('small') ? 'Small' : setupStatus.models_installed.includes('large-v3-turbo') ? 'Large V3 Turbo' : setupStatus.models_installed[0]}
                        {recommendedModel} model recommended{#if installedCount > 1}, {installedCount - 1} more installed{/if}{#if availableCount > 0}, {availableCount} more available{/if}
                      {:else}
                        No models installed, 3 available
                      {/if}
                    </div>
                  </div>
                  <div class="settings-item-status">
                    {#if setupStatus.models_installed.length > 0}
                      <span class="status-badge active">Installed {setupStatus.models_installed.length} {setupStatus.models_installed.length === 1 ? 'model' : 'models'}</span>
                    {:else}
                      <span class="status-badge inactive">Not Set Up</span>
                    {/if}
                  </div>
                </div>
              </div>
            {/if}
          </div>
        </div>
      </div>
    </div>
  {/if}
  
  <!-- First-Run Setup Dialog -->
  {#if showSetupDialog}
    <div class="setup-overlay">
      <div class="setup-modal">
        <h2>Enable AI Subtitle Generation?</h2>
        <p class="setup-description">
          Automatically generate subtitles from video audio using AI.
          This feature requires downloading additional components.
        </p>
        
        {#if setupStatus}
          <div class="setup-checklist">
            <h3>Requirements</h3>
            
            <!-- FFmpeg -->
            <div class="setup-item">
              <div class="setup-item-header">
                <div class="checkbox" class:checked={setupStatus.ffmpeg_installed}>
                  {#if setupStatus.ffmpeg_installed}
                    <Check size={16} strokeWidth={3} />
                  {/if}
                </div>
                <div class="setup-item-info">
                  <div class="setup-item-title">FFmpeg (Required)</div>
                  <div class="setup-item-desc">
                    {#if setupStatus.ffmpeg_installed}
                      ✓ Already installed
                    {:else}
                      ❌ Not installed - Please install FFmpeg manually
                    {/if}
                  </div>
                </div>
              </div>
            </div>
            
            <!-- AI Model Selection -->
            <div class="setup-item">
              <div class="setup-item-header">
                <div class="setup-item-info full-width">
                  <div class="setup-item-title">AI Model (Choose one)</div>
                  <div class="model-choices">
                    <label class="model-radio" class:installed={setupStatus.models_installed.includes('tiny')}>
                      <input 
                        type="radio" 
                        name="model" 
                        value="tiny" 
                        bind:group={selectedModelForSetup}
                        disabled={isDownloading}
                      />
                      <div class="radio-content">
                        <div class="radio-header">
                          <span class="radio-title">Lite Model</span>
                          {#if setupStatus.models_installed.includes('tiny')}
                            <span class="installed-badge">✓ Installed</span>
                          {/if}
                        </div>
                        <span class="radio-desc">75 MB • Fastest • Good accuracy</span>
                      </div>
                    </label>
                    
                    <label class="model-radio" class:installed={setupStatus.models_installed.includes('small')}>
                      <input 
                        type="radio" 
                        name="model" 
                        value="small" 
                        bind:group={selectedModelForSetup}
                        disabled={isDownloading}
                      />
                      <div class="radio-content">
                        <div class="radio-header">
                          <span class="radio-title">Optimal Model</span>
                          {#if setupStatus.models_installed.includes('small')}
                            <span class="installed-badge">✓ Installed</span>
                          {/if}
                        </div>
                        <span class="radio-desc">466 MB • Balanced • Very good accuracy</span>
                      </div>
                    </label>
                    
                    <label class="model-radio" class:installed={setupStatus.models_installed.includes('large-v3-turbo')}>
                      <input 
                        type="radio" 
                        name="model" 
                        value="large-v3-turbo" 
                        bind:group={selectedModelForSetup}
                        disabled={isDownloading}
                      />
                      <div class="radio-content">
                        <div class="radio-header">
                          <span class="radio-title">Most Optimal Model</span>
                          {#if setupStatus.models_installed.includes('large-v3-turbo')}
                            <span class="installed-badge">✓ Installed</span>
                          {/if}
                        </div>
                        <span class="radio-desc">574 MB • Multilingual • Best accuracy</span>
                      </div>
                    </label>
                  </div>
                </div>
              </div>
            </div>
          </div>
          
          {#if isDownloading}
            <div class="download-progress">
              <div class="progress-track">
                <div class="progress-fill" style="width: {downloadProgress}%"></div>
              </div>
              <div class="download-status">
                <span>{downloadMessage}</span>
                <span>{Math.round(downloadProgress)}%</span>
              </div>
            </div>
          {/if}
          
          <div class="setup-actions">
            <button 
              class="setup-button secondary" 
              onclick={skipSetup}
              disabled={isDownloading}
            >
              Maybe Later
            </button>
            <button 
              class="setup-button primary" 
              onclick={runSetup}
              disabled={isDownloading || !setupStatus.ffmpeg_installed}
            >
              {#if isDownloading}
                Downloading...
              {:else if setupStatus.models_installed.includes(selectedModelForSetup)}
                Enable
              {:else}
                Download & Enable
              {/if}
            </button>
          </div>
          
          {#if !setupStatus.ffmpeg_installed}
            <div class="setup-warning">
              ⚠️ FFmpeg must be installed first. 
              <a href="https://ffmpeg.org/download.html" target="_blank" rel="noopener">Download FFmpeg</a>
            </div>
          {/if}
        {/if}
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
    border-radius: 6px;
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
    background: rgba(255, 255, 255, 0.15);
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
    padding: 3rem 5rem 3rem 2rem;
  }

  .library-header {
    position: sticky;
    top: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 3rem;
    padding: 1.5rem 0;
    background: transparent;
    z-index: 10;
    margin-top: -1.5rem;
    margin-left: -5rem;
    margin-right: -5rem;
    padding-left: 5rem;
    padding-right: 5rem;
  }

  .library-header .logo {
    height: 48px;
    width: auto;
    opacity: 0.95;
  }

  .header-buttons {
    display: flex;
    align-items: center;
    gap: 0.75rem;
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
  
  .video-progress-bar {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    height: 4px;
    background: rgba(0, 0, 0, 0.5);
    z-index: 2;
  }
  
  .video-progress-fill {
    height: 100%;
    background: rgba(255, 255, 255, 0.9);
    transition: width 0.3s ease;
    box-shadow: 0 0 8px rgba(255, 255, 255, 0.5);
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
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }
  
  .video-duration {
    font-variant-numeric: tabular-nums;
  }
  
  .video-separator {
    opacity: 0.5;
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
  
  
  /* Unified subtitles control */
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
  
  /* Settings Overlay Styles */
  .settings-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1500;
    animation: fadeIn 0.3s ease;
  }
  
  .settings-modal {
    background: rgba(20, 20, 20, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    width: 90%;
    max-width: 700px;
    max-height: 80vh;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
    animation: slideUp 0.3s ease;
    display: flex;
    flex-direction: column;
  }
  
  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2rem 2.5rem 1.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  
  .settings-header h2 {
    font-size: 1.75rem;
    font-weight: 600;
    margin: 0;
    color: #fff;
  }
  
  .settings-close {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 50%;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    transition: all 0.2s ease;
  }
  
  .settings-close:hover {
    background: rgba(255, 0, 0, 0.2);
    border-color: rgba(255, 0, 0, 0.3);
    color: #ff5555;
    transform: scale(1.1);
  }
  
  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 2rem 2.5rem;
  }
  
  .settings-section {
    margin-bottom: 2rem;
  }
  
  .settings-section:last-child {
    margin-bottom: 0;
  }
  
  .settings-section h3 {
    font-size: 1.125rem;
    font-weight: 600;
    color: #fff;
    margin: 0 0 1.5rem 0;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  
  .settings-group {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  
  .settings-item {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 1rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    gap: 1rem;
  }
  
  .settings-item-label {
    flex: 1;
  }
  
  .settings-item-title {
    font-size: 0.9375rem;
    font-weight: 600;
    color: #fff;
    margin-bottom: 0.25rem;
  }
  
  .settings-item-desc {
    font-size: 0.8125rem;
    color: rgba(255, 255, 255, 0.6);
    line-height: 1.4;
  }
  
  .settings-item-status {
    display: flex;
    align-items: center;
  }
  
  .status-badge {
    padding: 0.375rem 0.75rem;
    border-radius: 6px;
    font-size: 0.8125rem;
    font-weight: 600;
    white-space: nowrap;
  }
  
  .status-badge.active {
    background: rgba(192, 101, 182, 0.2);
    color: #C065B6;
    border: 1px solid rgba(192, 101, 182, 0.3);
  }
  
  .status-badge.inactive {
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  
  
  /* Setup Dialog Styles */
  .setup-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
    animation: fadeIn 0.3s ease;
  }
  
  .setup-modal {
    background: rgba(20, 20, 20, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    padding: 2.5rem;
    min-width: 500px;
    max-width: 600px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
    animation: slideUp 0.3s ease;
  }
  
  .setup-modal h2 {
    font-size: 1.75rem;
    font-weight: 600;
    margin: 0 0 1rem 0;
    color: #fff;
  }
  
  .setup-description {
    font-size: 0.9375rem;
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.6;
    margin: 0 0 2rem 0;
  }
  
  .setup-checklist {
    margin-bottom: 2rem;
  }
  
  .setup-checklist h3 {
    font-size: 0.875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.6);
    margin: 0 0 1rem 0;
  }
  
  .setup-item {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1rem;
  }
  
  .setup-item-header {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }
  
  .checkbox {
    width: 24px;
    height: 24px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-top: 2px;
  }
  
  .checkbox.checked {
    background: #C065B6;
    border-color: #C065B6;
  }
  
  .setup-item-info {
    flex: 1;
  }
  
  .setup-item-info.full-width {
    width: 100%;
  }
  
  .setup-item-title {
    font-size: 1rem;
    font-weight: 600;
    color: #fff;
    margin-bottom: 0.25rem;
  }
  
  .setup-item-desc {
    font-size: 0.875rem;
    color: rgba(255, 255, 255, 0.6);
  }
  
  .model-choices {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 1rem;
  }
  
  .model-radio {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    padding: 0.75rem;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
    background: rgba(255, 255, 255, 0.02);
  }
  
  .model-radio:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.2);
  }
  
  .model-radio:has(input[type="radio"]:checked) {
    background: rgba(192, 101, 182, 0.12);
    border-color: rgba(192, 101, 182, 0.4);
  }
  
  .model-radio input[type="radio"] {
    margin-top: 2px;
    cursor: pointer;
  }
  
  .model-radio input[type="radio"]:checked + .radio-content {
    color: #fff;
  }
  
  .model-radio input[type="radio"]:checked + .radio-content .radio-title {
    color: #C065B6;
  }
  
  .radio-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .radio-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }
  
  .radio-title {
    font-size: 0.9375rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
  }
  
  .installed-badge {
    font-size: 0.75rem;
    font-weight: 600;
    color: #C065B6;
    background: rgba(192, 101, 182, 0.15);
    padding: 0.125rem 0.5rem;
    border-radius: 4px;
    border: 1px solid rgba(192, 101, 182, 0.3);
  }
  
  .radio-desc {
    font-size: 0.8125rem;
    color: rgba(255, 255, 255, 0.6);
  }
  
  .download-progress {
    margin: 1.5rem 0;
    padding: 1rem;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 8px;
  }
  
  .download-status {
    display: flex;
    justify-content: space-between;
    align-items: center;
    font-size: 0.875rem;
    color: rgba(255, 255, 255, 0.8);
    margin-top: 0.5rem;
  }
  
  .setup-actions {
    display: flex;
    gap: 1rem;
    justify-content: flex-end;
  }
  
  .setup-button {
    padding: 0.75rem 1.5rem;
    border-radius: 8px;
    font-size: 0.9375rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
    border: none;
  }
  
  .setup-button.secondary {
    background: rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.9);
  }
  
  .setup-button.secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.15);
  }
  
  .setup-button.primary {
    background: #fff;
    color: #000;
    border: 1px solid rgba(255, 255, 255, 0.2);
  }
  
  .setup-button.primary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.9);
    transform: translateY(-1px);
  }
  
  .setup-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
  
  .setup-warning {
    margin-top: 1rem;
    padding: 0.75rem 1rem;
    background: rgba(255, 171, 151, 0.1);
    border: 1px solid rgba(255, 171, 151, 0.3);
    border-radius: 6px;
    font-size: 0.875rem;
    color: rgba(255, 255, 255, 0.9);
    text-align: center;
  }
  
  .setup-warning a {
    color: #FFAB97;
    text-decoration: underline;
  }
  
  .setup-warning a:hover {
    color: #FF6362;
  }
  
  /* Custom Context Menu */
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
    animation: fadeIn 0.15s ease;
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
    transition: background 0.15s ease;
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }
  
  .context-menu-item:hover {
    background: rgba(255, 255, 255, 0.1);
  }
  
  
  .context-menu-item span {
    flex: 1;
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
    animation: fadeIn 0.15s ease;
  }
</style>
