<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";
  import { FolderOpen, Plus, Trash2 } from "lucide-svelte";
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

  async function addFolder() {
    const folder = await invoke<string | null>("open_folder_dialog");
    if (folder && !paths.includes(folder)) {
      paths = [...paths, folder];
      await save();
    }
  }

  async function removeFolder(path: string) {
    paths = paths.filter((p) => p !== path);
    await save();
  }

  async function save() {
    try {
      await invoke("save_gallery_paths", { paths });
      galleryRefreshStore.refresh();
    } catch (err) {
      console.error("Failed to save gallery paths:", err);
    }
  }
</script>

<div class="settings-section">
  <h3>Gallery Locations</h3>
  <p class="section-desc">
    Videos and audio from these folders appear in the gallery. Subfolders are
    scanned up to 5 levels deep.
  </p>

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
      {#each paths as path}
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
