<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { AlertTriangle, Check, ChevronDown, Copy, Minus, X } from "lucide-svelte";

  // Full-window error screen, shared by the route error page (+error.svelte) and the
  // error boundary around each screen in +layout.svelte.
  let {
    status,
    message,
    screen,
    onHome,
    onRetry,
    retryLabel = "Try again",
  }: {
    status: number;
    message: string;
    // The route that failed, for the technical details.
    screen: string;
    onHome: () => void;
    // Omitted when retrying cannot help, e.g. a screen that does not exist.
    onRetry?: () => void;
    retryLabel?: string;
  } = $props();

  const ISSUES_URL = "https://github.com/rudi-q/glucose_media_player/issues/new";

  let notFound = $derived(status === 404);
  let title = $derived(notFound ? "This screen doesn't exist" : "Something went wrong");
  let description = $derived(
    notFound
      ? "Glucose tried to open a screen that isn't there. Head back to your library to keep going."
      : "Glucose hit an unexpected problem on this screen. Your library and watch progress are safe.",
  );
  let showDetails = $state(false);
  let copied = $state(false);
  const time = new Date().toISOString();

  let details = $derived(
    [`Status: ${status}`, `Error: ${message}`, `Screen: ${screen}`, `Time: ${time}`].join("\n"),
  );

  async function copyDetails() {
    try {
      await navigator.clipboard.writeText(details);
      copied = true;
      setTimeout(() => (copied = false), 2000);
    } catch (err) {
      console.error("Failed to copy error details:", err);
    }
  }

  function reportIssue() {
    const fence = "```";
    const body = `**What were you doing?**\n\n\n**Details**\n${fence}\n${details}\n${fence}`;
    const url = `${ISSUES_URL}?title=${encodeURIComponent(`Error: ${message}`)}&body=${encodeURIComponent(body)}`;
    openUrl(url).catch((err) => console.error("Failed to open issue page:", err));
  }

  async function minimizeApp() {
    await getCurrentWindow().minimize();
  }

  async function closeApp() {
    await getCurrentWindow().close();
  }
</script>

<div class="error-page">
  <div class="error-titlebar" data-tauri-drag-region>
    <div class="error-window-controls">
      <button class="window-btn" onclick={minimizeApp} data-tooltip="Minimize" aria-label="Minimize">
        <Minus size={15} />
      </button>
      <button class="window-btn window-btn-close" onclick={closeApp} data-tooltip="Close" aria-label="Close">
        <X size={15} />
      </button>
    </div>
  </div>

  <main class="error-content">
    <div class="error-icon" aria-hidden="true">
      <AlertTriangle size={28} strokeWidth={1.75} />
    </div>
    <h1>{title}</h1>
    <p class="error-description">{description}</p>

    <div class="error-actions">
      <button class="error-btn primary" onclick={onHome}>Back to library</button>
      {#if onRetry && !notFound}
        <button class="error-btn" onclick={onRetry}>{retryLabel}</button>
      {/if}
    </div>

    <button
      class="details-toggle"
      class:open={showDetails}
      aria-expanded={showDetails}
      onclick={() => (showDetails = !showDetails)}
    >
      Technical details
      <ChevronDown size={14} />
    </button>

    {#if showDetails}
      <div class="details-panel">
        <pre>{details}</pre>
        <div class="details-actions">
          <button class="details-btn" onclick={copyDetails}>
            {#if copied}<Check size={13} /> Copied{:else}<Copy size={13} /> Copy details{/if}
          </button>
          <button class="details-btn" onclick={reportIssue}>Report issue</button>
        </div>
      </div>
    {/if}
  </main>
</div>

<style>
  .error-page {
    position: fixed;
    inset: 0;
    display: flex;
    flex-direction: column;
    background: rgba(10, 10, 10, 0.92);
    border-radius: 12px;
    color: #fff;
  }

  .error-titlebar {
    display: flex;
    justify-content: flex-end;
    padding: 1rem 1rem 0;
    min-height: 52px;
  }

  .error-window-controls {
    display: flex;
    gap: 0.5rem;
  }

  .error-content {
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 0 2rem 4rem;
    max-width: 480px;
    margin: 0 auto;
  }

  .error-icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 56px;
    height: 56px;
    margin-bottom: 1.25rem;
    border-radius: 16px;
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
    color: rgba(255, 255, 255, 0.8);
  }

  h1 {
    font-size: 1.375rem;
    font-weight: 600;
    letter-spacing: -0.01em;
    margin-bottom: 0.5rem;
  }

  .error-description {
    font-size: 0.875rem;
    line-height: 1.6;
    color: rgba(255, 255, 255, 0.55);
    margin-bottom: 1.75rem;
  }

  .error-actions {
    display: flex;
    gap: 0.625rem;
    margin-bottom: 1.5rem;
  }

  .error-btn {
    padding: 0.625rem 1.125rem;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    font: inherit;
    font-size: 0.8125rem;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.2s ease, border-color 0.2s ease;
  }

  .error-btn:hover {
    background: rgba(255, 255, 255, 0.12);
    border-color: rgba(255, 255, 255, 0.25);
  }

  .error-btn.primary {
    background: #fff;
    border-color: #fff;
    color: #000;
  }

  .error-btn.primary:hover {
    background: rgba(255, 255, 255, 0.85);
  }

  .error-btn:focus-visible,
  .details-toggle:focus-visible,
  .details-btn:focus-visible {
    outline: 2px solid rgba(255, 255, 255, 0.6);
    outline-offset: 2px;
  }

  .details-toggle {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    background: none;
    border: none;
    color: rgba(255, 255, 255, 0.45);
    font: inherit;
    font-size: 0.75rem;
    cursor: pointer;
  }

  .details-toggle:hover {
    color: rgba(255, 255, 255, 0.75);
  }

  .details-toggle :global(svg) {
    transition: transform 0.2s ease;
  }

  .details-toggle.open :global(svg) {
    transform: rotate(180deg);
  }

  .details-panel {
    width: 100%;
    margin-top: 0.75rem;
    padding: 0.875rem;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.08);
    text-align: left;
  }

  pre {
    font-family: ui-monospace, "Cascadia Code", Consolas, monospace;
    font-size: 0.7rem;
    line-height: 1.6;
    color: rgba(255, 255, 255, 0.6);
    white-space: pre-wrap;
    word-break: break-word;
    user-select: text;
  }

  .details-actions {
    display: flex;
    gap: 0.5rem;
    margin-top: 0.75rem;
  }

  .details-btn {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    padding: 0.375rem 0.625rem;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.1);
    background: rgba(255, 255, 255, 0.04);
    color: rgba(255, 255, 255, 0.7);
    font: inherit;
    font-size: 0.7rem;
    cursor: pointer;
  }

  .details-btn:hover {
    background: rgba(255, 255, 255, 0.1);
    color: #fff;
  }
</style>
