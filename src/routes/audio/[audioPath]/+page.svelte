<script lang="ts">
  import { onMount, onDestroy, untrack } from 'svelte';
  import { goto } from '$app/navigation';
  import { convertFileSrc } from '@tauri-apps/api/core';
  import { invoke } from '@tauri-apps/api/core';
  import { appSettings } from '$lib/stores/appStore';
  import { watchProgressStore } from '$lib/stores/watchProgressStore';
  import { createFadedMediaPlayback } from '$lib/utils/fadedMediaPlayback';
  import { formatDuration } from '$lib/utils/time';
  import { X, Play, Pause, Volume1, Volume2, VolumeX, Home } from 'lucide-svelte';

  let { data } = $props();
  const audioPath: string = $derived(data.audioPath);
  const fileName = $derived(audioPath.split(/[\\/]/).pop() ?? audioPath);
  const displayName = $derived(fileName.replace(/\.[^.]+$/, ''));

  // DOM refs
  let audioEl: HTMLAudioElement;
  let canvas: HTMLCanvasElement;

  // Web Audio API
  let audioCtx: AudioContext | null = null;
  let analyser: AnalyserNode | null = null;
  let gainNode: GainNode | null = null;
  let sourceConnected = false;

  // Visualizer state
  let animId: number;
  let smoothed: Float32Array = new Float32Array(512);
  let freqData = new Uint8Array(new ArrayBuffer(0));
  let waveData = new Uint8Array(new ArrayBuffer(0));

  // Cinematic Engine State
  let bassEnergy = 0;
  let midEnergy = 0;
  let trebleEnergy = 0;
  let globalRotation = 0;
  let pumpScale = 1;
  let hueOffset = 210;
  let idleClock = 0;

  type Particle = { angle: number; radius: number; speed: number; length: number; life: number; maxLife: number; hue: number; alpha: number; thickness: number; };
  const MAX_PARTICLES = 200; // Optimized count
  let particles: Particle[] = [];

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
  let pendingRestoreTime: number | null = null;
  let progressRestorePromise: Promise<void> = Promise.resolve();
  let currentLoadId = 0;

  // UI
  let showCloseBtn = $state(false);
  let hideCloseBtnTimer: ReturnType<typeof setTimeout>;
  let controlsVisible = $state(true);
  let hideControlsTimer: ReturnType<typeof setTimeout>;
  let controlsEl: HTMLElement;
  let volumeMenuAutoTimer: ReturnType<typeof setTimeout>;
  let previousAudioPath = '';

  function getAudioOutputVolume() {
    if (gainNode) return gainNode.gain.value;
    return audioEl?.volume ?? 0;
  }

  function setAudioOutputVolume(value: number) {
    const safeValue = Math.max(0, value);
    if (gainNode) {
      gainNode.gain.value = safeValue;
    } else if (audioEl) {
      audioEl.volume = Math.min(1, safeValue);
    }
  }

  const fadedPlayback = createFadedMediaPlayback({
    getMediaElement: () => audioEl,
    getTargetVolume: () => (isMuted ? 0 : volume),
    getOutputVolume: getAudioOutputVolume,
    setOutputVolume: setAudioOutputVolume,
    onPlayingChange: (playing) => {
      isPlaying = playing;
    },
  });

  // ── Audio context setup ─────────────────────────────────────────────────────

  function setupAudio() {
    if (!audioEl || sourceConnected) return;
    try {
      audioCtx = new AudioContext();
      const src = audioCtx.createMediaElementSource(audioEl);
      analyser = audioCtx.createAnalyser();

      analyser.fftSize = 1024; // Balanced resolution for performance and visuals
      analyser.smoothingTimeConstant = 0.8;

      gainNode = audioCtx.createGain();
      gainNode.gain.value = isPlaying && !isMuted ? volume : 0;

      src.connect(analyser);
      analyser.connect(gainNode);
      gainNode.connect(audioCtx.destination);
      audioEl.volume = 1;
      audioEl.muted = isMuted;
      sourceConnected = true;

      smoothed = new Float32Array(analyser.frequencyBinCount);
      freqData = new Uint8Array(analyser.frequencyBinCount);
      waveData = new Uint8Array(analyser.fftSize);
      startVisualizer();
    } catch (e) {
      console.error('Audio context setup failed:', e);
      if (gainNode) gainNode.disconnect();
      if (analyser) analyser.disconnect();
      gainNode = null;
      analyser = null;
      smoothed = new Float32Array();
      freqData = new Uint8Array();
      waveData = new Uint8Array();
      if (audioCtx) audioCtx.close().catch(() => {});
      audioCtx = null;
    }
  }

  // ── Cinematic 2D Visualizer ──────────────────────────────────────────────────

  function startVisualizer() {
    if (animId) cancelAnimationFrame(animId);
    drawFrame();
  }

  function drawFrame() {
    animId = requestAnimationFrame(drawFrame);
    if (!canvas) return;

    const ctx = canvas.getContext('2d');
    if (!ctx) return;

    const dpr = window.devicePixelRatio || 1;
    const W = canvas.width / dpr;
    const H = canvas.height / dpr;
    const cx = W / 2;
    const cy = H / 2;
    const innerR = Math.min(W, H) * 0.18;

    idleClock += 0.01;
    hueOffset += 0.15; // Slow ambient color drift

    // ── IDLE / NO AUDIO STATE ──
    if (!analyser) {
      ctx.clearRect(0, 0, W, H);
      const pulse = 0.5 + 0.5 * Math.sin(idleClock * 2);

      const bgGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, W);
      bgGrad.addColorStop(0, `hsla(${hueOffset}, 40%, 10%, ${0.3 + pulse * 0.2})`);
      bgGrad.addColorStop(1, '#05070a');
      ctx.fillStyle = bgGrad;
      ctx.fillRect(0, 0, W, H);

      ctx.save();
      ctx.translate(cx, cy);
      // Reduced idle glow blur to save frames
      ctx.shadowBlur = 15 + pulse * 10;
      ctx.shadowColor = `hsla(${hueOffset}, 80%, 60%, ${0.3 + pulse * 0.3})`;
      ctx.strokeStyle = `hsla(${hueOffset}, 80%, 60%, ${0.2 + pulse * 0.2})`;
      ctx.lineWidth = 2;
      ctx.beginPath();
      ctx.arc(0, 0, innerR, 0, Math.PI * 2);
      ctx.stroke();
      ctx.restore();
      return;
    }

    // ── AUDIO ANALYSIS ──
    const bins = freqData.length;
    analyser.getByteFrequencyData(freqData);

    const lerpFactor = isPlaying ? 0.25 : 0.05;
    let bSum = 0, mSum = 0, tSum = 0;

    for (let i = 0; i < bins; i++) {
      smoothed[i] += (freqData[i] - smoothed[i]) * lerpFactor;
      if (i < 10) bSum += smoothed[i];
      else if (i < 60) mSum += smoothed[i];
      else if (i < 200) tSum += smoothed[i];
    }

    const currentBass = (bSum / 10) / 255;
    const currentMid = (mSum / 50) / 255;
    const currentTreble = (tSum / 140) / 255;

    bassEnergy += (currentBass - bassEnergy) * 0.2;
    midEnergy += (currentMid - midEnergy) * 0.2;
    trebleEnergy += (currentTreble - trebleEnergy) * 0.2;

    // ── CLEAR & AMBIENT GLOW ──
    ctx.fillStyle = '#05070a';
    ctx.fillRect(0, 0, W, H);

    const bgRadius = Math.max(W, H) * (0.5 + bassEnergy * 0.4);
    const bgGrad = ctx.createRadialGradient(cx, cy, 0, cx, cy, bgRadius);
    bgGrad.addColorStop(0, `hsla(${hueOffset}, 70%, 15%, ${0.3 + bassEnergy * 0.5})`);
    bgGrad.addColorStop(1, 'hsla(220, 80%, 4%, 0)');
    ctx.fillStyle = bgGrad;
    ctx.fillRect(0, 0, W, H);

    // ── CINEMATIC TRANSFORM (PUMP & ROTATION) ──
    globalRotation += 0.0005 + (bassEnergy * 0.005);
    const targetPump = 1 + (bassEnergy * 0.12) + (midEnergy * 0.03);
    pumpScale += (targetPump - pumpScale) * 0.2;

    ctx.save();
    ctx.translate(cx, cy);
    ctx.scale(pumpScale, pumpScale);
    ctx.rotate(globalRotation);

    // ── PARTICLE ENGINE (2D OUTWARD) ──
    if (isPlaying && (bassEnergy > 0.5 || trebleEnergy > 0.4) && Math.random() > 0.3) {
      let numToSpawn = Math.floor(bassEnergy * 4 + trebleEnergy * 2);
      for (let i = 0; i < numToSpawn; i++) {
        const maxLife = 30 + Math.random() * 50;
        particles.push({
          angle: Math.random() * Math.PI * 2,
          radius: innerR + Math.random() * 20,
          speed: 2 + Math.random() * 5 + (bassEnergy * 8),
          length: 5 + Math.random() * 25,
          life: maxLife,
          maxLife,
          hue: hueOffset + Math.random() * 60 - 30,
          alpha: 0.5 + Math.random() * 0.5,
          thickness: 1.5 + Math.random() * 2
        });
      }
    }
    if (particles.length > MAX_PARTICLES) particles.splice(0, particles.length - MAX_PARTICLES);

    ctx.globalCompositeOperation = 'screen';
    ctx.lineCap = 'round';
    for (let i = particles.length - 1; i >= 0; i--) {
      let p = particles[i];
      p.radius += p.speed;
      p.life--;

      if (p.life <= 0) {
        particles.splice(i, 1);
        continue;
      }

      const lifePct = p.life / p.maxLife;
      const x1 = Math.cos(p.angle) * p.radius;
      const y1 = Math.sin(p.angle) * p.radius;
      const x2 = Math.cos(p.angle) * (p.radius - p.length * lifePct);
      const y2 = Math.sin(p.angle) * (p.radius - p.length * lifePct);

      ctx.beginPath();
      ctx.moveTo(x1, y1);
      ctx.lineTo(x2, y2);
      ctx.strokeStyle = `hsla(${p.hue}, 90%, 70%, ${p.alpha * lifePct})`;
      ctx.lineWidth = p.thickness;
      ctx.stroke();
    }
    ctx.globalCompositeOperation = 'source-over';

    // ── SYMMETRICAL FREQUENCY SPECTRUM ──
    const activeBins = 180;
    const barAngle = Math.PI / activeBins;
    const maxBarH = Math.min(W, H) * 0.35;

    // Optimized glowing effect: We use slightly lower shadowBlur to heavily reduce GPU cost
    for (let i = 0; i < activeBins; i++) {
      const norm = Math.min(smoothed[i] / 255, 1);
      const h = norm * norm * maxBarH * 1.2;
      if (h < 1) continue;

      const binHue = hueOffset + (i / activeBins) * 90;
      ctx.strokeStyle = `hsla(${binHue}, 85%, 60%, ${0.3 + norm * 0.7})`;
      ctx.shadowBlur = 4 + norm * 8; // Perf fix: Kept blur tight and constrained
      ctx.shadowColor = `hsla(${binHue}, 90%, 65%, ${norm})`;
      ctx.lineWidth = Math.max(2, ((Math.PI * innerR) / activeBins) * 1.5);

      // Right side (forward)
      let angleR = i * barAngle - Math.PI / 2;
      ctx.beginPath();
      ctx.moveTo(Math.cos(angleR) * innerR, Math.sin(angleR) * innerR);
      ctx.lineTo(Math.cos(angleR) * (innerR + h), Math.sin(angleR) * (innerR + h));
      ctx.stroke();

      // Left side (mirrored) — skip i === 0 to avoid doubling the top-center bar
      if (i > 0) {
        let angleL = -i * barAngle - Math.PI / 2;
        ctx.beginPath();
        ctx.moveTo(Math.cos(angleL) * innerR, Math.sin(angleL) * innerR);
        ctx.lineTo(Math.cos(angleL) * (innerR + h), Math.sin(angleL) * (innerR + h));
        ctx.stroke();
      }
    }
    ctx.shadowBlur = 0; // Reset blur

    // ── CHROMATIC ABERRATION WAVEFORM (CORE) ──
    analyser.getByteTimeDomainData(waveData);

    const drawWave = (offset: number, color: string) => {
      ctx.beginPath();
      // Step by 2 to halve path calculation cost without noticeably affecting visuals
      for (let i = 0; i < waveData.length; i += 2) {
        const norm = waveData[i] / 128 - 1;
        const r = (innerR * 0.85) + (norm * innerR * 0.35) + offset;
        const angle = (i / waveData.length) * Math.PI * 2 - Math.PI / 2;

        const localAngle = angle - globalRotation * 2;
        const x = Math.cos(localAngle) * r;
        const y = Math.sin(localAngle) * r;

        if (i === 0) ctx.moveTo(x, y);
        else ctx.lineTo(x, y);
      }
      ctx.closePath();
      ctx.strokeStyle = color;
      ctx.stroke();
    };

    ctx.globalCompositeOperation = 'screen';
    ctx.lineWidth = 2.5;

    // Core splits apart into RGB channels on heavy bass
    const aberration = bassEnergy * 25;

    drawWave(aberration, `rgba(255, 40, 80, ${0.4 + bassEnergy * 0.5})`);
    drawWave(-aberration, `rgba(40, 200, 255, ${0.4 + bassEnergy * 0.5})`);

    ctx.lineWidth = 3;
    drawWave(0, `rgba(220, 240, 255, 0.9)`);

    ctx.globalCompositeOperation = 'source-over';

    // ── INNER DARK VOID (Contrast backing for title) ──
    const innerFill = ctx.createRadialGradient(0, 0, 0, 0, 0, innerR * 0.8);
    innerFill.addColorStop(0, `rgba(5, 7, 10, 0.95)`);
    innerFill.addColorStop(1, `rgba(5, 7, 10, 0.7)`);
    ctx.beginPath();
    ctx.arc(0, 0, innerR * 0.75, 0, Math.PI * 2);
    ctx.fillStyle = innerFill;
    ctx.fill();

    ctx.restore(); // Restore master transform
  }

  // ── Playback controls ───────────────────────────────────────────────────────

  async function togglePlay() {
    if (!audioEl) return;
    if (isPlaying) {
      await fadedPlayback.pause();
    } else {
      setupAudio(); // must run first — creates audioCtx
      if (audioCtx?.state === 'suspended') await audioCtx.resume();
      try {
        await fadedPlayback.play();
      } catch (err) {
        console.log('Play prevented:', err);
      }
    }
  }

  function applyGain() {
    if (audioEl) {
      audioEl.muted = isMuted;
    }
    fadedPlayback.syncOutputVolume();
    appSettings.updateVolume(volume);
    appSettings.updateMuted(isMuted);
  }

  function flashVolumeMenu() {
    showVolumeMenu = true;
    clearTimeout(volumeMenuAutoTimer);
    volumeMenuAutoTimer = setTimeout(() => { showVolumeMenu = false; }, 1500);
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
    const clamped = Math.max(0, Math.min(duration, t));
    audioEl.currentTime = clamped;
    currentTime = clamped;
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

    if (e.key === 'Escape') {
      e.preventDefault();
      goBack();
      return;
    }

    if (e.key === 'Backspace') {
      e.preventDefault();
      goBack();
      return;
    }

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
        flashVolumeMenu();
        break;
      case 'ArrowDown':
        e.preventDefault();
        setVolume(volume - 0.1);
        flashVolumeMenu();
        break;
      case 'm':
        toggleMute();
        flashVolumeMenu();
        break;
    }
  }

  function handleProgressKeydown(e: KeyboardEvent) {
    if (!duration) return;
    let handled = false;
    let newTime = currentTime;
    const step = duration * 0.01;
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

  async function goBack() {
    if (isPlaying) await fadedPlayback.pause();
    goto('/');
  }

  // ── Controls visibility ─────────────────────────────────────────────────────

  function showControls() {
    controlsVisible = true;
    clearTimeout(hideControlsTimer);
    if (!showVolumeMenu && !controlsEl?.matches(':focus-within') && !controlsEl?.matches(':hover')) {
      hideControlsTimer = setTimeout(() => {
        if (isPlaying) controlsVisible = false;
      }, 2500);
    }
  }

  $effect(() => {
    if (!showVolumeMenu) showControls();
  });

  function handleMouseMove() {
    showControls();
    showCloseBtn = true;
    clearTimeout(hideCloseBtnTimer);
    hideCloseBtnTimer = setTimeout(() => { showCloseBtn = false; }, 1200);
  }

  // ── Watch progress ──────────────────────────────────────────────────────────

  function saveProgress() {
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

  $effect(() => {
    const path = audioPath;
    let cancelled = false;

    untrack(() => {
      const outgoingPath = previousAudioPath;
      const outgoingDuration = duration;
      const outgoingPos = audioEl?.currentTime ?? currentTime;
      if (outgoingPath && outgoingDuration > 0 && outgoingPos > 2) {
        watchProgressStore.setProgress(outgoingPath, {
          path: outgoingPath,
          current_time: outgoingPos,
          duration: outgoingDuration,
          last_watched: Math.floor(Date.now() / 1000)
        });
        invoke('save_watch_progress', {
          videoPath: outgoingPath,
          currentTime: outgoingPos,
          duration: outgoingDuration
        }).catch(console.error);
      }
      previousAudioPath = path;
    });

    pendingRestoreTime = null;
    currentTime = 0;
    duration = 0;

    progressRestorePromise = new Promise<void>((resolve) => {
      invoke<{ current_time: number; duration: number } | null>('get_watch_progress', {
        videoPath: path,
      }).then((progress) => {
        if (!cancelled && progress && progress.duration > 0) {
          const pct = progress.current_time / progress.duration;
          if (pct > 0.05 && pct < 0.95) {
            pendingRestoreTime = progress.current_time;
            if (audioEl && audioEl.readyState >= 1) {
              audioEl.currentTime = pendingRestoreTime;
            }
          }
        }
        resolve();
      }).catch(() => {
        if (!cancelled) {
          const saved = $watchProgressStore.get(path);
          if (saved && saved.current_time > 5) {
            pendingRestoreTime = saved.current_time;
            if (audioEl && audioEl.readyState >= 1) {
              audioEl.currentTime = pendingRestoreTime;
            }
          }
        }
        resolve();
      });
    });

    return () => { cancelled = true; };
  });

  onMount(() => {
    resizeCanvas();
    window.addEventListener('resize', resizeCanvas);
    window.addEventListener('keydown', handleKey);

    volume = $appSettings.volume ?? 1;
    isMuted = $appSettings.isMuted ?? false;

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
      fadedPlayback.destroy();
    };
  });

  onDestroy(() => {
    cancelAnimationFrame(animId);
    fadedPlayback.destroy();
    audioCtx?.close();
    clearTimeout(hideCloseBtnTimer);
    clearTimeout(hideControlsTimer);
    clearInterval(progressSaveInterval);
    saveProgress();
  });

  async function closeApp() {
    if (isPlaying) {
      await fadedPlayback.pause();
    }

    const pos = audioEl?.currentTime ?? currentTime;
    if (duration > 0 && pos > 2) {
      await invoke('save_watch_progress', {
        videoPath: audioPath,
        currentTime: pos,
        duration
      }).catch(console.error);
    }
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
        oncontextmenu={(e) => e.preventDefault()}
        ondragover={(e) => e.preventDefault()}
        ondrop={(e) => e.preventDefault()}
>
  <!-- Close button -->
  <button class="close-btn" class:visible={showCloseBtn} tabindex={showCloseBtn ? 0 : -1} onclick={closeApp} onfocus={() => { showCloseBtn = true; clearTimeout(hideCloseBtnTimer); hideCloseBtnTimer = setTimeout(() => { showCloseBtn = false; }, 1200); }} title="Close (Esc)">
    <X size={16} />
  </button>

  <!-- Visualizer fills the entire screen -->
  <div class="viz-area">
    <canvas bind:this={canvas}></canvas>

    <!-- Song title overlay (centered perfectly in the inner circle) -->
    <div class="title-overlay">
      <p class="song-name">{displayName}</p>
    </div>
  </div>

  <!-- Controls zone -->
  <div class="controls-zone">
    <!-- svelte-ignore a11y_no_static_element_interactions -->
    <div class="controls" class:visible={controlsVisible} inert={!controlsVisible} bind:this={controlsEl} onmouseenter={showControls}>

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
            <button class="control-button" onclick={() => { clearTimeout(volumeMenuAutoTimer); showVolumeMenu = !showVolumeMenu; }} title="Volume">
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
                        value={volume}
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

<audio
        bind:this={audioEl}
        src={convertFileSrc(audioPath)}
        crossorigin="anonymous"
        onplay={() => { audioCtx?.resume(); showControls(); }}
        onpause={() => { isPlaying = false; saveProgress(); }}
        ontimeupdate={() => { if (!isScrubbing) currentTime = audioEl.currentTime; }}
        onloadedmetadata={async () => {
    const myLoadId = ++currentLoadId;
    duration = audioEl.duration;
    setupAudio();
    await progressRestorePromise;
    if (myLoadId !== currentLoadId) return;
    if (pendingRestoreTime !== null) {
      audioEl.currentTime = pendingRestoreTime;
    }
    fadedPlayback.play().catch((err) => console.log('Auto-play prevented:', err));
  }}
        onended={() => { fadedPlayback.pauseNow(); saveProgress(); }}
        preload="metadata"
></audio>

<style>
  :global(body) {
    margin: 0;
    background: #05070a;
    overflow: hidden;
    user-select: none;
  }

  .root {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: #05070a;
    font-family: system-ui, sans-serif;
    color: rgba(255, 255, 255, 0.9);
    cursor: default;
  }

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
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    /* Bounded tightly to the inner core circle */
    width: calc(min(100vw, 100vh) * 0.28);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 10;
    pointer-events: none;
  }

  .song-name {
    margin: 0;
    font-size: clamp(0.75rem, 1.8vw, 1.25rem);
    font-weight: 600;
    color: rgba(255, 255, 255, 0.95);
    letter-spacing: 0.03em;
    line-height: 1.35;
    text-align: center;

    /* Multiline clamp ensuring it stays neatly inside the sphere */
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow: hidden;

    text-shadow:
            0 2px 10px rgba(0, 0, 0, 0.9),
            0 0 20px rgba(0, 0, 0, 0.8),
            0 0 35px rgba(120, 180, 255, 0.6);
  }

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
            rgba(0, 0, 0, 0.9) 0%,
            rgba(0, 0, 0, 0.6) 50%,
            transparent 100%
    );
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.3s ease;
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

  .controls-left { justify-content: flex-start; }
  .controls-center { justify-content: center; }
  .controls-right { justify-content: flex-end; }

  .control-button {
    background: none;
    border: none;
    color: #fff;
    cursor: pointer;
    padding: 0.5rem;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: opacity 0.15s ease, transform 0.15s ease;
    opacity: 0.8;
  }

  .control-button:hover {
    opacity: 1;
    transform: scale(1.05);
  }

  .time {
    font-size: 0.875rem;
    font-variant-numeric: tabular-nums;
    letter-spacing: 0.01em;
    opacity: 0.8;
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
    background: rgba(20, 24, 30, 0.85);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 1rem 0.75rem;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.6);
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

  .volume-slider-vertical::-webkit-slider-thumb:hover { transform: scale(1.2); }
  .volume-slider-vertical::-moz-range-thumb:hover { transform: scale(1.2); }

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
