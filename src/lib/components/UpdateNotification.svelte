<script lang="ts">
	import { updateStore } from '$lib/stores/updateStore';
	import { onDestroy } from 'svelte';
	import { Loader2 } from 'lucide-svelte';

	$: updateState = $updateStore;
  $: progressPercentage = updateState.contentLength > 0 
    ? Math.round((updateState.downloaded / updateState.contentLength) * 100) 
    : 0;

  // Auto-hide completed state after 5 seconds
  let hideTimeout: ReturnType<typeof setTimeout>;
  $: if (updateState.completed) {
    hideTimeout = setTimeout(() => {
      updateStore.reset();
    }, 5000);
  }

  onDestroy(() => {
    if (hideTimeout) {
      clearTimeout(hideTimeout);
    }
  });

  function formatBytes(bytes: number): string {
    if (bytes === 0) return '0 B';
    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(1)) + ' ' + sizes[i];
  }
</script>

<!-- Update Notification Panel -->
{#if updateState.checking || updateState.available || updateState.downloading || updateState.completed || updateState.error}
  <div class="update-notification">
    <div class="update-card">
      {#if updateState.checking}
        <div class="update-content">
          <div class="update-icon">
            <Loader2 size={20} class="spinner" />
          </div>
          <div class="update-text">
            <h3 class="update-title">Checking for updates...</h3>
            <p class="update-desc">Looking for the latest version</p>
          </div>
        </div>
      {/if}

      {#if updateState.available && !updateState.downloading && !updateState.completed}
        <div class="update-content">
          <div class="update-icon success">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"></path>
              <polyline points="7 10 12 15 17 10"></polyline>
              <line x1="12" y1="15" x2="12" y2="3"></line>
            </svg>
          </div>
          <div class="update-text">
            <h3 class="update-title">Update Available</h3>
            {#if updateState.version}
              <p class="update-desc">Version {updateState.version}</p>
            {/if}
            {#if updateState.body}
              <p class="update-body">{updateState.body}</p>
            {/if}
            <p class="update-status">Download starting automatically...</p>
          </div>
        </div>
      {/if}

      {#if updateState.downloading}
        <div class="update-content">
          <div class="update-icon">
            <Loader2 size={20} class="spinner" />
          </div>
          <div class="update-text">
            <h3 class="update-title">Downloading Update...</h3>
            <div class="progress-container">
              <div class="progress-info">
                <span>{progressPercentage}%</span>
                <span>
                  {formatBytes(updateState.downloaded)} / {formatBytes(updateState.contentLength)}
                </span>
              </div>
              <div class="progress-track">
                <div 
                  class="progress-fill"
                  style="width: {progressPercentage}%"
                ></div>
              </div>
            </div>
            <p class="update-desc">The app will restart automatically when complete</p>
          </div>
        </div>
      {/if}

      {#if updateState.completed}
        <div class="update-content">
          <div class="update-icon success">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <polyline points="20 6 9 17 4 12"></polyline>
            </svg>
          </div>
          <div class="update-text">
            <h3 class="update-title">Update Complete</h3>
            <p class="update-desc">Restarting application...</p>
          </div>
        </div>
      {/if}

      {#if updateState.error}
        <div class="update-content">
          <div class="update-icon error">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <circle cx="12" cy="12" r="10"></circle>
              <line x1="12" y1="8" x2="12" y2="12"></line>
              <line x1="12" y1="16" x2="12.01" y2="16"></line>
            </svg>
          </div>
          <div class="update-text">
            <h3 class="update-title">Update Failed</h3>
            <p class="update-error">{updateState.error}</p>
            <button 
              class="dismiss-button"
              on:click={() => updateStore.reset()}
            >
              Dismiss
            </button>
          </div>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .update-notification {
    position: fixed;
    top: 1.5rem;
    right: 1.5rem;
    z-index: 1000;
    animation: slideIn 0.3s ease-out;
  }

  @keyframes slideIn {
    from {
      opacity: 0;
      transform: translateY(-20px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }

  .update-card {
    background: rgba(0, 0, 0, 0.95);
    backdrop-filter: blur(10px);
    -webkit-backdrop-filter: blur(10px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 1rem;
    width: 320px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
  }

  .update-content {
    display: flex;
    gap: 0.75rem;
    align-items: flex-start;
  }

  .update-icon {
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: rgba(255, 255, 255, 0.9);
  }

  .update-icon.success {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .update-icon.error {
    background: rgba(255, 0, 0, 0.1);
    border-color: rgba(255, 0, 0, 0.3);
    color: #ff5555;
  }

  .update-icon :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from {
      transform: rotate(0deg);
    }
    to {
      transform: rotate(360deg);
    }
  }

  .update-text {
    flex: 1;
    min-width: 0;
  }

  .update-title {
    font-size: 0.875rem;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.9);
    margin: 0 0 0.25rem 0;
  }

  .update-desc {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.5);
    margin: 0;
  }

  .update-body {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.6);
    margin: 0.5rem 0;
    line-height: 1.4;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
  }

  .update-status {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.7);
    margin: 0.5rem 0 0 0;
  }

  .update-error {
    font-size: 0.75rem;
    color: #ff5555;
    margin: 0.5rem 0;
    line-height: 1.4;
  }

  .progress-container {
    margin: 0.75rem 0 0.5rem 0;
  }

  .progress-info {
    display: flex;
    justify-content: space-between;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.7);
    margin-bottom: 0.5rem;
    font-variant-numeric: tabular-nums;
  }

  .progress-track {
    width: 100%;
    height: 4px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 2px;
    overflow: hidden;
  }

  .progress-fill {
    height: 100%;
    background: rgba(255, 255, 255, 0.9);
    transition: width 0.3s ease;
    border-radius: 2px;
  }

  .dismiss-button {
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    padding: 0.5rem 1rem;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.9);
    cursor: pointer;
    transition: all 0.15s ease;
    margin-top: 0.5rem;
  }

  .dismiss-button:hover {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.2);
  }
</style>
