<script lang="ts">
  import "$lib/styles/global.css";
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { listen, type UnlistenFn } from "@tauri-apps/api/event";
  import { onMount, setContext } from "svelte";
  import { fade } from "svelte/transition";
  import { goto } from "$app/navigation";
  import { page } from "$app/stores";
  import {
    X,
    Settings,
    Check,
    Loader2,
    Cpu,
    Download,
    Info,
    ChevronDown,
    ExternalLink,
    Code,
    Mail,
    Heart,
    Bug,
    Globe,
    Shield,
    ScrollText,
    Users,
    Star,
    Keyboard,
  } from "lucide-svelte";
  import Button from "$lib/components/Button.svelte";
  import UpdateManager, {
    type UpdateManagerAPI,
  } from "$lib/components/UpdateManager.svelte";
  import UpdateNotification from "$lib/components/UpdateNotification.svelte";
  import { getFormattedVersion } from "$lib/utils/version";
  import { isAudio } from "$lib/utils/mediaType";
  import {
    appSettings,
    setupStore,
    type SetupStatus,
  } from "$lib/stores/appStore";
  import { watchProgressStore } from "$lib/stores/watchProgressStore";

  let { children } = $props();

  let appReady = $state(false);

  // Shared state accessible via context
  let showSettings = $state(false);
  let showSetupDialog = $state(false);
  let selectedModelForSetup = $state<string>("tiny");
  let isDownloading = $state(false);
  let downloadProgress = $state(0);
  let downloadMessage = $state("");
  let isCheckingForUpdates = $state(false);
  let selectedTab = $state("ai"); // 'ai' | 'shortcuts' | 'updates' | 'community' | 'about'
  let modelsExpanded = $state(false);

  // Update manager
  let updateManager: UpdateManagerAPI;
  let isOnGallery = $state(true);

  // Initialize from localStorage if available
  let lastAutoCheckTime = $state<number>(0);
  if (typeof localStorage !== "undefined") {
    const stored = localStorage.getItem("lastAutoCheckTime");
    lastAutoCheckTime = stored ? parseInt(stored, 10) || 0 : 0;
  }

  // Subscribe to stores
  let settings = $state($appSettings);
  let setupStatus = $state<SetupStatus | null>(null);

  // Subscribe to setupStore changes
  setupStore.subscribe((status) => {
    setupStatus = status;
  });

  // Track current route
  $effect(() => {
    isOnGallery = $page.route.id === "/" || $page.route.id === null;
  });

  // Provide context for child routes
  setContext("showSettings", () => (showSettings = true));
  setContext("showSetupDialog", () => (showSetupDialog = true));
  setContext("setupStatus", () => setupStatus);

  onMount(() => {
    let disposed = false;
    const unsubs: UnlistenFn[] = [];

    // Register Tauri event listeners
    (async () => {
      const results = await Promise.allSettled([
        listen<string>("open-file", async (event) => {
          const encodedPath = encodeURIComponent(event.payload);
          await goto(
            isAudio(event.payload)
              ? `/audio/${encodedPath}`
              : `/player/${encodedPath}`,
          );
          invoke("mark_file_processed").catch(console.error);
        }),
        listen<string[]>("tauri://drag-drop", async (event) => {
          if (event.payload && event.payload.length > 0) {
            const encodedPath = encodeURIComponent(event.payload[0]);
            await goto(
              isAudio(event.payload[0])
                ? `/audio/${encodedPath}`
                : `/player/${encodedPath}`,
            );
          }
        }),
        // Listen for download progress
        listen<{
          downloaded: number;
          total: number;
          percentage: number;
          message: string;
        }>("download-progress", (event) => {
          downloadProgress = event.payload.percentage;
          downloadMessage = event.payload.message;
        }),
      ]);

      for (const r of results) {
        if (r.status === "fulfilled") {
          const un = r.value;
          if (disposed) {
            try {
              un();
            } catch (e) {
              console.error("Unlisten failed", e);
            }
          } else {
            unsubs.push(un);
          }
        } else {
          console.error("Failed to register Tauri listener:", r.reason);
        }
      }
    })();

    // Check setup status on first launch, then reveal the app
    checkSetupStatus().finally(() => {
      appReady = true;
    });

    // Notify backend that frontend is ready
    invoke("frontend_ready").catch(console.error);

    return () => {
      disposed = true;
      for (const un of unsubs) {
        try {
          un();
        } catch (e) {
          console.error("Unlisten failed", e);
        }
      }
    };
  });

  async function checkSetupStatus() {
    try {
      const status = await invoke<SetupStatus>("get_setup_status");
      setupStore.setStatus(status);

      // Show setup dialog on first launch if not completed
      if (!status.setup_completed && status.models_installed.length === 0) {
        setTimeout(() => {
          showSetupDialog = true;
        }, 1500);
      }
    } catch (err) {
      console.error("Failed to check setup status:", err);
    }
  }

  async function runSetup(modelSize?: string) {
    if (!setupStatus) return;

    const targetModel = modelSize || selectedModelForSetup;
    const isModelInstalled = setupStatus.models_installed.includes(targetModel);

    if (isModelInstalled) {
      try {
        await invoke("mark_setup_completed");
        await checkSetupStatus();
        showSetupDialog = false;
      } catch (err) {
        console.error("Failed to mark setup complete:", err);
      }
      return;
    }

    isDownloading = true;
    downloadProgress = 0;
    downloadMessage = "Starting download...";

    try {
      if (modelSize) {
        selectedModelForSetup = modelSize;
      }

      await invoke("download_whisper_model", {
        modelSize: targetModel,
      });

      await invoke("mark_setup_completed");
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
      console.error("Setup failed:", err);
      alert(`Setup failed: ${err}`);
      isDownloading = false;
      downloadProgress = 0;
      downloadMessage = "";
    }
  }

  function skipSetup() {
    showSetupDialog = false;
    invoke("mark_setup_completed").catch(console.error);
  }

  function handleAutoCheckStart() {
    lastAutoCheckTime = Date.now();
  }

  function handleAutoCheckTimeUpdate(time: number) {
    lastAutoCheckTime = time;
    if (typeof localStorage !== "undefined") {
      localStorage.setItem("lastAutoCheckTime", time.toString());
    }
  }
</script>

<!-- Update System -->
<UpdateManager
  bind:this={updateManager}
  disableAutoCheck={!isOnGallery}
  onAutoCheckStart={handleAutoCheckStart}
  onAutoCheckTimeUpdate={handleAutoCheckTimeUpdate}
  {lastAutoCheckTime}
/>
<UpdateNotification />

{#if !appReady}
  <div class="splash-screen" out:fade={{ duration: 250 }}>
    <img src="/logo-dark.svg" alt="glucose" class="splash-logo" />
  </div>
{/if}

{@render children()}

<!-- Settings Overlay -->
{#if showSettings}
  <!-- svelte-ignore a11y_click_events_have_key_events -->
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="settings-overlay"
    onclick={(e) => {
      if (e.target === e.currentTarget) showSettings = false;
    }}
  >
    <div class="settings-modal">
      <div class="settings-header">
        <div class="settings-header-content">
          <h2>Settings</h2>
        </div>
        <Button
          variant="secondary"
          size="sm"
          class="settings-close"
          onclick={() => (showSettings = false)}
          title="Close"
        >
          <X size={20} />
        </Button>
      </div>

      <div class="settings-layout">
        <div class="settings-sidebar">
          <button
            class="sidebar-tab"
            class:active={selectedTab === "ai"}
            onclick={() => (selectedTab = "ai")}
          >
            <Cpu size={18} />
            <span>AI Settings</span>
          </button>
          <button
            class="sidebar-tab"
            class:active={selectedTab === "shortcuts"}
            onclick={() => (selectedTab = "shortcuts")}
          >
            <Keyboard size={18} />
            <span>Shortcuts</span>
          </button>
          <button
            class="sidebar-tab"
            class:active={selectedTab === "updates"}
            onclick={() => (selectedTab = "updates")}
          >
            <Download size={18} />
            <span>Updates</span>
          </button>
          <button
            class="sidebar-tab"
            class:active={selectedTab === "community"}
            onclick={() => (selectedTab = "community")}
          >
            <Heart size={18} />
            <span>Community</span>
          </button>
          <button
            class="sidebar-tab"
            class:active={selectedTab === "about"}
            onclick={() => (selectedTab = "about")}
          >
            <Info size={18} />
            <span>About</span>
          </button>
        </div>

        <div class="settings-content" class:about-tab={selectedTab === "about"}>
          {#if selectedTab === "ai"}
            <div class="settings-section">
              <h3>AI Capability</h3>
              {#if setupStatus}
                <div class="requirements-grid">
                  <div
                    class="requirement-card"
                    class:ready={setupStatus.ffmpeg_installed}
                  >
                    <div class="requirement-info">
                      <span class="requirement-label">FFmpeg</span>
                      <span class="requirement-status">
                        {#if setupStatus.ffmpeg_installed}✓ Installed{:else}✗
                          Required{/if}
                      </span>
                    </div>
                  </div>
                  <div
                    class="requirement-card"
                    class:ready={setupStatus.models_installed.length > 0}
                  >
                    <div class="requirement-info">
                      <span class="requirement-label">AI Engine</span>
                      <span class="requirement-status">
                        {#if setupStatus.models_installed.length > 0}✓ Ready{:else}✗
                          Models Missing{/if}
                      </span>
                    </div>
                  </div>
                </div>
              {/if}
            </div>

            <div class="settings-section">
              {#if setupStatus}
                {@const models = [
                  {
                    id: "tiny",
                    name: "Lite Model",
                    desc: "75 MB • Fastest • Good accuracy",
                  },
                  {
                    id: "small",
                    name: "Optimal Model",
                    desc: "466 MB • Balanced • Very good accuracy",
                  },
                  {
                    id: "large-v3-turbo",
                    name: "Most Optimal Model",
                    desc: "574 MB • Multilingual • Best accuracy",
                  },
                ]}
                {@const activeModel = models.find(
                  (m) => m.id === selectedModelForSetup,
                )}

                <div
                  class="models-collapsible-card"
                  class:expanded={modelsExpanded}
                  onclick={() => (modelsExpanded = !modelsExpanded)}
                  onkeydown={(e) => {
                    if (e.key === 'Enter' || e.key === ' ' || e.key === 'Spacebar') {
                      e.preventDefault();
                      modelsExpanded = !modelsExpanded;
                    }
                  }}
                  role="button"
                  tabindex="0"
                >
                  <div class="collapsible-header">
                    <div class="collapsible-title-area">
                      <div class="collapsible-title-row">
                        <h3>AI Models</h3>
                        {#if activeModel}
                          <span class="active-model-pill">
                            {activeModel.name.split(" ")[0]}
                          </span>
                        {/if}
                      </div>
                      <p class="collapsible-desc">
                        Select the AI engine size used for subtitle generation.
                      </p>
                    </div>

                    <div class="collapsible-action-area">
                      {#if isDownloading}
                        <span class="downloading-badge-mini">
                          <Loader2 size={12} class="spinner" />
                          {Math.round(downloadProgress)}%
                        </span>
                      {/if}
                      <ChevronDown size={18} class="chevron-icon" />
                    </div>
                  </div>

                  {#if modelsExpanded}
                    <div
                      class="models-expanded-content"
                      onclick={(e) => e.stopPropagation()}
                    >
                      <div class="models-mgmt-list">
                        {#each models as model}
                          {@const isInstalled =
                            setupStatus.models_installed.includes(model.id)}
                          {@const isActive = selectedModelForSetup === model.id}
                          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
                          <div
                            class="model-mgmt-card"
                            class:active={isActive}
                            class:installed={isInstalled}
                            class:selectable={isInstalled && !isActive}
                            role={isInstalled && !isActive ? "button" : undefined}
                            tabindex={isInstalled && !isActive ? 0 : undefined}
                            onclick={() => {
                              if (isInstalled && !isActive)
                                selectedModelForSetup = model.id;
                            }}
                            onkeydown={(e) => {
                              if (isInstalled && !isActive && (e.key === "Enter" || e.key === " ")) {
                                e.preventDefault();
                                selectedModelForSetup = model.id;
                              }
                            }}
                          >
                            <div class="model-mgmt-info">
                              <div
                                class="model-mgmt-title"
                                class:active={isActive}
                              >
                                {model.name}
                                {#if isInstalled}
                                  <span class="installed-tag">Installed</span>
                                {/if}
                              </div>
                              <div class="model-mgmt-desc">{model.desc}</div>
                            </div>

                            <div class="model-mgmt-actions">
                              {#if isActive}
                                <div class="selection-check">
                                  <Check size={20} strokeWidth={2.5} />
                                </div>
                              {/if}
                              {#if !isInstalled}
                                <Button
                                  variant="primary"
                                  size="sm"
                                  loading={isDownloading &&
                                    selectedModelForSetup === model.id}
                                  disabled={isDownloading}
                                  onclick={(e) => {
                                    e.stopPropagation();
                                    runSetup(model.id);
                                  }}
                                >
                                  Download
                                </Button>
                              {/if}
                            </div>
                          </div>
                        {/each}
                      </div>

                      {#if isDownloading}
                        <div class="inline-progress-container">
                          <div class="inline-progress-message">
                            {downloadMessage}
                          </div>
                          <div class="inline-progress-track">
                            <div
                              class="inline-progress-fill"
                              style="width: {downloadProgress}%"
                            ></div>
                          </div>
                        </div>
                      {/if}
                    </div>
                  {/if}
                </div>
              {/if}
            </div>

            <div class="settings-section">
              <h3>Language Preferences</h3>
              <div class="settings-group">
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
                      onchange={(e) =>
                        appSettings.updateSubtitleLanguage(
                          (e.target as HTMLSelectElement).value,
                        )}
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
            </div>
          {:else if selectedTab === "updates"}
            <!-- App Updates Section -->
            <div class="settings-section">
              <h3>App Updates</h3>

              <div class="settings-group">
                <div class="settings-item">
                  <div class="settings-item-label">
                    <div class="settings-item-title">Current Version</div>
                    <div class="settings-item-desc">
                      Check if you are running the latest version
                    </div>
                  </div>
                  <div class="settings-item-action">
                    <span class="about-version-pill"
                      >{getFormattedVersion()}</span
                    >
                  </div>
                </div>

                <div class="settings-item">
                  <div class="settings-item-label">
                    <div class="settings-item-title">Check for Updates</div>
                    <div class="settings-item-desc">
                      Keep Glucose up to date with the latest features and
                      improvements
                    </div>
                  </div>
                  <div class="settings-item-action">
                    <Button
                      variant="primary"
                      loading={isCheckingForUpdates}
                      onclick={async () => {
                        if (updateManager && !isCheckingForUpdates) {
                          try {
                            isCheckingForUpdates = true;
                            const checkPromise =
                              updateManager.manualCheckForUpdates();
                            if (checkPromise) {
                              await checkPromise;
                            } else {
                              console.log("Update check already in progress");
                            }
                          } finally {
                            isCheckingForUpdates = false;
                          }
                        }
                      }}
                    >
                      Check Now
                    </Button>
                  </div>
                </div>

                <div class="settings-item">
                  <div class="settings-item-label">
                    <div class="settings-item-title">What's new?</div>
                    <div class="settings-item-desc">
                      See the latest changes and improvements
                    </div>
                  </div>
                  <div class="settings-item-action">
                    <Button
                      variant="secondary"
                      size="sm"
                      onclick={() => openUrl("https://glucose.media/changelog")}
                    >
                      View Changelog <ExternalLink size={14} />
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          {:else if selectedTab === "community"}
            <!-- Community Section -->
            <div class="settings-section community-section">
              <h3>Community</h3>

              <div class="settings-group">
                <div class="settings-item">
                  <div class="settings-item-label">
                    <div class="settings-item-title">Share Feedback</div>
                    <div class="settings-item-desc">
                      Tell us what you think or report issues you've encountered
                    </div>
                  </div>
                  <div
                    class="settings-item-action"
                    style="gap: 0.5rem; display: flex; flex-wrap: wrap; justify-content: flex-end;"
                  >
                    <Button
                      variant="secondary"
                      size="sm"
                      onclick={() => openUrl("mailto:support@glucose.media")}
                    >
                      <Mail size={14} /> Feedback
                    </Button>
                    <Button
                      variant="outline"
                      size="sm"
                      onclick={() =>
                        openUrl(
                          "https://github.com/rudi-q/glucose_media_player/issues",
                        )}
                    >
                      <Bug size={14} /> Report Bug
                    </Button>
                  </div>
                </div>

                <div class="settings-item">
                  <div class="settings-item-label">
                    <div class="settings-item-title">Support Glucose</div>
                    <div class="settings-item-desc">
                      Help keep the project free and open-source
                    </div>
                  </div>
                  <div
                    class="settings-item-action"
                    style="gap: 0.5rem; display: flex; flex-wrap: wrap; justify-content: flex-end;"
                  >
                    <Button
                      variant="primary"
                      size="sm"
                      onclick={() =>
                        openUrl("https://github.com/sponsors/rudi-q")}
                    >
                      <Heart size={14} fill="white" /> Sponsor
                    </Button>
                    <Button
                      variant="secondary"
                      size="sm"
                      onclick={() =>
                        openUrl(
                          "https://github.com/rudi-q/glucose_media_player",
                        )}
                    >
                      <Star size={14} /> Star
                    </Button>
                  </div>
                </div>
              </div>
            </div>
          {:else if selectedTab === "shortcuts"}
            <div class="settings-section">
              <div class="shortcuts-layout">
                <div class="shortcut-group">
                  <div class="shortcut-category-title">Playback</div>
                  <div class="shortcut-list">
                    <div class="shortcut-item">
                      <span class="shortcut-label">Play / Pause</span>
                      <div class="shortcut-keys">
                        <kbd class="key">Space</kbd>
                        <span class="key-sep">or</span>
                        <kbd class="key">K</kbd>
                      </div>
                    </div>
                    <div class="shortcut-item">
                      <span class="shortcut-label"
                        >Toggle View Mode (cinematic/fullscreen/pip)</span
                      >
                      <kbd class="key">F</kbd>
                    </div>
                    <div class="shortcut-item">
                      <span class="shortcut-label">Enter/Exit PiP</span>
                      <kbd class="key">P</kbd>
                    </div>
                  </div>
                </div>

                <div class="shortcut-group">
                  <div class="shortcut-category-title">Seeking</div>
                  <div class="shortcut-list">
                    <div class="shortcut-item">
                      <span class="shortcut-label">Skip Backward 5s</span>
                      <kbd class="key">←</kbd>
                    </div>
                    <div class="shortcut-item">
                      <span class="shortcut-label">Skip Forward 5s</span>
                      <kbd class="key">→</kbd>
                    </div>
                    <div class="shortcut-item">
                      <span class="shortcut-label">Jump to percentage</span>
                      <kbd class="key">0-9</kbd>
                    </div>
                  </div>
                </div>

                <div class="shortcut-group">
                  <div class="shortcut-category-title">Audio & Subtitles</div>
                  <div class="shortcut-list">
                    <div class="shortcut-item">
                      <span class="shortcut-label">Increase Volume</span>
                      <kbd class="key">↑</kbd>
                    </div>
                    <div class="shortcut-item">
                      <span class="shortcut-label">Decrease Volume</span>
                      <kbd class="key">↓</kbd>
                    </div>
                    <div class="shortcut-item">
                      <span class="shortcut-label">Toggle Mute</span>
                      <kbd class="key">M</kbd>
                    </div>
                    <div class="shortcut-item">
                      <span class="shortcut-label">Toggle Subtitles</span>
                      <div class="shortcut-keys">
                        <kbd class="key">C</kbd>
                        <span class="key-sep">or</span>
                        <kbd class="key">S</kbd>
                      </div>
                    </div>
                  </div>
                </div>

                <div class="shortcut-group">
                  <div class="shortcut-category-title">General</div>
                  <div class="shortcut-list">
                    <div class="shortcut-item">
                      <span class="shortcut-label">Go Back to Home</span>
                      <kbd class="key">Backspace</kbd>
                    </div>
                    <div class="shortcut-item">
                      <span class="shortcut-label">Close Application</span>
                      <kbd class="key">Esc</kbd>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          {:else if selectedTab === "about"}
            <!-- About Section -->
            <div class="settings-section about-section">
              <div class="about-hero">
                <div class="about-brand">
                  <img
                    src="/logo-dark.svg"
                    alt="Glucose Logo"
                    class="about-logo"
                  />
                  <p class="about-description">
                    A lightweight, AI-powered minimalist video player designed
                    for seamless playback and accessibility.
                  </p>
                </div>
                <span class="about-version-pill">{getFormattedVersion()}</span>
              </div>

              <div class="about-credits-area">
                <button
                  type="button"
                  class="credit-card"
                  onclick={() => openUrl("https://doubl.one")}
                >
                  <span class="credit-label">Brought to you by</span>
                  <span class="credit-value">
                    <img
                      src="/doublone-studios-logo.png"
                      alt="DoublOne Studios"
                      class="credit-logo"
                    />
                  </span>
                  <span class="credit-sub">Created by <span class="serif-name">Rudi K</span></span>
                  <span class="credit-location">Maintained in Finland 🤍</span>
                </button>
              </div>

              <div class="about-links-grid">
                <button
                  class="about-link-item"
                  onclick={() => openUrl("https://glucose.media")}
                >
                  <Globe size={14} /> Website
                </button>
                <button
                  class="about-link-item"
                  onclick={() => openUrl("https://glucose.media/privacy")}
                >
                  <Shield size={14} /> Privacy Policy
                </button>
                <button
                  class="about-link-item"
                  onclick={() =>
                    openUrl(
                      "https://github.com/rudi-q/glucose_media_player/blob/main/LICENSE",
                    )}
                >
                  <Info size={14} /> License
                </button>
                <button
                  class="about-link-item"
                  onclick={() => openUrl("https://glucose.media/terms")}
                >
                  <ScrollText size={14} /> Terms of Use
                </button>
              </div>
            </div>
          {/if}
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
        Automatically generate subtitles from video audio using AI. This feature
        requires downloading additional components.
      </p>

      {#if setupStatus}
        <div class="setup-checklist">
          <h3>Requirements</h3>

          <!-- FFmpeg -->
          <div class="setup-item">
            <div class="setup-item-header">
              <div
                class="checkbox"
                class:checked={setupStatus.ffmpeg_installed}
              >
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
                  <label
                    class="model-radio"
                    class:installed={setupStatus.models_installed.includes(
                      "tiny",
                    )}
                  >
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
                        {#if setupStatus.models_installed.includes("tiny")}
                          <span class="installed-badge">✓ Installed</span>
                        {/if}
                      </div>
                      <span class="radio-desc"
                        >75 MB • Fastest • Good accuracy</span
                      >
                    </div>
                  </label>

                  <label
                    class="model-radio"
                    class:installed={setupStatus.models_installed.includes(
                      "small",
                    )}
                  >
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
                        {#if setupStatus.models_installed.includes("small")}
                          <span class="installed-badge">✓ Installed</span>
                        {/if}
                      </div>
                      <span class="radio-desc"
                        >466 MB • Balanced • Very good accuracy</span
                      >
                    </div>
                  </label>

                  <label
                    class="model-radio"
                    class:installed={setupStatus.models_installed.includes(
                      "large-v3-turbo",
                    )}
                  >
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
                        {#if setupStatus.models_installed.includes("large-v3-turbo")}
                          <span class="installed-badge">✓ Installed</span>
                        {/if}
                      </div>
                      <span class="radio-desc"
                        >574 MB • Multilingual • Best accuracy</span
                      >
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
              <div
                class="progress-fill"
                style="width: {downloadProgress}%"
              ></div>
            </div>
            <div class="download-status">
              <span>{downloadMessage}</span>
              <span>{Math.round(downloadProgress)}%</span>
            </div>
          </div>
        {/if}

        <div class="setup-actions">
          <Button
            variant="secondary"
            onclick={skipSetup}
            disabled={isDownloading}
          >
            Maybe Later
          </Button>
          <Button
            variant="primary"
            loading={isDownloading}
            disabled={isDownloading || !setupStatus.ffmpeg_installed}
            onclick={() => runSetup()}
          >
            {#if isDownloading}
              Downloading...
            {:else if setupStatus.models_installed.includes(selectedModelForSetup)}
              Enable
            {:else}
              Download & Enable
            {/if}
          </Button>
        </div>

        {#if !setupStatus.ffmpeg_installed}
          <div class="setup-warning">
            ⚠️ FFmpeg must be installed first.
            <a
              href="https://ffmpeg.org/download.html"
              target="_blank"
              rel="noopener">Download FFmpeg</a
            >
          </div>
        {/if}
      {/if}
    </div>
  </div>
{/if}

<style>
  .splash-screen {
    position: fixed;
    inset: 0;
    z-index: 9999;
    background: #080a10;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .splash-logo {
    width: 120px;
    height: auto;
    opacity: 0.9;
  }

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
    from {
      opacity: 0;
    }
    to {
      opacity: 1;
    }
  }

  .settings-modal {
    background: rgba(20, 20, 20, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    width: 90%;
    max-width: 800px;
    height: 600px;
    max-height: 85vh;
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
    padding: 1.5rem 2rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.02);
  }

  .settings-header-content {
    display: flex;
    align-items: baseline;
    gap: 1rem;
  }

  .settings-header h2 {
    font-size: 1.25rem;
    font-weight: 600;
    margin: 0;
    color: #fff;
    opacity: 0.9;
  }

  :global(.settings-close) {
    width: 32px !important;
    height: 32px !important;
    min-width: 32px !important;
    padding: 0 !important;
    background: transparent !important;
    border: 1px solid rgba(255, 255, 255, 0.1) !important;
    border-radius: 8px !important;
    color: rgba(255, 255, 255, 0.5) !important;
  }

  .settings-layout {
    display: flex;
    flex: 1;
    overflow: hidden;
  }

  .settings-sidebar {
    width: 220px;
    background: rgba(0, 0, 0, 0.2);
    border-right: 1px solid rgba(255, 255, 255, 0.05);
    padding: 1.5rem 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .sidebar-tab {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    border: 1px solid transparent;
    border-radius: 10px;
    background: transparent;
    color: rgba(255, 255, 255, 0.6);
    font-size: 0.875rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    text-align: left;
  }

  .sidebar-tab:hover {
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
  }

  .sidebar-tab.active {
    background: rgba(192, 101, 182, 0.15);
    color: #c065b6;
    border-color: rgba(192, 101, 182, 0.3);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.1);
  }

  .settings-content {
    flex: 1;
    overflow-y: auto;
    padding: 1.5rem 2.5rem 2.5rem;
    background: radial-gradient(
      circle at top right,
      rgba(192, 101, 182, 0.03),
      transparent 40%
    );
  }

  .settings-content.about-tab {
    padding-bottom: 1rem;
    display: flex;
    flex-direction: column;
  }

  .settings-section {
    margin-bottom: 2rem;
    animation: fadeInContent 0.4s ease;
  }

  @keyframes fadeInContent {
    from {
      opacity: 0;
      transform: translateX(10px);
    }
    to {
      opacity: 1;
      transform: translateX(0);
    }
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
    letter-spacing: -0.01em;
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
    padding: 1.25rem;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    gap: 1rem;
    transition: all 0.2s ease;
  }

  .settings-item:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.1);
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
    color: rgba(255, 255, 255, 0.5);
    line-height: 1.5;
  }

  .settings-item-action {
    display: flex;
    align-items: center;
  }

  .language-select {
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
    border: 1px solid rgba(255, 255, 255, 0.1);
    padding: 0.625rem 2.5rem 0.625rem 1rem;
    font-size: 0.8125rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s ease;
    border-radius: 8px;
    outline: none;
    min-width: 180px;
    appearance: none;
    background-image: url('data:image/svg+xml;charset=UTF-8,%3Csvg xmlns="http://www.w3.org/2000/svg" width="12" height="12" viewBox="0 0 12 12"%3E%3Cpath fill="%23ffffff" d="M6 9L1 4h10z"/%3E%3C/svg%3E');
    background-repeat: no-repeat;
    background-position: right 1rem center;
    background-size: 10px;
  }

  .language-select:hover {
    background-color: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.3);
  }

  .language-select option {
    background: #1a1a1a;
    color: #fff;
    padding: 0.5rem;
  }

  /* About Section Styles */
  .about-section {
    display: flex;
    flex-direction: column;
    gap: 1rem;
    padding-top: 0;
    flex: 1;
  }

  .about-hero {
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    gap: 1rem;
    margin-bottom: 2rem;
  }

  .about-brand {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 1rem;
  }

  .about-logo {
    width: 112px;
    height: auto;
    filter: drop-shadow(0 0 20px rgba(192, 101, 182, 0.3));
    margin-top: 1rem;
  }

  .about-version-pill {
    font-size: 0.75rem;
    font-weight: 700;
    color: #c065b6;
    background: rgba(192, 101, 182, 0.1);
    border: 1px solid rgba(192, 101, 182, 0.2);
    padding: 0.25rem 0.75rem;
    border-radius: 20px;
    letter-spacing: 0.02em;
  }

  .about-description {
    font-size: 0.9375rem;
    color: rgba(255, 255, 255, 0.6);
    line-height: 1.6;
    max-width: 440px;
    margin: 0;
  }

  .about-credits-area {
    display: flex;
    justify-content: center;
  }

  .credit-card {
    appearance: none;
    font: inherit;
    color: inherit;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 12px;
    padding: 1.25rem 2rem;
    text-align: center;
    cursor: pointer;
    transition: all 0.2s ease;
    min-width: 280px;
  }

  .credit-card:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.15);
  }

  .credit-label {
    display: block;
    font-size: 0.6875rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.3);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 0.5rem;
  }

  .credit-value {
    margin: 0.5rem 0;
    display: flex;
    justify-content: center;
    align-items: center;
  }

  .credit-logo {
    height: 18px;
    width: auto;
    filter: brightness(0) invert(1);
    opacity: 0.9;
  }

  .credit-sub {
    display: block;
    font-size: 0.8125rem;
    color: #c065b6;
    font-weight: 500;
  }

  .credit-location {
    display: block;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.35);
    font-weight: 400;
    margin-top: 0.5rem;
  }

  .serif-name {
    font-family: "Instrument Serif", serif;
    font-style: italic;
    font-size: 1rem;
    opacity: 0.95;
    letter-spacing: 0.01em;
  }

  .about-links-grid {
    display: flex;
    justify-content: center;
    gap: 2rem;
    border-top: 1px solid rgba(255, 255, 255, 0.05);
    padding-top: 1.5rem;
    flex-wrap: wrap;
    margin-top: auto;
  }

  .about-link-item {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    font-size: 0.8125rem;
    color: rgba(255, 255, 255, 0.4);
    background: none;
    border: none;
    padding: 0.5rem 0;
    cursor: pointer;
    transition: color 0.2s ease;
    text-align: left;
  }

  .about-link-item:hover {
    color: #c065b6;
  }

  /* Inline AI Settings Styles */
  .requirements-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 1rem;
    margin-bottom: 1rem;
  }

  .requirement-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 1rem;
    display: flex;
    align-items: center;
    transition: all 0.2s ease;
  }

  .requirement-card.ready {
    border-color: rgba(192, 101, 182, 0.2);
    background: rgba(192, 101, 182, 0.05);
  }

  .requirement-info {
    display: flex;
    flex-direction: column;
    gap: 0.25rem;
  }

  .requirement-label {
    font-size: 0.75rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.4);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  /* Shortcuts Tab Styles */
  .shortcuts-layout {
    display: flex;
    flex-direction: column;
    gap: 1.25rem;
  }

  .shortcut-group {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    padding: 1.25rem;
  }

  .shortcut-category-title {
    font-size: 0.7rem;
    font-weight: 500;
    color: #c065b6;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    margin-bottom: 1rem;
    opacity: 0.9;
  }

  .shortcut-list {
    display: flex;
    flex-direction: column;
    gap: 0.875rem;
  }

  .shortcut-item {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .shortcut-label {
    font-size: 0.875rem;
    color: rgba(255, 255, 255, 0.7);
  }

  .shortcut-keys {
    display: flex;
    align-items: center;
    gap: 0.5rem;
  }

  .key {
    background: rgba(255, 255, 255, 0.08);
    border: 1px solid rgba(255, 255, 255, 0.15);
    padding: 0.25rem 0.5rem;
    border-radius: 6px;
    font-family: inherit;
    font-size: 0.75rem;
    font-weight: 400;
    color: #fff;
    min-width: 1.8rem;
    text-align: center;
    box-shadow: 0 2px 0 rgba(0, 0, 0, 0.25);
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .key-sep {
    font-size: 0.7rem;
    color: rgba(255, 255, 255, 0.3);
  }

  .requirement-status {
    font-size: 0.9375rem;
    font-weight: 600;
    color: #fff;
  }

  .requirement-card.ready .requirement-status {
    color: #c065b6;
  }

  .models-collapsible-card {
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.05);
    border-radius: 12px;
    overflow: hidden;
    transition: all 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    cursor: pointer;
  }

  .models-collapsible-card:hover {
    background: rgba(255, 255, 255, 0.04);
    border-color: rgba(255, 255, 255, 0.1);
  }

  .models-collapsible-card.expanded {
    background: rgba(255, 255, 255, 0.03);
    border-color: rgba(192, 101, 182, 0.2);
  }

  .collapsible-header {
    padding: 1.25rem;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .collapsible-title-area {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 0.25rem;
  }

  .collapsible-title-row {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .collapsible-title-row h3 {
    margin: 0;
    border: none;
    padding: 0;
    font-size: 1rem;
  }

  .collapsible-desc {
    font-size: 0.8125rem;
    color: rgba(255, 255, 255, 0.4);
    margin: 0;
  }

  .active-model-pill {
    font-size: 0.6875rem;
    font-weight: 700;
    color: #c065b6;
    background: rgba(192, 101, 182, 0.1);
    padding: 0.25rem 0.625rem;
    border-radius: 6px;
    border: 1px solid rgba(192, 101, 182, 0.2);
    text-transform: uppercase;
    letter-spacing: 0.02em;
  }

  .collapsible-action-area {
    display: flex;
    align-items: center;
    gap: 1rem;
  }

  .downloading-badge-mini {
    font-size: 0.75rem;
    font-weight: 600;
    color: #c065b6;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    background: rgba(192, 101, 182, 0.1);
    padding: 0.25rem 0.625rem;
    border-radius: 20px;
  }

  .models-expanded-content {
    padding: 0 1.25rem 1.25rem 1.25rem;
    animation: fadeIn 0.2s ease-out;
  }

  .models-mgmt-list {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .model-mgmt-card {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 1rem;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
    border-radius: 10px;
    gap: 1rem;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
  }

  .model-mgmt-card.selectable {
    cursor: pointer;
  }

  .model-mgmt-card.selectable:hover {
    background: rgba(255, 255, 255, 0.05);
    border-color: rgba(255, 255, 255, 0.12);
  }

  .model-mgmt-card.active {
    background: rgba(192, 101, 182, 0.08);
    border-color: rgba(192, 101, 182, 0.4);
    box-shadow: inset 0 0 20px rgba(192, 101, 182, 0.05);
  }

  .model-mgmt-info {
    flex: 1;
  }

  .model-mgmt-title {
    font-size: 0.875rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.7);
    margin-bottom: 0.125rem;
    display: flex;
    align-items: center;
    gap: 0.625rem;
    transition: color 0.2s ease;
  }

  .model-mgmt-title.active {
    color: #fff;
  }

  .selection-check {
    display: flex;
    align-items: center;
    justify-content: center;
    color: #c065b6;
    flex-shrink: 0;
  }

  .selection-check :global(svg) {
    stroke-width: 3px;
  }

  .installed-tag {
    font-size: 0.625rem;
    font-weight: 700;
    color: rgba(255, 255, 255, 0.3);
    background: rgba(255, 255, 255, 0.05);
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    text-transform: uppercase;
    display: flex;
    align-items: center;
    gap: 0.2rem;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .model-mgmt-desc {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.4);
  }

  .inline-progress-container {
    margin-top: 1.25rem;
    padding: 1rem;
    background: rgba(0, 0, 0, 0.2);
    border-radius: 10px;
    border: 1px solid rgba(255, 255, 255, 0.05);
  }

  .inline-progress-message {
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.5);
    margin-bottom: 0.75rem;
    display: flex;
    justify-content: space-between;
  }

  .inline-progress-track {
    height: 4px;
    background: rgba(255, 255, 255, 0.05);
    border-radius: 10px;
    overflow: hidden;
  }

  .inline-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, #c065b6, #8c77ff);
    border-radius: 10px;
    transition: width 0.3s ease;
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
    background: #c065b6;
    border-color: #c065b6;
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
    color: #c065b6;
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
    color: #c065b6;
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
    background: linear-gradient(90deg, #c065b6, #8c77ff);
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
    color: #c065b6;
    text-decoration: underline;
  }
</style>
