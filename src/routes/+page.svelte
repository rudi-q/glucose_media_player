<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, getContext } from "svelte";
  import { goto } from "$app/navigation";
  import { convertFileSrc } from "@tauri-apps/api/core";
  import { X, Settings, FolderOpen, Play, Music2, Maximize2, PictureInPicture2, Cloud, ArrowUpDown, Volume2, VolumeX, ListFilter } from "lucide-svelte";
  import { revealItemInDir } from "@tauri-apps/plugin-opener";
  import { isAudio } from "$lib/utils/mediaType";
  import { watchProgressStore, type WatchProgress } from "$lib/stores/watchProgressStore";
  import { galleryRefreshStore } from "$lib/stores/appStore";
  import type { VideoFile } from "$lib/types/video";
  import { createFadedMediaPlayback } from "$lib/utils/fadedMediaPlayback";
  import { formatDuration } from "$lib/utils/time";
  import Button from "$lib/components/Button.svelte";
  
  // Per-instance cache that persists across remounts within the same component
  // lifetime. These are intentionally instance-scoped (not module-level); the
  // cache is valid as long as the component has not been destroyed.
  let cachedVideos: VideoFile[] = [];
  let videosLoaded = false;
  // Set to true during the unmount cleanup path so any in-flight createThumbnail
  // jobs know they must not store or expose new blob URLs.
  let destroyed = false;
  
  let recentVideos = $state<VideoFile[]>([]);
  let loadingRecent = $state(true);
  let thumbnailCache = $state<Map<string, string>>(new Map());
  const thumbnailPromises = new Map<string, Promise<string>>();
  const thumbnailQueue: Array<() => void> = [];
  let activeThumbnailJobs = 0;
  const MAX_THUMBNAIL_JOBS = 2;
  let watchProgressMap = $derived($watchProgressStore);
  let selectedVideoIndex = $state(0);
  let showCloseButton = $state(false);
  let hideCloseButtonTimeout: ReturnType<typeof setTimeout>;
  let showGalleryContextMenu = $state(false);
  let galleryContextMenuPosition = $state({ x: 0, y: 0 });
  let showCardContextMenu = $state(false);
  let cardContextMenuPosition = $state({ x: 0, y: 0 });
  let cardContextMenuVideo = $state<VideoFile | null>(null);
  let isDragging = $state(false);
  let logoReady = $state(false);
  let hoveredPath = $state<string | null>(null);
  let previewActivePath = $state<string | null>(null);
  let previewPlayingPath = $state<string | null>(null);
  let previewTransformOrigin = $state('center center');
  let hoverTimer: ReturnType<typeof setTimeout> | null = null;
  let previewFadeOutTimer: ReturnType<typeof setTimeout> | null = null;
  const _previewMutedStored = localStorage.getItem('glucose_preview_muted');
  let previewMuted = $state(_previewMutedStored === null ? true : _previewMutedStored === 'true');
  const _savedSort = localStorage.getItem('glucose_sort');
  let sortBy = $state<'added' | 'watched'>(
    _savedSort === 'added' || _savedSort === 'watched' ? _savedSort : 'watched'
  );
  let showSortMenu = $state(false);
  let sortMenuPos = $state({ top: 0, right: 0 });

  const _savedFilter = localStorage.getItem('glucose_filter');
  let filterBy = $state<'all' | 'video' | 'audio'>(
    _savedFilter === 'video' || _savedFilter === 'audio' ? _savedFilter : 'all'
  );
  let showFilterMenu = $state(false);
  let filterMenuPos = $state({ top: 0, right: 0 });

  let libraryHeaderHeight = $state(96);

  let filteredVideos = $derived(
    filterBy === 'all'
      ? recentVideos
      : recentVideos.filter(v => {
          const audio = isAudio(v.path);
          return filterBy === 'audio' ? audio : !audio;
        })
  );

  let sortedVideos = $derived(
    sortBy === 'watched'
      ? [...filteredVideos].sort((a, b) => {
          const aTime = watchProgressMap.get(a.path)?.last_watched ?? 0;
          const bTime = watchProgressMap.get(b.path)?.last_watched ?? 0;
          return bTime - aTime;
        })
      : filteredVideos
  );

  $effect(() => {
    localStorage.setItem('glucose_sort', sortBy);
    // Reset keyboard focus when sort order changes (watching sortBy, not sortedVideos, to
    // avoid resetting on every duration update which also replaces the recentVideos array)
    selectedVideoIndex = 0;
  });

  $effect(() => {
    localStorage.setItem('glucose_filter', filterBy);
    selectedVideoIndex = 0;
  });

  $effect(() => {
    localStorage.setItem('glucose_preview_muted', String(previewMuted));
  });

  type VideoGroup = { label: string | null; videos: { video: VideoFile; index: number }[] };

  function getTimeGroup(timestampSecs: number): string {
    if (timestampSecs === 0) return 'Older';
    const date = new Date(timestampSecs * 1000);
    const now = new Date();
    const yr = now.getFullYear();
    const mo = now.getMonth();
    const dy = now.getDate();
    if (date.getFullYear() === yr && date.getMonth() === mo && date.getDate() === dy) return 'Today';
    const yesterday = new Date(now); yesterday.setDate(dy - 1);
    if (date.getFullYear() === yesterday.getFullYear() && date.getMonth() === yesterday.getMonth() && date.getDate() === yesterday.getDate()) return 'Yesterday';
    const nowSecs = Date.now() / 1000;
    const daySecs = 86400;
    if (timestampSecs >= nowSecs - 7 * daySecs) return 'This Week';
    if (timestampSecs >= nowSecs - 14 * daySecs) return 'Last Week';
    if (date.getFullYear() === yr && date.getMonth() === mo) return 'This Month';
    const prevMo = mo === 0 ? 11 : mo - 1;
    const prevMoYr = mo === 0 ? yr - 1 : yr;
    if (date.getFullYear() === prevMoYr && date.getMonth() === prevMo) return 'Last Month';
    if (date.getFullYear() === yr) return 'This Year';
    if (date.getFullYear() === yr - 1) return 'Last Year';
    return 'Older';
  }

  let groupedVideos = $derived.by((): VideoGroup[] => {
    const order = ['Today', 'Yesterday', 'This Week', 'Last Week', 'This Month', 'Last Month', 'This Year', 'Last Year', 'Older'];
    const buckets = new Map<string, { video: VideoFile; index: number }[]>();
    for (const label of order) buckets.set(label, []);
    sortedVideos.forEach((video, index) => {
      const ts = sortBy === 'watched'
        ? (watchProgressMap.get(video.path)?.last_watched ?? 0)
        : video.modified;
      buckets.get(getTimeGroup(ts))!.push({ video, index });
    });
    const groups: VideoGroup[] = [];
    for (const label of order) {
      const vids = buckets.get(label)!;
      if (vids.length > 0) {
        groups.push({ label, videos: vids });
      }
    }
    return groups;
  });

  // Get context functions from layout
  const showSettings = getContext<() => void>('showSettings');
  
  async function loadVideos() {
    loadingRecent = true;
    try {
      const videosPromise = invoke<VideoFile[]>("get_recent_videos");
      const progressPromise = invoke<Record<string, WatchProgress>>("get_all_watch_progress");
      const [videos, progressData] = await Promise.all([videosPromise, progressPromise]);
      watchProgressStore.loadAllProgress(progressData);
      recentVideos = videos;
      cachedVideos = videos;
      videosLoaded = true;
      // Fetch durations in the background â€” gallery is already visible at this point
      invoke("fetch_video_durations", { paths: videos.filter(v => !v.is_cloud_only).map(v => v.path) }).catch(console.error);
    } catch (err) {
      console.error("Failed to load recent videos:", err);
    } finally {
      loadingRecent = false;
    }
  }

  $effect(() => {
    if ($galleryRefreshStore > 0) {
      selectedVideoIndex = 0;
      videosLoaded = false;
      cachedVideos = [];
      watchProgressStore.clear();
      loadVideos();
    }
  });

  onMount(() => {
    logoReady = true;
    document.addEventListener("keydown", handleKeyPress);
    document.addEventListener("click", handleClickOutside);

    let unlistenDuration: (() => void) | undefined;
    let cancelled = false;

    // Await listener registration before triggering any duration fetches so no
    // "video-duration-ready" events are dropped between invoke and handler setup
    (async () => {
      const unlisten = await listen<{ path: string; duration: number | null }>("video-duration-ready", (event) => {
        const { path, duration } = event.payload;
        if (duration !== null) {
          recentVideos = recentVideos.map(v => v.path === path ? { ...v, duration } : v);
          cachedVideos = cachedVideos.map(v => v.path === path ? { ...v, duration } : v);
        }
      });

      if (cancelled) {
        unlisten();
        return;
      }

      unlistenDuration = unlisten;

      if (!videosLoaded) {
        loadVideos();
      } else {
        recentVideos = cachedVideos;
        loadingRecent = false;
        // Re-fetch durations for any cached videos that are still missing them
        const missing = cachedVideos.filter(v => !v.duration && !v.is_cloud_only).map(v => v.path);
        if (missing.length > 0) {
          invoke("fetch_video_durations", { paths: missing }).catch(console.error);
        }
      }
    })();

    return () => {
      cancelled = true;
      destroyed = true;
      if (hoverTimer !== null) { clearTimeout(hoverTimer); hoverTimer = null; }
      if (previewFadeOutTimer !== null) { clearTimeout(previewFadeOutTimer); previewFadeOutTimer = null; }
      clearThumbnailCache();
      document.removeEventListener("keydown", handleKeyPress);
      document.removeEventListener("click", handleClickOutside);
      unlistenDuration?.();
    };
  });

  function onCardHoverEnter(video: VideoFile) {
    if (hoverTimer !== null) { clearTimeout(hoverTimer); hoverTimer = null; }
    const progress = watchProgressMap.get(video.path);
    if (!progress || !(progress.current_time > 0) || video.is_cloud_only || isAudio(video.path)) return;
    // Only cancel the fade-out timer when we know we're activating a new eligible preview.
    if (previewFadeOutTimer !== null) { clearTimeout(previewFadeOutTimer); previewFadeOutTimer = null; }
    if (hoveredPath === video.path) {
      previewActivePath = video.path;
      return;
    }
    hoverTimer = setTimeout(() => {
      hoveredPath = video.path;
      previewActivePath = video.path;
    }, 400);
  }

  function onCardHoverLeave() {
    if (hoverTimer !== null) { clearTimeout(hoverTimer); hoverTimer = null; }
    previewActivePath = null;
    if (previewFadeOutTimer !== null) { clearTimeout(previewFadeOutTimer); previewFadeOutTimer = null; }

    const fadingPath = hoveredPath;
    if (!fadingPath) {
      previewPlayingPath = null;
      previewTransformOrigin = 'center center';
      return;
    }

    previewFadeOutTimer = setTimeout(() => {
      if (hoveredPath === fadingPath && previewActivePath === null) {
        hoveredPath = null;
        previewPlayingPath = null;
        previewTransformOrigin = 'center center';
      }
      previewFadeOutTimer = null;
    }, 800);
  }

  type HoverPreviewOptions = {
    startTime: number;
    active: boolean;
    muted: boolean;
  };

  function hoverPreview(node: HTMLVideoElement, options: HoverPreviewOptions) {
    let currentOptions = options;
    let aborted = false;
    const safetyTimeout = setTimeout(() => { aborted = true; abort(); }, 4000);
    const seekEpsilon = 0.01;
    const fadedPlayback = createFadedMediaPlayback({
      getMediaElement: () => node,
      getTargetVolume: () => currentOptions.muted ? 0 : 1,
      getOutputVolume: () => node.volume,
      setOutputVolume: (value) => {
        node.volume = Math.min(1, Math.max(0, value));
      },
    });

    function abort() {
      fadedPlayback.destroy();
      node.pause();
      try { node.removeAttribute('src'); node.load(); } catch {}
    }

    function startPlayback() {
      if (aborted || !currentOptions.active) return;
      clearTimeout(safetyTimeout);
      node.muted = currentOptions.muted;
      fadedPlayback.play().catch(() => {});
    }

    function onMeta() {
      if (aborted) return;
      if (Math.abs(node.currentTime - currentOptions.startTime) < seekEpsilon) {
        startPlayback();
        return;
      }
      try { node.currentTime = currentOptions.startTime; } catch { startPlayback(); }
    }

    function onSeeked() {
      startPlayback();
    }

    function onPlaying() {
      if (!aborted) node.classList.add('playing');
    }

    node.addEventListener('loadedmetadata', onMeta, { once: true });
    node.addEventListener('seeked', onSeeked, { once: true });
    node.addEventListener('playing', onPlaying, { once: true });
    node.addEventListener('error', () => { aborted = true; clearTimeout(safetyTimeout); abort(); }, { once: true });

    return {
      update(nextOptions: HoverPreviewOptions) {
        const wasActive = currentOptions.active;
        currentOptions = nextOptions;
        node.muted = currentOptions.muted;

        if (aborted) return;

        if (wasActive && !currentOptions.active) {
          clearTimeout(safetyTimeout);
          fadedPlayback.pause().catch(() => {});
        } else if (!wasActive && currentOptions.active && node.readyState >= 2) {
          startPlayback();
        } else {
          fadedPlayback.syncOutputVolume();
        }
      },
      destroy() {
        aborted = true;
        clearTimeout(safetyTimeout);
        abort();
      }
    };
  }
  
  function handleKeyPress(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    const tag = target.tagName;
    const isVideoCard = target.classList.contains('video-card');
    if (
      tag === 'BUTTON' || tag === 'A' || tag === 'INPUT' ||
      tag === 'TEXTAREA' || tag === 'SELECT' ||
      (target.getAttribute('role') === 'button' && !isVideoCard) ||
      target.isContentEditable ||
      (target.tabIndex > 0 && tag !== 'DIV')
    ) return;

    // Close app
    if (e.key === "Escape") {
      e.preventDefault();
      closeApp();
      return;
    }
    
    // Gallery navigation
    if (sortedVideos.length > 0) {
      switch (e.key) {
        case "ArrowLeft":
          e.preventDefault();
          selectedVideoIndex = Math.max(0, selectedVideoIndex - 1);
          scrollSelectedVideoIntoView();
          break;
        case "ArrowRight":
          e.preventDefault();
          selectedVideoIndex = Math.min(sortedVideos.length - 1, selectedVideoIndex + 1);
          scrollSelectedVideoIntoView();
          break;
        case "ArrowUp":
          e.preventDefault();
          selectedVideoIndex = getVerticalNavigationIndex('up');
          scrollSelectedVideoIntoView();
          break;
        case "ArrowDown":
          e.preventDefault();
          selectedVideoIndex = getVerticalNavigationIndex('down');
          scrollSelectedVideoIntoView();
          break;
        case "Enter":
        case " ":
          if (isVideoCard) return;
          e.preventDefault();
          if (sortedVideos[selectedVideoIndex]) {
            loadVideo(sortedVideos[selectedVideoIndex].path);
          }
          break;
      }
    }
  }

  function getVerticalNavigationIndex(direction: 'up' | 'down'): number {
    const cards = Array.from(document.querySelectorAll<HTMLElement>('.video-card[data-index]'));
    const currentCard = cards.find((card) => Number(card.dataset.index) === selectedVideoIndex);
    if (!currentCard) return selectedVideoIndex;

    const currentRect = currentCard.getBoundingClientRect();
    const currentCenterX = currentRect.left + currentRect.width / 2;
    const currentCenterY = currentRect.top + currentRect.height / 2;
    const rowThreshold = Math.max(8, currentRect.height * 0.25);

    let bestCard: HTMLElement | null = null;
    let bestScore = Number.POSITIVE_INFINITY;

    for (const card of cards) {
      if (card === currentCard) continue;

      const rect = card.getBoundingClientRect();
      const centerX = rect.left + rect.width / 2;
      const centerY = rect.top + rect.height / 2;
      const verticalDistance = direction === 'down'
        ? centerY - currentCenterY
        : currentCenterY - centerY;

      if (verticalDistance <= rowThreshold) continue;

      const horizontalDistance = Math.abs(centerX - currentCenterX);
      const score = verticalDistance * 1000 + horizontalDistance;

      if (score < bestScore) {
        bestScore = score;
        bestCard = card;
      }
    }

    return bestCard ? Number(bestCard.dataset.index) : selectedVideoIndex;
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
  
  async function loadVideo(path: string, mode?: string) {
    const encodedPath = encodeURIComponent(path);
    const modeParam = mode ? `?mode=${encodeURIComponent(mode)}` : '';
    await goto(isAudio(path) ? `/audio/${encodedPath}` : `/player/${encodedPath}${modeParam}`);
  }

  async function openContainingFolder(path: string) {
    showCardContextMenu = false;
    try {
      await revealItemInDir(path);
    } catch (err) {
      console.error("Failed to reveal item in directory:", err);
    }
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
  
  async function generateThumbnail(videoPath: string, seekTime?: number): Promise<string> {
    const hasSeek = seekTime != null && seekTime > 0;
    const cacheKey = hasSeek ? `${videoPath}@${Math.floor(seekTime!)}` : videoPath;
    if (thumbnailCache.has(cacheKey)) {
      return thumbnailCache.get(cacheKey)!;
    }
    if (thumbnailPromises.has(cacheKey)) {
      return thumbnailPromises.get(cacheKey)!;
    }

    const promise = scheduleThumbnailJob(() => createThumbnail(videoPath, seekTime, hasSeek, cacheKey))
      .finally(() => thumbnailPromises.delete(cacheKey));
    thumbnailPromises.set(cacheKey, promise);
    return promise;
  }

  function scheduleThumbnailJob(job: () => Promise<string>): Promise<string> {
    return new Promise((resolve) => {
      const run = () => {
        activeThumbnailJobs += 1;
        job()
          .then(resolve)
          .catch(() => resolve(''))
          .finally(() => {
            activeThumbnailJobs -= 1;
            thumbnailQueue.shift()?.();
          });
      };

      if (activeThumbnailJobs < MAX_THUMBNAIL_JOBS) {
        run();
      } else {
        thumbnailQueue.push(run);
      }
    });
  }

  function createThumbnail(videoPath: string, seekTime: number | undefined, hasSeek: boolean, cacheKey: string): Promise<string> {
    return new Promise((resolve) => {
      const video = document.createElement('video');
      const canvas = document.createElement('canvas');
      const ctx = canvas.getContext('2d');
      let settled = false;
      const timeout = setTimeout(() => settle(''), 8000);

      function cleanup() {
        clearTimeout(timeout);
        video.onloadedmetadata = null;
        video.onseeked = null;
        video.onerror = null;
        try {
          video.removeAttribute('src');
          video.load();
        } catch {}
      }

      function settle(thumbnail: string) {
        if (settled) return;
        settled = true;
        cleanup();
        resolve(thumbnail);
      }

      function capture() {
        try {
          const targetWidth = 320;
          const aspectRatio = video.videoWidth / video.videoHeight;

          if (!Number.isFinite(aspectRatio) || aspectRatio <= 0) {
            settle('');
            return;
          }

          canvas.width = targetWidth;
          canvas.height = Math.round(targetWidth / aspectRatio);

          ctx!.drawImage(video, 0, 0, canvas.width, canvas.height);
          canvas.toBlob((blob) => {
            if (!blob) {
              settle('');
              return;
            }
            const url = URL.createObjectURL(blob);
            // Guard against post-unmount blob URL leaks: if the component was
            // destroyed while this async job was in flight, immediately revoke
            // the URL we just created and resolve with an empty string.
            if (destroyed) {
              URL.revokeObjectURL(url);
              settle('');
              return;
            }
            thumbnailCache.set(cacheKey, url);
            settle(url);
          }, 'image/jpeg', 0.7);
        } catch (err) {
          if (import.meta.env.DEV) {
            console.log('Thumbnail generation skipped:', videoPath, err);
          }
          settle('');
        }
      }

      if (!ctx) {
        settle('');
        return;
      }

      video.muted = true;
      video.preload = 'metadata';
      video.playsInline = true;
      video.crossOrigin = 'anonymous';

      video.onloadedmetadata = () => {
        const defaultTime = Number.isFinite(video.duration) ? Math.min(1, video.duration * 0.1) : 0;
        const targetTime = hasSeek ? seekTime! : defaultTime;
        if (targetTime <= 0) {
          capture();
          return;
        }
        try {
          video.currentTime = targetTime;
        } catch {
          capture();
        }
      };

      video.onseeked = capture;

      video.onerror = () => settle('');
      video.src = convertFileSrc(videoPath);
    });
  }

  function clearThumbnailCache() {
    thumbnailQueue.length = 0;
    thumbnailPromises.clear();
    for (const thumbnail of thumbnailCache.values()) {
      if (thumbnail.startsWith('blob:')) {
        URL.revokeObjectURL(thumbnail);
      }
    }
    thumbnailCache.clear();
  }

  function handleMainContainerMouseMove() {
    showCloseButton = true;
    clearTimeout(hideCloseButtonTimeout);
    hideCloseButtonTimeout = setTimeout(() => {
      showCloseButton = false;
    }, 1000);
  }
  
  function toggleSortMenu(e: MouseEvent) {
    if (showSortMenu) { showSortMenu = false; return; }
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    sortMenuPos = { top: rect.bottom + 8, right: window.innerWidth - rect.right };
    showSortMenu = true;
    showFilterMenu = false;
  }

  function toggleFilterMenu(e: MouseEvent) {
    if (showFilterMenu) { showFilterMenu = false; return; }
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    filterMenuPos = { top: rect.bottom + 8, right: window.innerWidth - rect.right };
    showFilterMenu = true;
    showSortMenu = false;
  }

  function handleGalleryContextMenu(e: MouseEvent) {
    e.preventDefault();
    const target = e.target as HTMLElement;
    const card = target.closest('.video-card');
    if (card) {
      const index = parseInt((card as HTMLElement).dataset.index ?? '-1', 10);
      const video = sortedVideos[index];
      if (video) {
        cardContextMenuVideo = video;
        cardContextMenuPosition = { x: e.clientX, y: e.clientY };
        showCardContextMenu = true;
        showGalleryContextMenu = false;
      }
      return;
    }
    showCardContextMenu = false;
    galleryContextMenuPosition = { x: e.clientX, y: e.clientY };
    showGalleryContextMenu = true;
  }
  
  function handleClickOutside(e: MouseEvent) {
    const target = e.target as HTMLElement;
    if (!target.closest('.context-menu')) {
      showGalleryContextMenu = false;
      showCardContextMenu = false;
    }
    if (!target.closest('.sort-menu') && !target.closest('.sort-toggle')) {
      showSortMenu = false;
    }
    if (!target.closest('.filter-menu') && !target.closest('.filter-toggle')) {
      showFilterMenu = false;
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
  
</script>

<main 
  class="player-container"
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
  onmousemove={handleMainContainerMouseMove}
  oncontextmenu={handleGalleryContextMenu}
>
  <div class="close-button-wrapper" class:visible={showCloseButton}>
    <Button
      variant="secondary"
      size="sm"
      onclick={closeApp}
      title="Close (Esc)"
    >
      <X size={16} />
    </Button>
  </div>
  

  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="empty-state" class:dragging={isDragging}>
    <div class="library-container">
      <div class="library-header" bind:offsetHeight={libraryHeaderHeight}>
        <img src="/logo-dark.svg" alt="glucose" class="logo" class:logo-animate={logoReady} />
        <div class="header-buttons">
          <button class="sort-toggle" onclick={toggleSortMenu} title="Sort" class:sort-active={sortBy === 'watched'}>
            <ArrowUpDown size={15} />
          </button>
          <button class="sort-toggle filter-toggle" onclick={toggleFilterMenu} title="Filter" class:sort-active={filterBy !== 'all'}>
            <ListFilter size={15} />
          </button>
          <Button variant="white" onclick={openFileDialog}>
            Open File
          </Button>
          <Button variant="secondary" onclick={() => showSettings()}>
            Settings
          </Button>
        </div>
      </div>
      
      {#if loadingRecent}
        <div class="loading">Scanning for videos...</div>
      {:else if sortedVideos.length === 0}
        <div class="empty-content">
          <Play size={64} strokeWidth={1.5} />
          <p>No recent videos found</p>
          <p class="hint">Drop a file or click Open File above</p>
        </div>
      {:else}
        <div class="recent-section">
          {#each groupedVideos as group}
            <div class="section-group">
              {#if group.label}
                <div class="section-header" style="top: calc({libraryHeaderHeight}px - 1.5rem)">{group.label}</div>
              {/if}
              <div class="video-grid">
                {#each group.videos as { video, index } (video.path)}
                  <div
                    class="video-card"
                    class:selected={selectedVideoIndex === index}
                    class:preview-expanding={previewPlayingPath === video.path}
                    style:transform-origin={previewPlayingPath === video.path ? previewTransformOrigin : ''}
                    data-index={index}
                    role="button"
                    tabindex="0"
                    onclick={() => loadVideo(video.path)}
                    onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); loadVideo(video.path); } }}
                    onmouseenter={() => onCardHoverEnter(video)}
                    onmouseleave={onCardHoverLeave}
                  >
                    <div class="video-thumbnail" class:audio-card={isAudio(video.path)}>
                      {#if isAudio(video.path)}
                        <div class="audio-thumb">
                          <Music2 size={40} strokeWidth={1.2} />
                        </div>
                      {:else}
                        {#await generateThumbnail(video.path, watchProgressMap.get(video.path)?.current_time)}
                          <Play size={48} strokeWidth={1.5} />
                        {:then thumbnail}
                          {#if thumbnail}
                            <img src={thumbnail} alt={video.name} class="thumbnail-img" />
                          {:else}
                            <Play size={48} strokeWidth={1.5} />
                          {/if}
                        {/await}
                        {#if hoveredPath === video.path}
                          {@const progress = watchProgressMap.get(video.path)!}
                          {@const lookback = Math.max(20, (progress.duration || 0) * 0.005)}
                          {@const startTime = Math.max(0, progress.current_time - lookback)}
                          <!-- svelte-ignore a11y_media_has_caption -->
                          <video
                            class="hover-preview"
                            src={convertFileSrc(video.path)}
                            playsinline
                            bind:muted={previewMuted}
                            use:hoverPreview={{
                              startTime,
                              active: previewActivePath === video.path,
                              muted: previewMuted,
                            }}
                            onplaying={(e) => {
                              const card = (e.currentTarget as HTMLElement).closest('.video-card') as HTMLElement | null;
                              if (card) {
                                const rect = card.getBoundingClientRect();
                                const halfExtra = rect.width * 0.09;
                                const ox = rect.left - halfExtra < 4 ? 'left'
                                  : rect.right + halfExtra > window.innerWidth - 4 ? 'right'
                                  : 'center';
                                previewTransformOrigin = `${ox} center`;
                              }
                              previewPlayingPath = video.path;
                            }}
                          ></video>
                          <button
                            class="preview-mute-btn"
                            type="button"
                            onclick={(e) => { e.stopPropagation(); previewMuted = !previewMuted; }}
                            onkeydown={(e) => { if (e.key === 'Enter' || e.key === ' ') e.stopPropagation(); }}
                            title={previewMuted ? 'Unmute preview' : 'Mute preview'}
                            aria-pressed={!previewMuted}
                          >
                            {#if previewMuted}
                              <VolumeX size={13} />
                            {:else}
                              <Volume2 size={13} />
                            {/if}
                          </button>
                        {/if}
                      {/if}
                      {#if previewPlayingPath !== video.path}
                        <div class="play-overlay">
                          <Play size={32} fill="white" stroke="none" />
                        </div>
                      {/if}
                      {#if watchProgressMap.has(video.path)}
                        {@const progress = watchProgressMap.get(video.path)}
                        {@const progressPercent = progress && progress.duration > 0 ? (progress.current_time / progress.duration) * 100 : 0}
                        {#if progressPercent > 0 && progressPercent < 100}
                          <div class="video-progress-bar">
                            <div class="video-progress-fill" style="width: {progressPercent}%"></div>
                          </div>
                        {/if}
                      {/if}
                      {#if video.is_cloud_only}
                        <div class="cloud-badge" title="Not downloaded â€” stored in cloud">
                          <Cloud size={13} />
                        </div>
                      {/if}
                    </div>
                    <div class="video-info">
                      <div class="video-name" title={video.name}>{video.name}</div>
                      <div class="video-meta">
                        {#if video.duration}
                          <span class="video-duration">{formatDuration(video.duration)}</span>
                          <span class="video-separator">â€¢</span>
                          <span class="video-remaining">{getRemainingTime(video.path, video.duration)}</span>
                        {:else}
                          <span>{(video.size / (1024 * 1024)).toFixed(1)} MB</span>
                        {/if}
                      </div>
                    </div>
                  </div>
                {/each}
              </div>
            </div>
          {/each}
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
        <span>Open File</span>
      </button>
      <button class="context-menu-item" onclick={() => { showSettings(); showGalleryContextMenu = false; }}>
        <Settings size={16} />
        <span>Settings</span>
      </button>
    </div>
  {/if}

  {#if showCardContextMenu && cardContextMenuVideo}
    <div
      class="context-menu"
      style="left: {cardContextMenuPosition.x}px; top: {cardContextMenuPosition.y}px;"
    >
      <button class="context-menu-item" onclick={() => { loadVideo(cardContextMenuVideo!.path); showCardContextMenu = false; }}>
        <Play size={16} />
        <span>Play</span>
      </button>
      <button class="context-menu-item" onclick={() => openContainingFolder(cardContextMenuVideo!.path)}>
        <FolderOpen size={16} />
        <span>Open Containing Folder</span>
      </button>
      {#if !isAudio(cardContextMenuVideo.path)}
        <div class="context-menu-separator"></div>
        <button class="context-menu-item" onclick={() => { loadVideo(cardContextMenuVideo!.path, 'fullscreen'); showCardContextMenu = false; }}>
          <Maximize2 size={16} />
          <span>Open in Fullscreen</span>
        </button>
        <button class="context-menu-item" onclick={() => { loadVideo(cardContextMenuVideo!.path, 'pip'); showCardContextMenu = false; }}>
          <PictureInPicture2 size={16} />
          <span>Open in PiP</span>
        </button>
      {/if}
    </div>
  {/if}

  {#if showSortMenu}
    <div class="sort-menu" style="top: {sortMenuPos.top}px; right: {sortMenuPos.right}px;">
      <div class="sort-menu-label">Sort by</div>
      <button class="sort-option" class:active={sortBy === 'added'} onclick={() => { sortBy = 'added'; showSortMenu = false; }}>
        Last Added
      </button>
      <button class="sort-option" class:active={sortBy === 'watched'} onclick={() => { sortBy = 'watched'; showSortMenu = false; }}>
        Last Watched
      </button>
    </div>
  {/if}

  {#if showFilterMenu}
    <div class="sort-menu" style="top: {filterMenuPos.top}px; right: {filterMenuPos.right}px;">
      <div class="sort-menu-label">Show</div>
      <button class="sort-option" class:active={filterBy === 'all'} onclick={() => { filterBy = 'all'; showFilterMenu = false; }}>
        All Files
      </button>
      <button class="sort-option" class:active={filterBy === 'video'} onclick={() => { filterBy = 'video'; showFilterMenu = false; }}>
        Videos Only
      </button>
      <button class="sort-option" class:active={filterBy === 'audio'} onclick={() => { filterBy = 'audio'; showFilterMenu = false; }}>
        Audio Only
      </button>
    </div>
  {/if}
</main>

<style>
  .player-container {
    user-select: none;
  }

  .player-container:has(.empty-state) {
    background:
      radial-gradient(ellipse at 25% 75%, rgba(35, 15, 45, 0.5) 0%, transparent 55%),
      radial-gradient(ellipse at 75% 25%, rgba(10, 20, 45, 0.4) 0%, transparent 55%),
      var(--surface-gallery);
    backdrop-filter: blur(var(--blur-lg));
    -webkit-backdrop-filter: blur(var(--blur-lg));
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

  @keyframes logo-smoke-in {
    0% {
      opacity: 0;
      filter: blur(8px);
    }
    30% {
      opacity: 0.3;
    }
    100% {
      opacity: 0.95;
      filter: blur(0px);
    }
  }

  .library-header .logo {
    height: 48px;
    width: auto;
    opacity: 0;
  }

  .library-header .logo.logo-animate {
    animation: logo-smoke-in 2.8s ease-out forwards;
  }

  .header-buttons {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .header-buttons :global(.btn) {
    backdrop-filter: blur(var(--blur-md));
    -webkit-backdrop-filter: blur(var(--blur-md));
    letter-spacing: 0.025em;
    font-weight: 500;
  }

  /* Open Video â€” prominent frosted glass */
  .header-buttons :global(.white) {
    background: rgba(255, 255, 255, 0.11);
    border-color: rgba(255, 255, 255, 0.22);
    color: #fff;
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.12);
    transform: none;
  }

  .header-buttons :global(.white:hover:not(:disabled)) {
    background: rgba(255, 255, 255, 0.18);
    border-color: rgba(255, 255, 255, 0.38);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.18), 0 4px 24px rgba(255, 255, 255, 0.07);
    transform: translateY(-1px);
  }

  /* Settings â€” subdued ghost */
  .header-buttons :global(.secondary) {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.09);
    color: rgba(255, 255, 255, 0.6);
  }

  .header-buttons :global(.secondary:hover:not(:disabled)) {
    background: rgba(255, 255, 255, 0.08);
    border-color: rgba(255, 255, 255, 0.16);
    color: rgba(255, 255, 255, 0.9);
    transform: translateY(-1px);
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

  .section-group:not(:first-child) {
    margin-top: 2.5rem;
  }

  .section-header {
    position: sticky;
    z-index: 5;
    padding: 1.25rem 0 0.75rem;
    font-size: 0.6875rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.3);
    text-transform: uppercase;
    letter-spacing: 0.08em;
    background: transparent;
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
    transition: transform 0.3s ease, background 0.2s ease, border-color 0.2s ease, box-shadow 0.2s ease;
    text-align: left;
    overflow: hidden;
    position: relative;
  }

  .video-card:hover {
    background: rgba(255, 255, 255, 0.06);
    border-color: rgba(255, 255, 255, 0.15);
    transform: translateY(-2px);
  }

  .video-card.preview-expanding {
    transform: scale(1.18);
    z-index: 10;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.6);
    transition: transform 0.4s cubic-bezier(0.34, 1.1, 0.64, 1), box-shadow 0.4s ease;
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
    background: transparent;
    z-index: 2;
  }
  
  .video-progress-fill {
    height: 100%;
    background: rgba(255, 255, 255, 0.9);
    transition: width 0.3s ease;
    box-shadow: 0 0 8px rgba(255, 255, 255, 0.5);
  }

  .cloud-badge {
    position: absolute;
    top: 0.5rem;
    right: 0.5rem;
    background: var(--surface-badge);
    backdrop-filter: blur(var(--blur-sm));
    -webkit-backdrop-filter: blur(var(--blur-sm));
    border: 1px solid var(--color-border);
    border-radius: 6px;
    padding: 0.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--color-text-muted);
    z-index: 3;
  }

  .thumbnail-img {
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: rgba(0, 0, 0, 0.3);
  }

  .hover-preview {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    background: #000;
    opacity: 0;
    transition: opacity 0.15s ease;
    pointer-events: none;
  }

  .hover-preview:global(.playing) {
    opacity: 1;
  }

  .preview-mute-btn {
    position: absolute;
    bottom: 0.75rem;
    right: 0.5rem;
    z-index: 10;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--surface-badge);
    border: 1px solid var(--color-border);
    border-radius: 50%;
    color: var(--color-text);
    cursor: pointer;
    backdrop-filter: blur(var(--blur-sm));
    -webkit-backdrop-filter: blur(var(--blur-sm));
    transition: background 0.15s ease, border-color 0.15s ease;
  }

  .preview-mute-btn:hover {
    background: var(--color-interactive-hover);
    border-color: var(--color-border-strong);
  }

  .audio-card {
    background: linear-gradient(135deg, rgba(30, 40, 70, 0.6) 0%, rgba(10, 15, 30, 0.8) 100%);
  }

  .audio-thumb {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    color: rgba(140, 180, 255, 0.55);
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

  .close-button-wrapper {
    position: absolute;
    top: 1rem;
    right: 1rem;
    opacity: 0;
    transition: opacity 0.2s ease;
    z-index: 1000;
  }

  .close-button-wrapper.visible {
    opacity: 1;
  }

  :global(.close-button-wrapper .btn) {
    min-width: 32px !important;
    width: 32px;
    padding: 0 !important;
  }

  
  /* Context Menu */
  .context-menu {
    position: fixed;
    background: var(--surface-panel);
    backdrop-filter: blur(var(--blur-md));
    -webkit-backdrop-filter: blur(var(--blur-md));
    border: 1px solid var(--color-border);
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
    color: var(--color-text);
    text-align: left;
    cursor: pointer;
    font-size: 0.875rem;
    transition: all 0.15s ease;
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .context-menu-item:hover {
    background: var(--color-interactive-hover);
  }

  .context-menu-separator {
    height: 1px;
    background: var(--color-interactive);
    margin: 0.25rem 0;
  }

  .sort-toggle {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    background: var(--color-interactive);
    border: 1px solid var(--color-border);
    border-radius: 8px;
    color: var(--color-text-muted);
    cursor: pointer;
    transition: all 0.2s ease;
    flex-shrink: 0;
  }

  .sort-toggle:hover {
    background: var(--color-interactive-hover);
    color: #fff;
  }

  .sort-toggle.sort-active {
    background: var(--color-accent-subtle);
    border-color: var(--color-accent-border);
    color: var(--color-accent);
  }

  .sort-menu {
    position: fixed;
    background: var(--surface-panel);
    backdrop-filter: blur(var(--blur-md));
    -webkit-backdrop-filter: blur(var(--blur-md));
    border: 1px solid var(--color-border);
    border-radius: 10px;
    padding: 0.375rem;
    min-width: 170px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
    z-index: 1000;
  }

  .sort-menu-label {
    font-size: 0.6875rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.3);
    text-transform: uppercase;
    letter-spacing: 0.06em;
    padding: 0.375rem 0.75rem 0.25rem;
  }

  .sort-option {
    width: 100%;
    padding: 0.625rem 0.75rem;
    background: none;
    border: none;
    border-radius: 6px;
    color: rgba(255, 255, 255, 0.75);
    text-align: left;
    font-size: 0.875rem;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .sort-option:hover {
    background: rgba(255, 255, 255, 0.07);
    color: #fff;
  }

  .sort-option.active {
    color: #c065b6;
    background: rgba(192, 101, 182, 0.1);
  }
</style>
