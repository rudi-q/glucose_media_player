<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount, getContext } from "svelte";
  import { goto } from "$app/navigation";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { X, Settings, FolderOpen, Play } from "lucide-svelte";
  import { watchProgressStore, type WatchProgress } from "$lib/stores/watchProgressStore";
  import type { VideoFile } from "$lib/types/video";
  import { formatDuration } from "$lib/utils/time";
  
  // Module-level cache that persists across component remounts
  let cachedVideos: VideoFile[] = [];
  let videosLoaded = false;
  
  let recentVideos = $state<VideoFile[]>([]);
  let loadingRecent = $state(true);
  let thumbnailCache = $state<Map<string, string>>(new Map());
  let watchProgressMap = $state($watchProgressStore);
  let selectedVideoIndex = $state(0);
  let showCloseButton = $state(false);
  let hideCloseButtonTimeout: ReturnType<typeof setTimeout>;
  let showGalleryContextMenu = $state(false);
  let galleryContextMenuPosition = $state({ x: 0, y: 0 });
  let isDragging = $state(false);
  
  // Get context functions from layout
  const showSettings = getContext<() => void>('showSettings');
  
  onMount(() => {
    // Only load if not already loaded
    if (videosLoaded) {
      recentVideos = cachedVideos;
      loadingRecent = false;
      return;
    }
    
    // Load recent videos
    (async () => {
      try {
        const videos = await invoke<VideoFile[]>("get_recent_videos");
        recentVideos = videos;
        cachedVideos = videos;
        videosLoaded = true;
        
        // Load watch progress for all videos
        const progressData = await invoke<Record<string, WatchProgress>>("get_all_watch_progress");
        watchProgressStore.loadAllProgress(progressData);
      } catch (err) {
        console.error("Failed to load recent videos:", err);
      } finally {
        loadingRecent = false;
      }
    })();
    
    // Keyboard shortcuts for gallery
    document.addEventListener("keydown", handleKeyPress);
    
    return () => {
      document.removeEventListener("keydown", handleKeyPress);
    };
  });
  
  function handleKeyPress(e: KeyboardEvent) {
    // Close app
    if (e.key === "Escape") {
      e.preventDefault();
      closeApp();
      return;
    }
    
    // Gallery navigation
    if (recentVideos.length > 0) {
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
          selectedVideoIndex = Math.max(0, selectedVideoIndex - 4);
          scrollSelectedVideoIntoView();
          break;
        case "ArrowDown":
          e.preventDefault();
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
  
  async function loadVideo(path: string) {
    const encodedPath = encodeURIComponent(path);
    await goto(`/player/${encodedPath}`);
  }
  
  async function closeApp() {
    console.log('closeApp called');
    try {
      const { exit } = await import('@tauri-apps/plugin-process');
      await exit(0);
    } catch (err) {
      console.error('Failed to exit app with process plugin:', err);
      try {
        const { getCurrentWindow } = await import('@tauri-apps/api/window');
        const window = getCurrentWindow();
        await window.close();
      } catch (fallbackErr) {
        console.error('Fallback close also failed:', fallbackErr);
        try {
          await invoke('exit_app');
        } catch (invokeErr) {
          console.error('Invoke exit_app also failed:', invokeErr);
        }
      }
    }
  }
  
  function getRemainingTime(videoPath: string, videoDuration?: number): string {
    if (!videoDuration) return '';
    
    const progress = watchProgressMap.get(videoPath);
    if (!progress || !progress.duration) {
      const mins = Math.ceil(videoDuration / 60);
      return `${mins} min${mins !== 1 ? 's' : ''} remaining`;
    }
    
    const remaining = videoDuration - progress.current_time;
    if (remaining <= 0) return 'Finished';
    
    const mins = Math.ceil(remaining / 60);
    return `${mins} min${mins !== 1 ? 's' : ''} remaining`;
  }
  
  async function generateThumbnail(videoPath: string): Promise<string> {
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
      video.crossOrigin = 'anonymous';
      
      video.onloadedmetadata = () => {
        video.currentTime = Math.min(1, video.duration * 0.1);
      };
      
      video.onseeked = () => {
        try {
          const targetWidth = 320;
          const aspectRatio = video.videoWidth / video.videoHeight;
          
          canvas.width = targetWidth;
          canvas.height = Math.round(targetWidth / aspectRatio);
          
          ctx.drawImage(video, 0, 0, canvas.width, canvas.height);
          const thumbnail = canvas.toDataURL('image/jpeg', 0.7);
          thumbnailCache.set(videoPath, thumbnail);
          resolve(thumbnail);
        } catch (err) {
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
  
  function handleMainContainerMouseMove() {
    showCloseButton = true;
    clearTimeout(hideCloseButtonTimeout);
    hideCloseButtonTimeout = setTimeout(() => {
      showCloseButton = false;
    }, 1000);
  }
  
  function handleGalleryContextMenu(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (target.closest('.video-card')) return;
    
    e.preventDefault();
    galleryContextMenuPosition = { x: e.clientX, y: e.clientY };
    showGalleryContextMenu = true;
  }
  
  function handleClickOutside(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (showGalleryContextMenu && !target.closest('.context-menu')) {
      showGalleryContextMenu = false;
    }
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
  
  onMount(() => {
    document.addEventListener("click", handleClickOutside);
    return () => {
      document.removeEventListener("click", handleClickOutside);
    };
  });
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
          <button class="open-button" onclick={() => showSettings()}>
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
      <button class="context-menu-item" onclick={() => { showSettings(); showGalleryContextMenu = false; }}>
        <Settings size={16} />
        <span>Settings</span>
      </button>
    </div>
  {/if}
</main>

<style>
  .player-container:has(.empty-state) {
    background: rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(40px);
    -webkit-backdrop-filter: blur(40px);
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
</style>
