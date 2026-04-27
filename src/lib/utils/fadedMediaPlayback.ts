export interface FadedMediaPlaybackOptions {
  getMediaElement: () => HTMLMediaElement | null | undefined;
  getSyncedMediaElements?: () => Array<HTMLMediaElement | null | undefined>;
  getTargetVolume: () => number;
  getOutputVolume?: () => number;
  setOutputVolume: (volume: number) => void;
  onPlayingChange?: (isPlaying: boolean) => void;
  fadeInMs?: number;
  fadeOutMs?: number;
}

export interface PlaybackTransitionOptions {
  fade?: boolean;
}

export interface StopTransitionOptions extends PlaybackTransitionOptions {
  resetTime?: boolean;
}

const DEFAULT_FADE_IN_MS = 800;
const DEFAULT_FADE_OUT_MS = 800;
const MIN_VOLUME_DELTA = 0.001;

function clampVolume(volume: number) {
  if (!Number.isFinite(volume)) return 0;
  return Math.max(0, volume);
}

function easeInOutCubic(progress: number) {
  return progress < 0.5
    ? 4 * progress * progress * progress
    : 1 - Math.pow(-2 * progress + 2, 3) / 2;
}

export function createFadedMediaPlayback(options: FadedMediaPlaybackOptions) {
  let animationFrame: number | null = null;
  let transitionId = 0;
  let requestedPlaying = false;
  let activeFadeTarget: number | null = null;
  // Resolver for the currently in-flight fadeTo() Promise (if any).
  // cancelFade() must call this before clearing the animation frame so the
  // Promise is always settled rather than left permanently pending.
  let pendingFadeResolve: ((value: boolean) => void) | null = null;

  const fadeInMs = options.fadeInMs ?? DEFAULT_FADE_IN_MS;
  const fadeOutMs = options.fadeOutMs ?? DEFAULT_FADE_OUT_MS;

  function getSyncedMediaElements() {
    return (options.getSyncedMediaElements?.() ?? []).filter(
      (element): element is HTMLMediaElement => Boolean(element),
    );
  }

  function getTargetVolume() {
    return clampVolume(options.getTargetVolume());
  }

  function getOutputVolume() {
    return clampVolume(options.getOutputVolume?.() ?? getTargetVolume());
  }

  function setOutputVolume(volume: number) {
    options.setOutputVolume(clampVolume(volume));
  }

  function clearAnimationFrame() {
    if (animationFrame !== null) {
      cancelAnimationFrame(animationFrame);
      animationFrame = null;
    }
  }

  function cancelFade() {
    // Settle any pending fadeTo() Promise before cancelling the frame so
    // callers that await fadeTo() are never left with an unresolved Promise.
    const resolve = pendingFadeResolve;
    pendingFadeResolve = null;
    transitionId += 1;
    activeFadeTarget = null;
    clearAnimationFrame();
    resolve?.(false);
  }

  function fadeTo(targetVolume: number, durationMs: number) {
    const target = clampVolume(targetVolume);
    const start = getOutputVolume();

    // Settle any previous in-flight fade before starting a new one.
    const prevResolve = pendingFadeResolve;
    pendingFadeResolve = null;
    transitionId += 1;
    const id = transitionId;
    activeFadeTarget = target;
    clearAnimationFrame();
    prevResolve?.(false);

    if (durationMs <= 0 || Math.abs(start - target) <= MIN_VOLUME_DELTA) {
      setOutputVolume(target);
      activeFadeTarget = null;
      return Promise.resolve(true);
    }

    return new Promise<boolean>((resolve) => {
      pendingFadeResolve = resolve;
      const startedAt = performance.now();

      const step = (now: number) => {
        if (id !== transitionId) {
          // cancelFade() already resolved the Promise; nothing left to do.
          return;
        }

        const progress = Math.min(1, (now - startedAt) / durationMs);
        const eased = easeInOutCubic(progress);
        setOutputVolume(start + (target - start) * eased);

        if (progress < 1) {
          animationFrame = requestAnimationFrame(step);
          return;
        }

        animationFrame = null;
        activeFadeTarget = null;
        pendingFadeResolve = null;
        resolve(true);
      };

      animationFrame = requestAnimationFrame(step);
    });
  }

  async function play({ fade = true }: PlaybackTransitionOptions = {}) {
    const mediaElement = options.getMediaElement();
    if (!mediaElement) return;

    requestedPlaying = true;
    cancelFade();
    const playId = transitionId;
    options.onPlayingChange?.(true);

    if (mediaElement.paused && fade) {
      setOutputVolume(0);
    }

    try {
      await mediaElement.play();
      if (!requestedPlaying || playId !== transitionId) {
        return;
      }
      await Promise.allSettled(
        getSyncedMediaElements().map((element) => element.play()),
      );
    } catch (err) {
      if (playId === transitionId) {
        requestedPlaying = false;
        options.onPlayingChange?.(false);
      }
      throw err;
    }

    if (!requestedPlaying || playId !== transitionId) {
      return;
    }

    if (fade) {
      await fadeTo(getTargetVolume(), fadeInMs);
    } else {
      setOutputVolume(getTargetVolume());
    }
  }

  async function pause({ fade = true }: PlaybackTransitionOptions = {}) {
    const mediaElement = options.getMediaElement();
    if (!mediaElement) return;

    requestedPlaying = false;
    options.onPlayingChange?.(false);

    if (!mediaElement.paused && fade) {
      const completed = await fadeTo(0, fadeOutMs);
      if (!completed) return;
    } else {
      cancelFade();
      setOutputVolume(0);
    }

    mediaElement.pause();
    for (const element of getSyncedMediaElements()) {
      element.pause();
    }
  }

  function pauseNow() {
    const mediaElement = options.getMediaElement();
    if (!mediaElement) return;

    requestedPlaying = false;
    cancelFade();
    setOutputVolume(0);
    mediaElement.pause();
    for (const element of getSyncedMediaElements()) {
      element.pause();
    }
    options.onPlayingChange?.(false);
  }

  async function stop({
    fade = true,
    resetTime = true,
  }: StopTransitionOptions = {}) {
    const mediaElement = options.getMediaElement();
    if (!mediaElement) return;

    requestedPlaying = false;
    options.onPlayingChange?.(false);

    if (!mediaElement.paused && fade) {
      const completed = await fadeTo(0, fadeOutMs);
      if (!completed) return;
    } else {
      cancelFade();
      setOutputVolume(0);
    }

    mediaElement.pause();
    if (resetTime) {
      mediaElement.currentTime = 0;
    }

    for (const element of getSyncedMediaElements()) {
      element.pause();
      if (resetTime) {
        element.currentTime = 0;
      }
    }
  }

  function syncOutputVolume() {
    const mediaElement = options.getMediaElement();

    if (activeFadeTarget === 0 && !requestedPlaying) {
      return;
    }

    if (activeFadeTarget !== null) {
      cancelFade();
    }

    if (requestedPlaying && mediaElement && !mediaElement.paused) {
      setOutputVolume(getTargetVolume());
    } else {
      setOutputVolume(0);
    }
  }

  return {
    play,
    pause,
    pauseNow,
    stop,
    syncOutputVolume,
    cancelFade,
    destroy: cancelFade,
  };
}
