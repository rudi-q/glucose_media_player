<script lang="ts">
  import { onMount, onDestroy } from 'svelte';
  import { goto } from '$app/navigation';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { invoke } from '@tauri-apps/api/core';
  import { appSettings } from '$lib/stores/appStore';
  import { watchProgressStore } from '$lib/stores/watchProgressStore';
  import { formatDuration } from '$lib/utils/time';
  import { X, Play, Pause, Volume1, Volume2, VolumeX, Home } from 'lucide-svelte';

  let { data } = $props();
  const audioPath: string = $derived(data.audioPath);
  const fileName = $derived(audioPath.split(/[\\/]/).pop() ?? audioPath);
  const displayName = $derived(fileName.replace(/\.[^.]+$/, ''));

  // DOM refs
  let audioEl: HTMLAudioElement;
  let canvas: HTMLCanvasElement;
  let containerEl: HTMLDivElement;

  // Web Audio API
  let audioCtx: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let gainNode: GainNode | null = null;
  let sourceConnected = false;

  // Visualizer state
  let animId: number;
  let smoothed: Float32Array = new Float32Array(128);
  // Reusable typed-array buffers — allocated once in setupAudio() to avoid
  // per-frame GC pressure in the animation loop.
  let freqData = new Uint8Array(new ArrayBuffer(0));
  let waveData = new Uint8Array(new ArrayBuffer(0));
  let bassEnergy = 0;

  // Playback state
  let isPlaying = $state(false);
  let currentTime = $state(0);
  let duration = $state(0);
  let isScrubbing = $state(false);
  let scrubValue = $state(0);

  // Volume (persisted via appSettings store)
  let volume = $state($appSettings.volume ?? 1);
  let isMuted = $state($appSettings.isMuted ?? false);
  let showVolumeMenu = $state(false);

  // Progress autosave
  let progressSaveInterval: ReturnType<typeof setInterval>;

  // UI
  let showCloseBtn = $state(false);
  let hideCloseBtnTimer: ReturnType<typeof setTimeout>;
  let controlsVisible = $state(true);
  let hideControlsTimer: ReturnType<typeof setTimeout>;

  // ── Audio context setup ─────────────────────────────────────────────────────

  function setupAudio() {
    if (!audioEl || sourceConnected) return;
    try {
      audioCtx = new AudioContext();
      const src = audioCtx.createMediaElementSource(audioEl);
      analyser = audioCtx.createAnalyser();
      analyser.fftSize = 256;
      analyser.smoothingTimeConstant = 0.75;
      gainNode = audioCtx.createGain();
      gainNode.gain.value = isMuted ? 0 : volume;
      src.connect(analyser);
      analyser.connect(gainNode);
      gainNode.connect(audioCtx.destination);
      audioEl.volume = 1;
      sourceConnected = true;
      smoothed = new Float32Array(analyser.frequencyBinCount);
      freqData = new Uint8Array(analyser.frequencyBinCount);
      waveData = new Uint8Array(analyser.fftSize);
      startVisualizer();
    } catch (e) {
      console.error('Audio context setup failed:', e);
    }
  }

  // ── Visualizer ──────────────────────────────────────────────────────────────

  function startVisualizer() {
    if (animId) cancelAnimationFrame(animId);
    drawFrame();
  }

  // Idle pulse clock — runs even before audio starts so the circle is visible
  let idleClock = 0;

  function drawFrame() {
    animId = requestAnimationFrame(drawFrame);
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    idleClock += 0.018;

    // If no analyser yet, draw an idle state so the user sees the visualizer
    if (!analyser) {
      const dpr = window.devicePixelRatio || 1;
      const W = canvas.width / dpr;
      const H = canvas.height / dpr;
      const cx = W / 2;
      const cy = H / 2;
      const innerR = Math.min(W, H) * 0.19;
      const pulse = 0.5 + 0.5 * Math.sin(idleClock);

      ctx.clearRect(0, 0, W, H);

      // Pulsing ring
      ctx.save();
      ctx.shadowBlur = 18 + pulse * 14;
      ctx.shadowColor = `hsla(210, 90%, 65%, ${0.25 + pulse * 0.2})`;
      ctx.strokeStyle = `hsla(210, 90%, 65%, ${0.2 + pulse * 0.15})`;
      ctx.lineWidth = 1.5;
      ctx.beginPath();
      ctx.arc(cx, cy, innerR, 0, Math.PI * 2);
      ctx.stroke();
      ctx.restore();

      // Inner circle fill
      const fill = ctx.createRadialGradient(cx, cy, 0, cx, cy, innerR);
      fill.addColorStop(0, 'hsla(215, 30%, 12%, 1)');
      fill.addColorStop(1, 'hsla(215, 20%, 7%, 1)');
      ctx.save();
      ctx.beginPath();
      ctx.arc(cx, cy, innerR, 0, Math.PI * 2);
      ctx.fillStyle = fill;
      ctx.fill();
      ctx.restore();
      return;
    }

    const dpr = window.devicePixelRatio || 1;
    const W = canvas.width / dpr;
    const H = canvas.height / dpr;
    const cx = W / 2;
    const cy = H / 2;

    // Read frequency data into reusable buffer
    const bins = freqData.length; // frequencyBinCount = fftSize / 2
    analyser.getByteFrequencyData(freqData);

    // Exponential smoothing
    const lerpFactor = isPlaying ? 0.14 : 0.06;
    for (let i = 0; i < bins; i++) {
      smoothed[i] += (freqData[i] - smoothed[i]) * lerpFactor;
    }

    // Bass energy (first 8 bins ≈ sub-bass & bass)
    let bassSum = 0;
    for (let i = 0; i < 8; i++) bassSum += smoothed[i];
    const targetBass = (bassSum / 8) / 255;
    bassEnergy += (targetBass - bassEnergy) * 0.1;

    // ── Clear ──
    ctx.clearRect(0, 0, W, H);

    // ── Background radial pulse (bass-driven) ──
    const bgRadius = Math.min(W, H) * (0.38 + bassEnergy * 0.12);
    const bgGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, bgRadius);
    bgGrad.addColorStop(0, `hsla(220, 80%, 8%, ${0.25 + bassEnergy * 0.35})`);
    bgGrad.addColorStop(1, 'hsla(220, 80%, 4%, 0)');
    ctx.fillStyle = bgGrad;
    ctx.fillRect(0, 0, W, H);

    // ── Frequency bars ──
    const numBars = bins * 2; // full 360° mirrored
    const innerR = Math.min(W, H) * 0.19;
    const maxBarH = Math.min(W, H) * 0.28;
    const barAngle = (Math.PI * 2) / numBars;

    for (let i = 0; i < numBars; i++) {
      // Mirror: first half maps 0→N forward, second half maps N→0 backward
      const binIdx = i < bins ? i : numBars - 1 - i;
      const norm = Math.min(smoothed[binIdx] / 255, 1);
      const barH = norm * maxBarH;

      const angle = i * barAngle - Math.PI / 2;

      // Color: bass = warm amber, mids = electric blue, highs = icy cyan
      const freqFrac = binIdx / bins; // 0 (bass) → 1 (treble)
      let hue: number;
      if (freqFrac < 0.15) {
        hue = 30 + freqFrac * (200 / 0.15); // amber → blue
      } else if (freqFrac < 0.6) {
        hue = 210 + ((freqFrac - 0.15) / 0.45) * 50; // blue → cyan
      } else {
        hue = 185 + ((freqFrac - 0.6) / 0.4) * 40; // cyan → ice
      }
      const sat = 85 - norm * 15;
      const lit = 45 + norm * 40;
      const alpha = 0.35 + norm * 0.65;

      const x1 = cx + Math.cos(angle) * innerR;
      const y1 = cy + Math.sin(angle) * innerR;
      const x2 = cx + Math.cos(angle) * (innerR + barH);
      const y2 = cy + Math.sin(angle) * (innerR + barH);

      ctx.save();
      ctx.lineWidth = Math.max(1.5, (Math.PI * 2 * innerR) / numBars - 0.5);
      ctx.lineCap = 'round';
      ctx.strokeStyle = `hsla(${hue}, ${sat}%, ${lit}%, ${alpha})`;
      ctx.shadowBlur = 6 + norm * 18;
      ctx.shadowColor = `hsla(${hue}, 90%, 70%, ${norm * 0.8})`;
      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.stroke();
      ctx.restore();
    }

    // ── Outer glow ring at bar base ──
    ctx.save();
    ctx.shadowBlur = 18 + bassEnergy * 30;
    ctx.shadowColor = `hsla(210, 90%, 65%, ${0.3 + bassEnergy * 0.4})`;
    ctx.strokeStyle = `hsla(210, 90%, 65%, ${0.15 + bassEnergy * 0.2})`;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(cx, cy, innerR, 0, Math.PI * 2);
    ctx.stroke();
    ctx.restore();

    // ── Inner circle fill ──
    const innerFill = ctx.createRadialGradient(cx, cy, 0, cx, cy, innerR);
    innerFill.addColorStop(0, `hsla(215, 30%, 12%, 1)`);
    innerFill.addColorStop(0.7, `hsla(215, 25%, 9%, 1)`);
    innerFill.addColorStop(1, `hsla(215, 20%, 7%, 1)`);
    ctx.save();
    ctx.beginPath();
    ctx.arc(cx, cy, innerR, 0, Math.PI * 2);
    ctx.fillStyle = innerFill;
    ctx.fill();
    ctx.restore();

    // ── Waveform inside inner circle ──
    if (analyser) {
      analyser.getByteTimeDomainData(waveData);
      const waveSlice = waveData.subarray(0, 128); // zero-copy view
      const waveR = innerR * 0.65;
      ctx.save();
      ctx.strokeStyle = `hsla(200, 80%, 75%, 0.5)`;
      ctx.lineWidth = 1.5;
      ctx.shadowBlur = 8;
      ctx.shadowColor = 'hsla(200, 80%, 75%, 0.6)';
      ctx.beginPath();
      for (let i = 0; i < waveSlice.length; i++) {
        const norm = waveSlice[i] / 128 - 1; // -1 to 1
        const angle = (i / waveSlice.length) * Math.PI * 2 - Math.PI / 2;
        const r = waveR + norm * waveR * 0.3;
        const x = cx + Math.cos(angle) * r;
        const y = cy + Math.sin(angle) * r;
        i === 0 ? ctx.moveTo(x, y) : ctx.lineTo(x, y);
      }
      ctx.closePath();
      ctx.stroke();
      ctx.restore();
    }
  }

  // ── Playback controls ───────────────────────────────────────────────────────

  async function togglePlay() {
    if (!audioEl) return;
    if (isPlaying) {
      audioEl.pause();
    } else {
      setupAudio(); // must run first — creates audioCtx
      if (audioCtx?.state === 'suspended') await audioCtx.resume();
      audioEl.play();
    }
  }

  function applyGain() {
    if (gainNode) gainNode.gain.value = isMuted ? 0 : volume;
    appSettings.updateVolume(volume);
    appSettings.updateMuted(isMuted);
  }

  function setVolume(v: number) {
    volume = Math.min(2, Math.max(0, v));
    applyGain();
  }

  function toggleMute() {
    isMuted = !isMuted;
    applyGain();
  }

  function seek(t: number) {
    if (!audioEl) return;
    audioEl.currentTime = Math.max(0, Math.min(duration, t));
  }

  // ── Canvas sizing ───────────────────────────────────────────────────────────

  function resizeCanvas() {
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = window.innerWidth * dpr;
    canvas.height = window.innerHeight * dpr;
    const ctx = canvas.getContext('2d');
    if (ctx) ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  }

  // ── Keyboard shortcuts ──────────────────────────────────────────────────────

  function handleKey(e: KeyboardEvent) {
    const active = document.activeElement;
    if (
      active instanceof HTMLElement &&
      active !== document.body &&
      active.matches('input, textarea, select, button, [contenteditable="true"], [role="slider"]')
    ) return;

    // Close app
    if (e.key === 'Escape') {
      e.preventDefault();
      closeApp();
      return;
    }

    // Back to library
    if (e.key === 'Backspace') {
      e.preventDefault();
      goBack();
      return;
    }

    // 0–9: seek to that tenth of the track
    if (e.key >= '0' && e.key <= '9') {
      e.preventDefault();
      seek(parseInt(e.key) * 0.1 * duration);
      return;
    }

    switch (e.key) {
      case ' ':
      case 'k':
        e.preventDefault();
        togglePlay();
        break;
      case 'ArrowLeft':
        e.preventDefault();
        seek(currentTime - 5);
        break;
      case 'ArrowRight':
        e.preventDefault();
        seek(currentTime + 5);
        break;
      case 'ArrowUp':
        e.preventDefault();
        setVolume(volume + 0.1);
        break;
      case 'ArrowDown':
        e.preventDefault();
        setVolume(volume - 0.1);
        break;
      case 'm':
        toggleMute();
        break;
    }
  }

  // Progress bar keyboard handler (when bar is focused)
  function handleProgressKeydown(e: KeyboardEvent) {
    if (!duration) return;
    let handled = false;
    let newTime = currentTime;
    const step = duration * 0.01; // 1% of track
    switch (e.key) {
      case 'ArrowLeft':  newTime = Math.max(0, currentTime - step);    handled = true; break;
      case 'ArrowRight': newTime = Math.min(duration, currentTime + step); handled = true; break;
      case 'Home':       newTime = 0;        handled = true; break;
      case 'End':        newTime = duration; handled = true; break;
    }
    if (handled) {
      e.preventDefault();
      seek(newTime);
    }
  }

  function goBack() {
    if (isPlaying) audioEl?.pause();
    goto('/');
  }

  // ── Controls visibility ─────────────────────────────────────────────────────

  function showControls() {
    controlsVisible = true;
    clearTimeout(hideControlsTimer);
    hideControlsTimer = setTimeout(() => {
      if (isPlaying) controlsVisible = false;
    }, 2500);
  }

  function handleMouseMove() {
    showControls();
    showCloseBtn = true;
    clearTimeout(hideCloseBtnTimer);
    hideCloseBtnTimer = setTimeout(() => { showCloseBtn = false; }, 1200);
  }

  // ── Watch progress ──────────────────────────────────────────────────────────

  function saveProgress() {
    // Read live position from the element so scrubs that haven't fired a
    // timeupdate yet are still captured correctly.
    const pos = audioEl?.currentTime ?? currentTime;
    if (duration > 0 && pos > 2) {
      watchProgressStore.setProgress(audioPath, {
        path: audioPath,
        current_time: pos,
        duration,
        last_watched: Math.floor(Date.now() / 1000)
      });
      invoke('save_watch_progress', {
        videoPath: audioPath,
        currentTime: pos,
        duration
      }).catch(console.error);
    }
  }

  // ── Lifecycle ───────────────────────────────────────────────────────────────

  onMount(() => {
    resizeCanvas();
    window.addEventListener('resize', resizeCanvas);
    window.addEventListener('keydown', handleKey);

    // Restore saved progress — invoke the backend directly so this works on
    // first load when watchProgressStore hasn't been populated yet.
    invoke<{ current_time: number; duration: number } | null>('get_watch_progress', {
      videoPath: audioPath,
    }).then((progress) => {
      if (progress && audioEl && progress.duration > 0) {
        const pct = progress.current_time / progress.duration;
        if (pct > 0.05 && pct < 0.95) {
          audioEl.currentTime = progress.current_time;
        }
      }
    }).catch(() => {
      // Fall back to in-memory store if the invoke fails
      const saved = $watchProgressStore.get(audioPath);
      if (saved && saved.current_time > 5) {
        audioEl.currentTime = saved.current_time;
      }
    });

    // Restore volume from store
    volume = $appSettings.volume ?? 1;
    isMuted = $appSettings.isMuted ?? false;

    // Draw idle frame immediately
    if (!analyser) {
      startVisualizer();
    }

    progressSaveInterval = setInterval(() => {
      if (isPlaying && duration > 0) saveProgress();
    }, 5000);

    return () => {
      window.removeEventListener('resize', resizeCanvas);
      window.removeEventListener('keydown', handleKey);
      clearInterval(progressSaveInterval);
    };
  });

  onDestroy(() => {
    cancelAnimationFrame(animId);
    audioCtx?.close();
    clearTimeout(hideCloseBtnTimer);
    clearTimeout(hideControlsTimer);
    clearInterval(progressSaveInterval);
    saveProgress();
  });

  async function closeApp() {
    try {
      const { exit } = await import('@tauri-apps/plugin-process');
      await exit(0);
    } catch {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      (await getCurrentWindow()).close();
    }
  }

  // Scrubber helpers
  function onScrubStart(e: MouseEvent) {
    isScrubbing = true;
    updateScrub(e);
  }
  function onScrubMove(e: MouseEvent) {
    if (isScrubbing) updateScrub(e);
  }
  function onScrubEnd() {
    if (isScrubbing) {
      seek(scrubValue);
      isScrubbing = false;
    }
  }
  function updateScrub(e: MouseEvent) {
    const bar = (e.currentTarget as HTMLElement);
    const rect = bar.getBoundingClientRect();
    scrubValue = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width)) * duration;
  }

</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
  class="root"
  onmousemove={handleMouseMove}
  onmouseleave={() => { showCloseBtn = false; }}
>
  <!-- Close button -->
  <button class="close-btn" class:visible={showCloseBtn} onclick={closeApp} title="Close (Esc)">
    <X size={16} />
  </button>

  <!-- Visualizer fills the entire screen -->
  <div class="viz-area" bind:this={containerEl}>
    <canvas bind:this={canvas}></canvas>

    <!-- Song title overlay (inside inner circle) -->
    <div class="title-overlay">
      <p class="song-name">{displayName}</p>
    </div>
  </div>

  <!-- Controls zone — overlaid at the bottom, identical structure to video player -->
  <div class="controls-zone">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="controls" class:visible={controlsVisible} onmouseenter={showControls}>

      <!-- Progress bar -->
      <div
        class="progress-bar"
        class:scrubbing={isScrubbing}
        onmousedown={onScrubStart}
        onmousemove={onScrubMove}
        onmouseup={onScrubEnd}
        onmouseleave={onScrubEnd}
        onkeydown={handleProgressKeydown}
        role="slider"
        aria-label="Audio progress"
        aria-valuemin={0}
        aria-valuemax={duration}
        aria-valuenow={isScrubbing ? scrubValue : currentTime}
        tabindex="0"
      >
        <div
          class="progress-filled"
          style="width: {duration ? Math.min(100, Math.max(0, ((isScrubbing ? scrubValue : currentTime) / duration) * 100)) : 0}%"
        >
          <div class="progress-handle"></div>
        </div>
      </div>

      <!-- Controls row -->
      <div class="controls-row">
        <div class="controls-left">
          <button class="control-button" onclick={goBack} title="Back to library">
            <Home size={20} />
          </button>
          <div class="time">
            {formatDuration(isScrubbing ? scrubValue : currentTime)} / {formatDuration(duration)}
          </div>
        </div>

        <div class="controls-center">
          <button class="control-button" onclick={togglePlay} title="Play/Pause (Space)">
            {#if isPlaying}
              <Pause size={24} fill="currentColor" />
            {:else}
              <Play size={24} fill="currentColor" />
            {/if}
          </button>
        </div>

        <div class="controls-right">
          <div class="volume-control">
            <button class="control-button" onclick={() => showVolumeMenu = !showVolumeMenu} title="Volume">
              {#if isMuted || volume === 0}
                <VolumeX size={20} />
              {:else if volume < 1}
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
                  max="2"
                  step="0.01"
                  aria-label="Volume"
                  bind:value={volume}
                  oninput={(e) => setVolume((e.target as HTMLInputElement).valueAsNumber)}
                />
                <span class="volume-percent">{Math.round((isMuted ? 0 : volume) * 100)}%</span>
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
        </div>
      </div>

    </div>
  </div>
</div>

<!-- Hidden audio element — crossorigin required for Web Audio API (CORS) -->
<audio
  bind:this={audioEl}
  src={convertFileSrc(audioPath)}
  crossorigin="anonymous"
  onplay={() => { isPlaying = true; audioCtx?.resume(); showControls(); }}
  onpause={() => { isPlaying = false; saveProgress(); }}
  ontimeupdate={() => { if (!isScrubbing) currentTime = audioEl.currentTime; }}
  onloadedmetadata={() => {
    duration = audioEl.duration;
    setupAudio();
    audioEl.play().catch((err) => console.log('Auto-play prevented:', err));
  }}
  onended={() => { isPlaying = false; saveProgress(); }}
  preload="metadata"
></audio>

<style>
  :global(body) {
    margin: 0;
    background: #080a10;
    overflow: hidden;
    user-select: none;
  }

  .root {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: #080a10;
    font-family: system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.9);
    cursor: default;
  }

  /* ── Close button ─────────────────────────────────── */
  .close-btn {
    position: fixed;
    top: 12px;
    right: 12px;
    z-index: 100;
    width: 28px;
    height: 28px;
    padding: 0;
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 6px;
    color: rgba(255, 255, 255, 0.6);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
    opacity: 0;
    transition: opacity 0.25s ease, color 0.15s ease, background 0.15s ease;
    pointer-events: none;
  }
  .close-btn.visible {
    opacity: 1;
    pointer-events: auto;
  }
  .close-btn:hover {
    background: rgba(255, 255, 255, 0.14);
    color: #fff;
  }

  /* ── Visualizer area ──────────────────────────────── */
  .viz-area {
    position: absolute;
    inset: 0;
    overflow: hidden;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  .title-overlay {
    position: relative;
    z-index: 10;
    text-align: center;
    pointer-events: none;
  }

  .song-name {
    margin: 0;
    font-size: clamp(0.75rem, 1.5vw, 1rem);
    font-weight: 400;
    color: rgba(255, 255, 255, 0.6);
    letter-spacing: 0.03em;
    max-width: 18vw;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    text-shadow: 0 0 20px rgba(120, 180, 255, 0.4);
  }

  /* ── Controls zone + controls (matches video player cinematic/overlay style) ── */
  .controls-zone {
    position: absolute;
    bottom: 0;
    left: 0;
    right: 0;
    z-index: 10;
    pointer-events: none;
  }

  .controls {
    padding: 2rem 1.5rem 1.5rem;
    background: linear-gradient(
      to top,
      rgba(0, 0, 0, 0.85) 0%,
      rgba(0, 0, 0, 0.6) 60%,
      transparent 100%
    );
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.25s ease;
  }

  .controls.visible {
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

  .volume-percent {
    font-size: 0.7rem;
    font-variant-numeric: tabular-nums;
    color: rgba(255, 255, 255, 0.6);
    letter-spacing: 0.02em;
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
</style>
