<script lang="ts">
  import '$lib/styles/global.css';
  import { invoke } from "@tauri-apps/api/core";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount, setContext } from "svelte";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import { X, Settings, Check, Loader2 } from "lucide-svelte";
  import UpdateManager, { type UpdateManagerAPI } from "$lib/components/UpdateManager.svelte";
  import UpdateNotification from "$lib/components/UpdateNotification.svelte";
  import { getFormattedVersion } from "$lib/utils/version";
  import { appSettings, setupStore, type SetupStatus } from "$lib/stores/appStore";
  import { watchProgressStore } from "$lib/stores/watchProgressStore";
  
  let { children } = $props();
  
  // Shared state accessible via context
  let showSettings = $state(false);
  let showSetupDialog = $state(false);
  let selectedModelForSetup = $state<string>("tiny");
  let isDownloading = $state(false);
  let downloadProgress = $state(0);
  let downloadMessage = $state("");
  let isCheckingForUpdates = $state(false);
  
  // Update manager
  let updateManager: UpdateManagerAPI;
  let isOnGallery = $state(true);
  
  // Initialize from localStorage if available
  let lastAutoCheckTime = $state<number>(0);
  if (typeof localStorage !== 'undefined') {
    const stored = localStorage.getItem('lastAutoCheckTime');
    lastAutoCheckTime = stored ? parseInt(stored, 10) || 0 : 0;
  }
  
  // Subscribe to stores
  let settings = $state($appSettings);
  let setupStatus = $state<SetupStatus | null>(null);
  
  // Subscribe to setupStore changes
  setupStore.subscribe(status => {
    setupStatus = status;
  });
  
  // Track current route
  $effect(() => {
    isOnGallery = $page.route.id === '/' || $page.route.id === null;
  });
  
  // Provide context for child routes
  setContext('showSettings', () => showSettings = true);
  setContext('showSetupDialog', () => showSetupDialog = true);
  setContext('setupStatus', () => setupStatus);
  
  onMount(() => {
    let disposed = false;
    const unsubs: UnlistenFn[] = [];
    
    // Register Tauri event listeners
    (async () => {
      const results = await Promise.allSettled([
        listen<string>("open-file", async (event) => {
          // Navigate to player with the file path
          const encodedPath = encodeURIComponent(event.payload);
          await goto(`/player/${encodedPath}`);
          // Mark file as processed
          invoke("mark_file_processed").catch(console.error);
        }),
        listen<string[]>("tauri://drag-drop", async (event) => {
          if (event.payload && event.payload.length > 0) {
            const encodedPath = encodeURIComponent(event.payload[0]);
            await goto(`/player/${encodedPath}`);
          }
        }),
        // Listen for download progress
        listen<{downloaded: number, total: number, percentage: number, message: string}>("download-progress", (event) => {
          downloadProgress = event.payload.percentage;
          downloadMessage = event.payload.message;
        }),
      ]);
      
      for (const r of results) {
        if (r.status === "fulfilled") {
          const un = r.value;
          if (disposed) {
            try { un(); } catch (e) { console.error("Unlisten failed", e); }
          } else {
            unsubs.push(un);
          }
        } else {
          console.error("Failed to register Tauri listener:", r.reason);
        }
      }
    })();
    
    // Check setup status on first launch
    checkSetupStatus();
    
    // Notify backend that frontend is ready
    invoke("frontend_ready").catch(console.error);
    
    return () => {
      disposed = true;
      for (const un of unsubs) {
        try { un(); } catch (e) { console.error("Unlisten failed", e); }
      }
    };
  });
  
  async function checkSetupStatus() {
    try {
      const status = await invoke<SetupStatus>('get_setup_status');
      setupStore.setStatus(status);
      
      // Show setup dialog on first launch if not completed
      if (!status.setup_completed && status.models_installed.length === 0) {
        setTimeout(() => {
          showSetupDialog = true;
        }, 1500);
      }
    } catch (err) {
      console.error('Failed to check setup status:', err);
    }
  }
  
  async function runSetup() {
    if (!setupStatus) return;
    
    const isModelInstalled = setupStatus.models_installed.includes(selectedModelForSetup);
    
    if (isModelInstalled) {
      try {
        await invoke('mark_setup_completed');
        await checkSetupStatus();
        showSetupDialog = false;
      } catch (err) {
        console.error('Failed to mark setup complete:', err);
      }
      return;
    }
    
    isDownloading = true;
    downloadProgress = 0;
    downloadMessage = "Starting download...";
    
    try {
      await invoke('download_whisper_model', {
        modelSize: selectedModelForSetup
      });
      
      await invoke('mark_setup_completed');
      await checkSetupStatus();
      
      isDownloading = false;
      downloadProgress = 100;
      downloadMessage = "Download complete!";
      
      setTimeout(() => {
        showSetupDialog = false;
        downloadProgress = 0;
        downloadMessage = "";
      }, 2000);
    } catch (err) {
      console.error('Setup failed:', err);
      alert(`Setup failed: ${err}`);
      isDownloading = false;
      downloadProgress = 0;
      downloadMessage = "";
    }
  }
  
  function skipSetup() {
    showSetupDialog = false;
    invoke('mark_setup_completed').catch(console.error);
  }
  
  function handleAutoCheckStart() {
    lastAutoCheckTime = Date.now();
  }
  
  function handleAutoCheckTimeUpdate(time: number) {
    lastAutoCheckTime = time;
    if (typeof localStorage !== 'undefined') {
      localStorage.setItem('lastAutoCheckTime', time.toString());
    }
  }
</script>

<!-- Update System -->
<UpdateManager 
  bind:this={updateManager} 
  disableAutoCheck={!isOnGallery}
  onAutoCheckStart={handleAutoCheckStart}
  onAutoCheckTimeUpdate={handleAutoCheckTimeUpdate}
  lastAutoCheckTime={lastAutoCheckTime}
/>
<UpdateNotification />

{@render children()}

<!-- Settings Overlay -->
{#if showSettings}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div class="settings-overlay" onclick={(e) => { if (e.target === e.currentTarget) showSettings = false; }}>
    <div class="settings-modal">
      <div class="settings-header">
        <div class="settings-header-content">
          <h2>Settings</h2>
          <span class="settings-version">{getFormattedVersion()}</span>
        </div>
        <button class="settings-close" onclick={() => showSettings = false} title="Close">
          <X size={20} />
        </button>
      </div>
      
      <div class="settings-content">
        <!-- AI Settings Section -->
        <div class="settings-section">
          <h3>AI Settings</h3>
          
          {#if setupStatus}
            <div class="settings-group">
              <div class="settings-item">
                <div class="settings-item-label">
                  <div class="settings-item-title">AI Subtitle Generation</div>
                  <div class="settings-item-desc">
                    Automatically generate subtitles from video audio using Whisper AI
                  </div>
                </div>
                <div class="settings-item-status">
                  {#if setupStatus.models_installed.length > 0}
                    <span class="status-badge active">Enabled</span>
                  {:else}
                    <span class="status-badge inactive">Not Set Up</span>
                  {/if}
                </div>
              </div>
              
              <div class="settings-item">
                <div class="settings-item-label">
                  <div class="settings-item-title">FFmpeg</div>
                  <div class="settings-item-desc">Required for audio extraction from videos</div>
                </div>
                <div class="settings-item-status">
                  {#if setupStatus.ffmpeg_installed}
                    <span class="status-badge active">✓ Installed</span>
                  {:else}
                    <span class="status-badge inactive">✗ Not Installed</span>
                  {/if}
                </div>
              </div>
              
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <!-- svelte-ignore a11y_no_static_element_interactions -->
              <div class="settings-item" onclick={() => { showSettings = false; showSetupDialog = true; }} style="cursor: pointer;" role="button" tabindex="0">
                <div class="settings-item-label">
                  <div class="settings-item-title">AI Models</div>
                  <div class="settings-item-desc">
                    {#if setupStatus.models_installed.length > 0}
                      {@const totalModels = 3}
                      {@const installedCount = setupStatus.models_installed.length}
                      {@const availableCount = totalModels - installedCount}
                      {@const recommendedModel = setupStatus.models_installed.includes('tiny') ? 'Tiny' : setupStatus.models_installed.includes('small') ? 'Small' : setupStatus.models_installed.includes('large-v3-turbo') ? 'Large V3 Turbo' : setupStatus.models_installed[0]}
                      {recommendedModel} model recommended{#if installedCount > 1}, {installedCount - 1} more installed{/if}{#if availableCount > 0}, {availableCount} more available{/if}
                    {:else}
                      No models installed, 3 available
                    {/if}
                  </div>
                </div>
                <div class="settings-item-status">
                  {#if setupStatus.models_installed.length > 0}
                    <span class="status-badge active">Installed {setupStatus.models_installed.length} {setupStatus.models_installed.length === 1 ? 'model' : 'models'}</span>
                  {:else}
                    <span class="status-badge inactive">Not Set Up</span>
                  {/if}
                </div>
              </div>
              
              <div class="settings-item">
                <div class="settings-item-label">
                  <div class="settings-item-title">Subtitle Language</div>
                  <div class="settings-item-desc">
                    Select the language for AI-generated subtitles
                  </div>
                </div>
                <div class="settings-item-action">
                  <select 
                    class="language-select"
                    bind:value={settings.subtitleLanguage}
                    onchange={(e) => appSettings.updateSubtitleLanguage((e.target as HTMLSelectElement).value)}
                  >
                    <option value="auto">Auto Detect</option>
                    <option value="en">English</option>
                    <option value="es">Spanish</option>
                    <option value="fr">French</option>
                    <option value="de">German</option>
                    <option value="it">Italian</option>
                    <option value="pt">Portuguese</option>
                    <option value="ru">Russian</option>
                    <option value="ja">Japanese</option>
                    <option value="ko">Korean</option>
                    <option value="zh">Chinese</option>
                    <option value="ar">Arabic</option>
                    <option value="hi">Hindi</option>
                    <option value="nl">Dutch</option>
                    <option value="pl">Polish</option>
                    <option value="tr">Turkish</option>
                  </select>
                </div>
              </div>
            </div>
          {/if}
        </div>
        
        <!-- App Updates Section -->
        <div class="settings-section">
          <h3>App Updates</h3>
          
          <div class="settings-group">
            <div class="settings-item">
              <div class="settings-item-label">
                <div class="settings-item-title">Check for Updates</div>
                <div class="settings-item-desc">
                  Keep Glucose up to date with the latest features and improvements
                </div>
              </div>
              <div class="settings-item-action">
                <button 
                  class="check-update-button"
                  disabled={isCheckingForUpdates}
                  onclick={async () => {
                    if (updateManager && !isCheckingForUpdates) {
                      try {
                        isCheckingForUpdates = true;
                        const checkPromise = updateManager.manualCheckForUpdates();
                        if (checkPromise) {
                          await checkPromise;
                        } else {
                          console.log('Update check already in progress');
                        }
                      } finally {
                        isCheckingForUpdates = false;
                      }
                    }
                  }}
                >
                  {isCheckingForUpdates ? 'Checking...' : 'Check Now'}
                </button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
{/if}

<!-- First-Run Setup Dialog -->
{#if showSetupDialog}
  <div class="setup-overlay">
    <div class="setup-modal">
      <h2>Enable AI Subtitle Generation?</h2>
      <p class="setup-description">
        Automatically generate subtitles from video audio using AI.
        This feature requires downloading additional components.
      </p>
      
      {#if setupStatus}
        <div class="setup-checklist">
          <h3>Requirements</h3>
          
          <!-- FFmpeg -->
          <div class="setup-item">
            <div class="setup-item-header">
              <div class="checkbox" class:checked={setupStatus.ffmpeg_installed}>
                {#if setupStatus.ffmpeg_installed}
                  <Check size={16} strokeWidth={3} />
                {/if}
              </div>
              <div class="setup-item-info">
                <div class="setup-item-title">FFmpeg (Required)</div>
                <div class="setup-item-desc">
                  {#if setupStatus.ffmpeg_installed}
                    ✓ Already installed
                  {:else}
                    ❌ Not installed - Please install FFmpeg manually
                  {/if}
                </div>
              </div>
            </div>
          </div>
          
          <!-- AI Model Selection -->
          <div class="setup-item">
            <div class="setup-item-header">
              <div class="setup-item-info full-width">
                <div class="setup-item-title">AI Model (Choose one)</div>
                <div class="model-choices">
                  <label class="model-radio" class:installed={setupStatus.models_installed.includes('tiny')}>
                    <input 
                      type="radio" 
                      name="model" 
                      value="tiny" 
                      bind:group={selectedModelForSetup}
                      disabled={isDownloading}
                    />
                    <div class="radio-content">
                      <div class="radio-header">
                        <span class="radio-title">Lite Model</span>
                        {#if setupStatus.models_installed.includes('tiny')}
                          <span class="installed-badge">✓ Installed</span>
                        {/if}
                      </div>
                      <span class="radio-desc">75 MB • Fastest • Good accuracy</span>
                    </div>
                  </label>
                  
                  <label class="model-radio" class:installed={setupStatus.models_installed.includes('small')}>
                    <input 
                      type="radio" 
                      name="model" 
                      value="small" 
                      bind:group={selectedModelForSetup}
                      disabled={isDownloading}
                    />
                    <div class="radio-content">
                      <div class="radio-header">
                        <span class="radio-title">Optimal Model</span>
                        {#if setupStatus.models_installed.includes('small')}
                          <span class="installed-badge">✓ Installed</span>
                        {/if}
                      </div>
                      <span class="radio-desc">466 MB • Balanced • Very good accuracy</span>
                    </div>
                  </label>
                  
                  <label class="model-radio" class:installed={setupStatus.models_installed.includes('large-v3-turbo')}>
                    <input 
                      type="radio" 
                      name="model" 
                      value="large-v3-turbo" 
                      bind:group={selectedModelForSetup}
                      disabled={isDownloading}
                    />
                    <div class="radio-content">
                      <div class="radio-header">
                        <span class="radio-title">Most Optimal Model</span>
                        {#if setupStatus.models_installed.includes('large-v3-turbo')}
                          <span class="installed-badge">✓ Installed</span>
                        {/if}
                      </div>
                      <span class="radio-desc">574 MB • Multilingual • Best accuracy</span>
                    </div>
                  </label>
                </div>
              </div>
            </div>
          </div>
        </div>
        
        {#if isDownloading}
          <div class="download-progress">
            <div class="progress-track">
              <div class="progress-fill" style="width: {downloadProgress}%"></div>
            </div>
            <div class="download-status">
              <span>{downloadMessage}</span>
              <span>{Math.round(downloadProgress)}%</span>
            </div>
          </div>
        {/if}
        
        <div class="setup-actions">
          <button 
            class="setup-button secondary" 
            onclick={skipSetup}
            disabled={isDownloading}
          >
            Maybe Later
          </button>
          <button 
            class="setup-button primary" 
            onclick={runSetup}
            disabled={isDownloading || !setupStatus.ffmpeg_installed}
          >
            {#if isDownloading}
              Downloading...
            {:else if setupStatus.models_installed.includes(selectedModelForSetup)}
              Enable
            {:else}
              Download & Enable
            {/if}
          </button>
        </div>
        
        {#if !setupStatus.ffmpeg_installed}
          <div class="setup-warning">
            ⚠️ FFmpeg must be installed first. 
            <a href="https://ffmpeg.org/download.html" target="_blank" rel="noopener">Download FFmpeg</a>
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  /* Settings and Setup dialog styles - imported from original */
  .settings-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1500;
    animation: fadeIn 0.3s ease;
  }
  
  @keyframes fadeIn {
    from { opacity: 0; }
    to { opacity: 1; }
  }
  
  .settings-modal {
    background: rgba(20, 20, 20, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    width: 90%;
    max-width: 700px;
    max-height: 80vh;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
    animation: slideUp 0.3s ease;
    display: flex;
    flex-direction: column;
  }
  
  @keyframes slideUp {
    from {
      transform: translateY(20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }
  
  .settings-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 2rem 2.5rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
  }
  
  .settings-header-content {
    display: flex;
    align-items: baseline;
    gap: 1rem;
  }
  
  .settings-header h2 {
    font-size: 1.75rem;
    font-weight: 600;
    margin: 0;
    color: #fff;
  }
  
  .settings-version {
    font-size: 0.875rem;
    font-weight: 500;
    color: rgba(255, 255, 255, 0.5);
    padding: 0.25rem 0.625rem;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 4px;
  }
  
  .settings-close {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    transition: all 0.2s ease;
  }
  
  .settings-close:hover {
    background: rgba(255, 255, 255, 0.15);
    border-color: rgba(255, 255, 255, 0.3);
    color: #fff;
    transform: scale(1.1);
  }
  
  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 2rem 2.5rem;
  }
  
  .settings-section {
    margin-bottom: 2rem;
  }
  
  .settings-section:last-child {
    margin-bottom: 0;
  }
  
  .settings-section h3 {
    font-size: 1.125rem;
    font-weight: 600;
    color: #fff;
    margin: 0 0 1.5rem 0;
    padding-bottom: 0.75rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }
  
  .settings-group {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    margin-bottom: 1.5rem;
  }
  
  .settings-item {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    padding: 1rem;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 8px;
    gap: 1rem;
  }
  
  .settings-item-label {
    flex: 1;
  }
  
  .settings-item-title {
    font-size: 0.9375rem;
    font-weight: 600;
    color: #fff;
    margin-bottom: 0.25rem;
  }
  
  .settings-item-desc {
    font-size: 0.8125rem;
    color: rgba(255, 255, 255, 0.6);
    line-height: 1.4;
  }
  
  .settings-item-status {
    display: flex;
    align-items: center;
  }
  
  .status-badge {
    padding: 0.375rem 0.75rem;
    border-radius: 6px;
    font-size: 0.8125rem;
    font-weight: 600;
    white-space: nowrap;
  }
  
  .status-badge.active {
    background: rgba(192, 101, 182, 0.2);
    color: #C065B6;
    border: 1px solid rgba(192, 101, 182, 0.3);
  }
  
  .status-badge.inactive {
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.5);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }
  
  .settings-item-action {
    display: flex;
    align-items: center;
  }
  
  .check-update-button {
    background: #fff;
    color: #000;
    border: none;
    padding: 0.625rem 1.25rem;
    font-size: 0.8125rem;
    font-weight: 600;
    cursor: pointer;
    transition: all 0.15s ease;
    border-radius: 6px;
    white-space: nowrap;
  }
  
  .check-update-button:hover {
    background: rgba(255, 255, 255, 0.9);
    transform: translateY(-1px);
  }
  
  .check-update-button:active {
    transform: translateY(0);
  }
  
  .check-update-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }
  
  .check-update-button:disabled:hover {
    background: #fff;
    transform: none;
  }
  
  .language-select {
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
    border: 1px solid rgba(255, 255, 255, 0.2);
    padding: 0.5rem 2.5rem 0.5rem 0.875rem;
    font-size: 0.8125rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.15s ease;
    border-radius: 6px;
    outline: none;
    width: auto;
    min-width: 160px;
    appearance: none;
    -webkit-appearance: none;
    -moz-appearance: none;
    background-image: url('data:image/svg+xml;charset=UTF-8,%3Csvg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"%3E%3Cpath fill="%23ffffff" d="M6 9L1 4h10z"/%3E%3C/svg%3E');
    background-repeat: no-repeat;
    background-position: right 0.75rem center;
    background-size: 12px;
  }
  
  .language-select:hover {
    background-color: rgba(255, 255, 255, 0.08);
    background-image: url('data:image/svg+xml;charset=UTF-8,%3Csvg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"%3E%3Cpath fill="%23ffffff" d="M6 9L1 4h10z"/%3E%3C/svg%3E');
    border-color: rgba(255, 255, 255, 0.3);
  }
  
  .language-select:focus {
    border-color: #C065B6;
    background-color: rgba(255, 255, 255, 0.08);
    background-image: url('data:image/svg+xml;charset=UTF-8,%3Csvg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"%3E%3Cpath fill="%23ffffff" d="M6 9L1 4h10z"/%3E%3C/svg%3E');
  }
  
  .language-select option {
    background: #1a1a1a;
    color: #fff;
    padding: 0.5rem;
  }
  
  /* Setup Dialog Styles */
  .setup-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.9);
    backdrop-filter: blur(20px);
    -webkit-backdrop-filter: blur(20px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2000;
    animation: fadeIn 0.3s ease;
  }
  
  .setup-modal {
    background: rgba(20, 20, 20, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    padding: 2.5rem;
    min-width: 500px;
    max-width: 600px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
    animation: slideUp 0.3s ease;
  }
  
  .setup-modal h2 {
    font-size: 1.75rem;
    font-weight: 600;
    margin: 0 0 1rem 0;
    color: #fff;
  }
  
  .setup-description {
    font-size: 0.9375rem;
    color: rgba(255, 255, 255, 0.7);
    line-height: 1.6;
    margin: 0 0 2rem 0;
  }
  
  .setup-checklist {
    margin-bottom: 2rem;
  }
  
  .setup-checklist h3 {
    font-size: 0.875rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: rgba(255, 255, 255, 0.6);
    margin: 0 0 1rem 0;
  }
  
  .setup-item {
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    border-radius: 8px;
    padding: 1rem;
    margin-bottom: 1rem;
  }
  
  .setup-item-header {
    display: flex;
    gap: 1rem;
    align-items: flex-start;
  }
  
  .checkbox {
    width: 24px;
    height: 24px;
    border: 2px solid rgba(255, 255, 255, 0.3);
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    margin-top: 2px;
  }
  
  .checkbox.checked {
    background: #C065B6;
    border-color: #C065B6;
  }
  
  .setup-item-info {
    flex: 1;
  }
  
  .setup-item-info.full-width {
    width: 100%;
  }
  
  .setup-item-title {
    font-size: 1rem;
    font-weight: 600;
    color: #fff;
    margin-bottom: 0.25rem;
  }
  
  .setup-item-desc {
    font-size: 0.875rem;
    color: rgba(255, 255, 255, 0.6);
  }
  
  .model-choices {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    margin-top: 1rem;
  }
  
  .model-radio {
    display: flex;
    align-items: flex-start;
    gap: 0.75rem;
    padding: 0.75rem;
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
    background: rgba(255, 255, 255, 0.02);
  }
  
  .model-radio:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.2);
  }
  
  .model-radio:has(input[type="radio"]:checked) {
    background: rgba(192, 101, 182, 0.12);
    border-color: rgba(192, 101, 182, 0.4);
  }
  
  .model-radio input[type="radio"] {
    margin-top: 2px;
    cursor: pointer;
  }
  
  .model-radio input[type="radio"]:checked + .radio-content {
    color: #fff;
  }
  
  .model-radio input[type="radio"]:checked + .radio-content .radio-title {
    color: #C065B6;
  }
  
  .radio-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }
  
  .radio-header {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 1rem;
  }
  
  .radio-title {
    font-size: 0.9375rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
  }
  
  .installed-badge {
    font-size: 0.75rem;
    font-weight: 600;
    color: #C065B6;
    background: rgba(192, 101, 182, 0.15);
    padding: 0.125rem 0.5rem;
    border-radius: 4px;
    border: 1px solid rgba(192, 101, 182, 0.3);
  }
  
  .radio-desc {
    font-size: 0.8125rem;
    color: rgba(255, 255, 255, 0.6);
  }
  
  .download-progress {
    margin-bottom: 2rem;
  }
  
  .progress-track {
    width: 100%;
    height: 8px;
    background: rgba(255, 255, 255, 0.1);
    border-radius: 4px;
    overflow: hidden;
    margin-bottom: 0.75rem;
  }
  
  .progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #C065B6, #8C77FF);
    border-radius: 4px;
    transition: width 0.3s ease;
    box-shadow: 0 0 10px rgba(192, 101, 182, 0.5);
  }
  
  .download-status {
    display: flex;
    justify-content: space-between;
    font-size: 0.875rem;
    color: rgba(255, 255, 255, 0.7);
  }
  
  .setup-actions {
    display: flex;
    gap: 1rem;
    justify-content: flex-end;
  }
  
  .setup-button {
    padding: 0.75rem 1.5rem;
    font-size: 0.875rem;
    font-weight: 600;
    border-radius: 6px;
    cursor: pointer;
    transition: all 0.15s ease;
    border: none;
  }
  
  .setup-button.secondary {
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.7);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }
  
  .setup-button.secondary:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }
  
  .setup-button.primary {
    background: #C065B6;
    color: #fff;
  }
  
  .setup-button.primary:hover {
    background: #a855a0;
    transform: translateY(-1px);
  }
  
  .setup-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none;
  }
  
  .setup-warning {
    margin-top: 1rem;
    padding: 1rem;
    background: rgba(255, 200, 0, 0.1);
    border: 1px solid rgba(255, 200, 0, 0.3);
    border-radius: 8px;
    font-size: 0.875rem;
    color: rgba(255, 255, 255, 0.9);
    line-height: 1.5;
  }
  
  .setup-warning a {
    color: #C065B6;
    text-decoration: underline;
  }
</style>
