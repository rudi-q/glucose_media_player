<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { Terminal, FolderOpen, RotateCcw } from "lucide-svelte";
  import Button from "$lib/components/Button.svelte";

  type FfmpegPathInfo = {
    path: string | null;
    is_custom: boolean;
  };

  let pathInfo = $state<FfmpegPathInfo | null>(null);
  let busy = $state(false);
  let error = $state<string | null>(null);
  let errorTimer: ReturnType<typeof setTimeout>;
  let { onPathChange }: { onPathChange?: () => void | Promise<void> } = $props();

  function showError(msg: string) {
    error = msg;
    clearTimeout(errorTimer);
    errorTimer = setTimeout(() => { error = null; }, 4000);
  }

  async function load() {
    try {
      pathInfo = await invoke<FfmpegPathInfo>("get_ffmpeg_path");
    } catch (e) {
      showError(String(e));
    }
  }

  async function pick() {
    busy = true;
    try {
      const picked = await invoke<string | null>("pick_ffmpeg_executable");
      if (!picked) return;
      await invoke("save_ffmpeg_custom_path", { path: picked });
      await load();
      await onPathChange?.();
    } catch (e) {
      showError(String(e));
    } finally {
      busy = false;
    }
  }

  async function reset() {
    busy = true;
    try {
      await invoke("save_ffmpeg_custom_path", { path: null });
      await load();
      await onPathChange?.();
    } catch (e) {
      showError(String(e));
    } finally {
      busy = false;
    }
  }

  onMount(load);
</script>

<div class="settings-section">
  <h3>FFmpeg</h3>
  <p class="section-desc">
    The binary used for media processing and subtitle generation.
  </p>

  {#if error}
    <div class="error-banner">{error}</div>
  {/if}

  <div class="path-card" class:missing={pathInfo !== null && !pathInfo.path}>
    <div class="path-main">
      <div class="path-icon-wrap">
        <Terminal size={15} class="path-icon" />
      </div>
      <div class="path-info">
        <div class="path-meta">
          <span class="path-source" class:custom={pathInfo?.is_custom}>
            {#if pathInfo === null}
              Checking
            {:else if pathInfo.path}
              {pathInfo.is_custom ? "Custom path" : "Auto-detected"}
            {:else}
              Missing
            {/if}
          </span>
        </div>
        <span class="path-text" title={pathInfo?.path ?? undefined}>
          {#if pathInfo === null}
            Looking for FFmpeg…
          {:else if pathInfo.path}
            {pathInfo.path}
          {:else}
            FFmpeg was not found
          {/if}
        </span>
      </div>
    </div>
    <div class="path-actions">
      {#if pathInfo?.is_custom}
        <Button
          variant="secondary"
          size="sm"
          onclick={reset}
          disabled={busy}
          title="Reset to auto-detected"
        >
          <RotateCcw size={13} />
        </Button>
      {/if}
      <Button variant="primary" size="sm" onclick={pick} loading={busy}>
        <FolderOpen size={13} /> Change
      </Button>
    </div>
  </div>
</div>

<style>
  .settings-section {
    margin-bottom: 2rem;
  }

  h3 {
    font-size: 1.125rem;
    font-weight: 600;
    color: #fff;
    margin: 0 0 1.5rem 0;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    letter-spacing: -0.01em;
  }

  .section-desc {
    font-size: 0.8125rem;
    color: rgba(255, 255, 255, 0.5);
    line-height: 1.5;
    margin: -0.5rem 0 1.25rem;
  }

  .error-banner {
    padding: 0.5rem 0.75rem;
    background: rgba(239, 68, 68, 0.1);
    border: 1px solid rgba(239, 68, 68, 0.3);
    border-radius: 6px;
    font-size: 0.8125rem;
    color: rgba(239, 68, 68, 0.9);
    margin-bottom: 0.75rem;
  }

  .path-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 1rem;
    padding: 1rem;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
  }

  .path-card.missing {
    border-color: rgba(239, 68, 68, 0.2);
    background: rgba(239, 68, 68, 0.04);
  }

  .path-main {
    display: flex;
    align-items: center;
    gap: 0.875rem;
    flex: 1;
    min-width: 0;
  }

  .path-icon-wrap {
    width: 2rem;
    height: 2rem;
    border-radius: 8px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.04);
    border: 1px solid rgba(255, 255, 255, 0.06);
    flex-shrink: 0;
  }

  :global(.path-icon) {
    color: rgba(255, 255, 255, 0.35);
    flex-shrink: 0;
  }

  .path-info {
    display: flex;
    flex-direction: column;
    gap: 0.35rem;
    flex: 1;
    min-width: 0;
  }

  .path-meta {
    display: flex;
    align-items: center;
    min-width: 0;
  }

  .path-source {
    font-size: 0.6875rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.42);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .path-source.custom {
    color: rgba(99, 179, 237, 0.9);
  }

  .path-text {
    font-size: 0.875rem;
    font-family: monospace;
    color: rgba(255, 255, 255, 0.76);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .path-actions {
    display: flex;
    gap: 0.5rem;
    flex-shrink: 0;
  }
</style>
