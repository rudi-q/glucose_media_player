import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow, currentMonitor } from "@tauri-apps/api/window";
import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";

// Saved windowed geometry so leaving fullscreen restores the exact pre-fullscreen
// size/position. null whenever we are NOT in borderless fullscreen.
//
// Module-level (not per-component) on purpose: only one player route — video OR audio —
// is ever mounted at a time, and both exit paths (leaving fullscreen, or unmounting via
// exitBorderlessFullscreen) clear this back to null, so the two pages can safely share a
// single fullscreen state machine without ever seeing each other's stale geometry.
let savedFullscreenGeometry:
  | { size: PhysicalSize; position: PhysicalPosition; maximized: boolean }
  | null = null;

// Borderless fullscreen sized to the FULL monitor (taskbar strip included). We size
// the window explicitly instead of relying on setFullscreen() because, on Windows, a
// transparent (layered) frameless window is sized to the work area by the OS
// fullscreen path — leaving the taskbar uncovered. always-on-top then lifts the
// window above the (topmost) taskbar; it is released/re-applied on blur/focus (see
// the focus listener each player registers in onMount) so the user can still Alt-Tab away.
export async function enterBorderlessFullscreen() {
  const win = getCurrentWindow();
  // Square off the windowed rounded corners + inset border (set on <body> globally)
  // so the media reaches the physical screen edges. Done here rather than in a
  // reactive $effect so it's tied directly to the fullscreen transition that we know
  // runs. The visible corner rounding is the OS (DWM) one, so also square the window
  // itself — CSS alone can't override the OS window shape.
  document.body.classList.add("fullscreen-chrome");
  invoke("set_window_corner_rounded", { rounded: false }).catch(() => {});
  // Already fullscreen: just re-assert the pin (e.g. after a focus blip) and bail so
  // we never overwrite the saved windowed geometry with the full-monitor size.
  if (savedFullscreenGeometry) {
    await win.setAlwaysOnTop(true);
    return;
  }
  const monitor = await currentMonitor();
  if (!monitor) {
    // Could not resolve the monitor — fall back to the OS fullscreen path so we at
    // least fill the work area rather than doing nothing.
    await win.setFullscreen(true);
    await win.setAlwaysOnTop(true);
    return;
  }
  const maximized = await win.isMaximized();
  savedFullscreenGeometry = {
    // innerSize (not outerSize): the exit restore uses setSize(), which sets the
    // INNER size. Saving the outer size here would re-inflate the window by the
    // frameless border margin on every fullscreen round-trip (it would creep larger
    // each cycle). position stays outer — setPosition is outer-relative too.
    size: await win.innerSize(),
    position: await win.outerPosition(),
    maximized,
  };
  // A maximized window reports work-area bounds; clear it so the exit restore puts
  // the window back to its true pre-fullscreen geometry.
  if (maximized) {
    await win.unmaximize();
  }
  // setSize sizes the inner/client area to the monitor, but a frameless Windows
  // window carries an invisible resize-border margin around the client. That margin
  // is NOT symmetric (typically ~13px left/right but ~0 top / ~15 bottom), so simply
  // centering the outer window leaves a gap at one edge (a strip on the left, or a
  // gap at the bottom). Instead, measure the actual client offset and shift the outer
  // window so the client's top-left lands exactly on the monitor origin — the client
  // (= the media) then covers the monitor precisely, flush on all four edges.
  await win.setSize(
    new PhysicalSize(monitor.size.width, monitor.size.height),
  );
  await win.setPosition(
    new PhysicalPosition(monitor.position.x, monitor.position.y),
  );
  const outerPos = await win.outerPosition();
  const innerPos = await win.innerPosition();
  const borderX = innerPos.x - outerPos.x;
  const borderY = innerPos.y - outerPos.y;
  await win.setPosition(
    new PhysicalPosition(
      monitor.position.x - borderX,
      monitor.position.y - borderY,
    ),
  );
  await win.setAlwaysOnTop(true);
}

export async function exitBorderlessFullscreen() {
  const win = getCurrentWindow();
  // Restore the windowed rounded corners + inset border (CSS) and the OS window
  // rounding.
  document.body.classList.remove("fullscreen-chrome");
  invoke("set_window_corner_rounded", { rounded: true }).catch(() => {});
  // Drop the pin and any OS-fullscreen fallback first. Re-assert resizability as a
  // self-healing safety net — the window must never get stuck non-resizable
  // (otherwise edge-resize and maximize silently stop working) — before restoring
  // geometry.
  await win.setAlwaysOnTop(false);
  await win.setFullscreen(false);
  await win.setResizable(true);
  if (!savedFullscreenGeometry) {
    return;
  }
  const { size, position, maximized } = savedFullscreenGeometry;
  savedFullscreenGeometry = null;
  if (maximized) {
    await win.maximize();
  } else {
    await win.setSize(size);
    await win.setPosition(position);
  }
}
