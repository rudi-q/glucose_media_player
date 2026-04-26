import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

const PIP_VIDEO_ACTIVE_CLASS = "pip-video-active";
const PIP_VIDEO_REFLOW_DELAY_MS = 150;
const PIP_WINDOW_SETTLE_DELAY_MS = 260;

type ResumePlayback = () => Promise<unknown>;

let originalPipBodyBackground: string | undefined;

export async function enterNativePipWindow() {
  await invoke("enter_pip_mode");
  if (originalPipBodyBackground === undefined) {
    originalPipBodyBackground = document.body.style.background;
    document.body.style.background = "#000";
  }
}

export async function exitNativePipWindow() {
  try {
    await invoke("exit_pip_mode");
  } finally {
    resetPipBodyBackground();
  }
}

export function resetPipBodyBackground() {
  if (originalPipBodyBackground) {
    document.body.style.background = originalPipBodyBackground;
  } else {
    document.body.style.removeProperty("background");
  }
  originalPipBodyBackground = undefined;
}

export async function applyPipVideoMode(
  videoElement: HTMLVideoElement | undefined,
  active: boolean,
  shouldResumePlayback: boolean,
  resumePlayback: ResumePlayback,
) {
  if (!videoElement) return;

  await delay(PIP_VIDEO_REFLOW_DELAY_MS);

  if (active) {
    videoElement.classList.add(PIP_VIDEO_ACTIVE_CLASS);
  } else {
    videoElement.classList.remove(PIP_VIDEO_ACTIVE_CLASS);
    for (const prop of [
      "width", "height", "max-width", "max-height",
      "position", "top", "left",
      "display", "visibility", "opacity", "object-fit",
    ]) {
      videoElement.style.removeProperty(prop);
    }
  }

  void videoElement.offsetHeight;

  if (shouldResumePlayback) {
    resumePlayback().catch((err) => console.debug("resumePlayback failed", err));
  }
}

export async function savePipWindowLayout() {
  await invoke("save_pip_window_layout");
}

export async function createPipWindowSettler(
  isPipActive: () => boolean,
): Promise<UnlistenFn> {
  const appWindow = getCurrentWindow();
  let settleTimer: ReturnType<typeof setTimeout> | null = null;
  let settleInFlight = false;
  let settleAgain = false;
  let destroyed = false;

  function scheduleSettle() {
    if (destroyed || !isPipActive()) return;

    if (settleTimer) {
      clearTimeout(settleTimer);
    }

    settleTimer = setTimeout(() => {
      settleTimer = null;
      settlePipWindow();
    }, PIP_WINDOW_SETTLE_DELAY_MS);
  }

  async function settlePipWindow() {
    if (destroyed || !isPipActive()) return;

    if (settleInFlight) {
      settleAgain = true;
      return;
    }

    settleInFlight = true;
    try {
      await invoke("settle_pip_window");
    } catch (err) {
      console.error("Failed to settle PiP window:", err);
    } finally {
      settleInFlight = false;
      if (!destroyed && settleAgain) {
        settleAgain = false;
        scheduleSettle();
      }
    }
  }

  const unlistenResize = await appWindow.onResized(scheduleSettle);
  let unlistenMove: UnlistenFn | undefined;
  try {
    unlistenMove = await appWindow.onMoved(scheduleSettle);
  } catch (err) {
    unlistenResize();
    throw err;
  }

  return () => {
    destroyed = true;
    if (settleTimer) {
      clearTimeout(settleTimer);
      settleTimer = null;
    }
    unlistenResize();
    unlistenMove?.();
  };
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
