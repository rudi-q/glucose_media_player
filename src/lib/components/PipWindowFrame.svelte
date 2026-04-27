<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";
  import { Minus, X } from "lucide-svelte";

  type ResizeDirection =
    | "East"
    | "North"
    | "NorthEast"
    | "NorthWest"
    | "South"
    | "SouthEast"
    | "SouthWest"
    | "West";

  let { onClose }: { onClose: () => void } = $props();

  const appWindow = getCurrentWindow();
  const resizeHandles: Array<{
    className: string;
    direction: ResizeDirection;
  }> = [
    { className: "edge-top", direction: "North" },
    { className: "edge-right", direction: "East" },
    { className: "edge-bottom", direction: "South" },
    { className: "edge-left", direction: "West" },
    { className: "corner-top-left", direction: "NorthWest" },
    { className: "corner-top-right", direction: "NorthEast" },
    { className: "corner-bottom-right", direction: "SouthEast" },
    { className: "corner-bottom-left", direction: "SouthWest" },
  ];

  const HIDE_DELAY_MS = 2000;
  let visible = $state(false);
  let hideTimer: ReturnType<typeof setTimeout> | null = null;

  function showHeader() {
    visible = true;
    if (hideTimer) clearTimeout(hideTimer);
    hideTimer = setTimeout(() => {
      visible = false;
      hideTimer = null;
    }, HIDE_DELAY_MS);
  }

  onMount(() => {
    document.addEventListener("mousemove", showHeader);
    return () => {
      document.removeEventListener("mousemove", showHeader);
      if (hideTimer) clearTimeout(hideTimer);
    };
  });

  function startWindowDrag(event: MouseEvent) {
    if (event.button !== 0) return;
    if ((event.target as HTMLElement).closest("button")) return;

    event.preventDefault();
    appWindow.startDragging().catch((err) => {
      console.error("Failed to start PiP window drag:", err);
    });
  }

  function startWindowResize(direction: ResizeDirection, event: MouseEvent) {
    if (event.button !== 0) return;

    event.preventDefault();
    event.stopPropagation();
    appWindow.startResizeDragging(direction).catch((err) => {
      console.error("Failed to start PiP window resize:", err);
    });
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="pip-drag-header" class:visible onmousedown={startWindowDrag}>
  <button class="pip-close-button" onclick={() => appWindow.minimize()} title="Minimize">
    <Minus size={14} />
  </button>
  <button class="pip-close-button" onclick={onClose} title="Close (Esc)">
    <X size={14} />
  </button>
</div>

{#each resizeHandles as handle}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="pip-resize-handle {handle.className}"
    onmousedown={(event) => startWindowResize(handle.direction, event)}
  ></div>
{/each}

<style>
  .pip-drag-header {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    height: 32px;
    background: rgba(0, 0, 0, 0.7);
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px 8px 0 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    padding: 0 8px;
    z-index: 220;
    cursor: move;
    opacity: 0;
    transition: opacity 0.2s ease;
  }

  .pip-drag-header.visible {
    opacity: 1;
  }

  .pip-close-button {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.1);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .pip-close-button:hover {
    background: rgba(255, 255, 255, 0.2);
    border-color: rgba(255, 255, 255, 0.3);
    color: #fff;
  }

  .pip-resize-handle {
    position: fixed;
    z-index: 230;
  }

  .edge-top {
    top: 0;
    left: 14px;
    right: 14px;
    height: 8px;
    cursor: n-resize;
  }

  .edge-right {
    top: 14px;
    right: 0;
    bottom: 14px;
    width: 8px;
    cursor: e-resize;
  }

  .edge-bottom {
    right: 14px;
    bottom: 0;
    left: 14px;
    height: 8px;
    cursor: s-resize;
  }

  .edge-left {
    top: 14px;
    bottom: 14px;
    left: 0;
    width: 8px;
    cursor: w-resize;
  }

  .corner-top-left,
  .corner-top-right,
  .corner-bottom-right,
  .corner-bottom-left {
    width: 16px;
    height: 16px;
  }

  .corner-top-left {
    top: 0;
    left: 0;
    cursor: nw-resize;
  }

  .corner-top-right {
    top: 0;
    right: 0;
    cursor: ne-resize;
  }

  .corner-bottom-right {
    right: 0;
    bottom: 0;
    cursor: se-resize;
  }

  .corner-bottom-left {
    bottom: 0;
    left: 0;
    cursor: sw-resize;
  }
</style>
