<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { FolderOpen, Plus, ShieldCheck, Trash2 } from "lucide-svelte";
  import Button from "$lib/components/Button.svelte";
  import { galleryRefreshStore } from "$lib/stores/appStore";

  let paths = $state<string[]>([]);
  let loading = $state(true);

  onMount(async () => {
    try {
      paths = await invoke<string[]>("get_gallery_paths");
    } catch (err) {
      console.error("Failed to load gallery paths:", err);
    } finally {
      loading = false;
    }
  });

  // Normalize for dedup comparison only (case-insensitive on Windows, no trailing slash)
  function normalizePath(p: string): string {
    return p.toLowerCase().replace(/\\/g, '/').replace(/\/$/, '');
  }

  async function addFolder() {
    const folder = await invoke<string | null>("open_folder_dialog");
    if (!folder) return;
    if (paths.some(p => normalizePath(p) === normalizePath(folder))) return;
    const prev = paths;
    paths = [...paths, folder];
    try {
      await save();
    } catch (err) {
      paths = prev;
      console.error("Failed to add folder:", err);
    }
  }

  async function removeFolder(path: string) {
    const prev = paths;
    paths = paths.filter((p) => p !== path);
    try {
      await save();
    } catch (err) {
      paths = prev;
      console.error("Failed to remove folder:", err);
    }
  }

  async function save() {
    await invoke("save_gallery_paths", { paths });
    galleryRefreshStore.refresh();
  }
</script>

<div class="settings-section">
  <h3>Allowed Locations</h3>
  <p class="section-desc">
    Media in these folders appears in the gallery, including subfolders up to 5 levels deep.
  </p>

  <div class="privacy-notice">
    <ShieldCheck size={14} class="privacy-icon" />
    <span><strong>Fully Offline & Private.</strong> Glucose never collects, uploads, or shares your data. You can safely add any personal folders to your gallery, knowing everything stays on your device.</span>
  </div>

  <div class="actions">
    <Button variant="primary" size="sm" onclick={addFolder}>
      <Plus size={14} /> Add Folder
    </Button>
  </div>

  {#if loading}
    <div class="state-msg">Loading…</div>
  {:else if paths.length === 0}
    <div class="state-msg">No locations configured.</div>
  {:else}
    <div class="path-list">
      {#each paths as path (path)}
        <div class="path-row">
          <FolderOpen size={16} class="path-icon" />
          <span class="path-text" title={path}>{path}</span>
          <Button
            variant="secondary"
            size="sm"
            onclick={() => removeFolder(path)}
            disabled={paths.length <= 1}
            title={paths.length <= 1
              ? "At least one location is required"
              : "Remove location"}
          >
            <Trash2 size={14} />
          </Button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
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

  .privacy-notice {
    display: flex;
    align-items: flex-start;
    gap: 0.5rem;
    padding: 0.625rem 0.875rem;
    background: rgba(74, 222, 128, 0.06);
    border: 1px solid rgba(74, 222, 128, 0.15);
    border-radius: 8px;
    margin-bottom: 1.25rem;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.45);
    line-height: 1.5;
  }

  :global(.privacy-icon) {
    color: rgba(74, 222, 128, 0.6);
    flex-shrink: 0;
    margin-top: 1px;
  }

  .actions {
    margin-bottom: 1rem;
  }

  .path-list {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .path-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    transition: all 0.2s ease;
  }

  .path-row:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
  }

  :global(.path-icon) {
    color: rgba(255, 255, 255, 0.35);
    flex-shrink: 0;
  }

  .path-text {
    flex: 1;
    font-size: 0.8125rem;
    font-family: monospace;
    color: rgba(255, 255, 255, 0.7);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    min-width: 0;
  }

  .state-msg {
    text-align: center;
    padding: 2rem;
    color: rgba(255, 255, 255, 0.35);
    font-size: 0.875rem;
  }
</style>
