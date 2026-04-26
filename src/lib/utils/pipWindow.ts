import { invoke } from "@tauri-apps/api/core";
import type { UnlistenFn } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";

const PIP_VIDEO_ACTIVE_CLASS = "pip-video-active";
const PIP_VIDEO_REFLOW_DELAY_MS = 150;
const PIP_WINDOW_SETTLE_DELAY_MS = 260;

type ResumePlayback = () => Promise<unknown>;

export async function enterNativePipWindow() {
  await invoke("enter_pip_mode");
  document.body.style.background = "#000";
}

export async function exitNativePipWindow() {
  await invoke("exit_pip_mode");
  resetPipBodyBackground();
}

export function resetPipBodyBackground() {
  document.body.style.background = "transparent";
}

export async function applyPipVideoMode(
  videoElement: HTMLVideoElement | undefined,
  active: boolean,
  shouldResumePlayback: boolean,
  resumePlayback: ResumePlayback,
) {
  await delay(PIP_VIDEO_REFLOW_DELAY_MS);

  if (!videoElement) return;

  if (active) {
    videoElement.classList.add(PIP_VIDEO_ACTIVE_CLASS);
  } else {
    videoElement.classList.remove(PIP_VIDEO_ACTIVE_CLASS);
    videoElement.style.cssText = "";
  }

  void videoElement.offsetHeight;

  if (shouldResumePlayback) {
    resumePlayback().catch(() => {});
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

  function scheduleSettle() {
    if (!isPipActive()) return;

    if (settleTimer) {
      clearTimeout(settleTimer);
    }

    settleTimer = setTimeout(() => {
      settleTimer = null;
      settlePipWindow();
    }, PIP_WINDOW_SETTLE_DELAY_MS);
  }

  async function settlePipWindow() {
    if (!isPipActive()) return;

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
      if (settleAgain) {
        settleAgain = false;
        scheduleSettle();
      }
    }
  }

  const unlistenResize = await appWindow.onResized(scheduleSettle);
  const unlistenMove = await appWindow.onMoved(scheduleSettle);

  return () => {
    if (settleTimer) {
      clearTimeout(settleTimer);
      settleTimer = null;
    }
    unlistenResize();
    unlistenMove();
  };
}

function delay(ms: number) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}
