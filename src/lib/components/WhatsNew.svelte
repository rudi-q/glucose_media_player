<script lang="ts">
  import { fade } from "svelte/transition";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { Activity, Captions, Check, ExternalLink, Languages, PartyPopper, ScanFace } from "lucide-svelte";
  import { RELEASE_SUMMARY, type WhatsNewAction, type WhatsNewHighlight } from "$lib/utils/whatsNew";
  import { trapFocus } from "$lib/utils/trapFocus";

  // One-time "What's new" screen after an update (see whatsNew.ts).
  let {
    version,
    highlights,
    onAction,
    onClose,
  }: {
    version: string;
    highlights: WhatsNewHighlight[];
    // Returns true when the action is done in place (the button then shows "Done").
    onAction: (action: WhatsNewAction) => boolean;
    onClose: () => void;
  } = $props();

  const ICONS = { captions: Captions, languages: Languages, "scan-face": ScanFace, activity: Activity };
  let done = $state<Partial<Record<WhatsNewAction, boolean>>>({});
  let majorMinor = $derived(version.split(".").slice(0, 2).join("."));
  // An action that opens Settings closes this screen at once, without the fade: while it
  // faded out, its focus trap could pull focus back from Settings.
  let closingForAction = $state(false);

  function act(action: WhatsNewAction) {
    closingForAction = true;
    if (onAction(action)) {
      closingForAction = false;
      done = { ...done, [action]: true };
    }
  }

  $effect(() => {
    function onKey(e: KeyboardEvent) {
      if (e.key === "Escape") onClose();
    }
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<div class="wn-overlay" transition:fade={{ duration: closingForAction ? 0 : 250 }}>
  <div
    class="wn-modal"
    role="dialog"
    aria-modal="true"
    aria-labelledby="wn-title"
    aria-describedby={RELEASE_SUMMARY ? "wn-summary" : undefined}
    tabindex="-1"
    use:trapFocus
  >
    <div class="wn-header">
      <div class="wn-badge"><PartyPopper size={22} /></div>
      <h2 id="wn-title">What's new in Glucose {majorMinor.endsWith(".0") ? majorMinor.slice(0, -2) : majorMinor}</h2>
      {#if RELEASE_SUMMARY}
        <p id="wn-summary" class="wn-summary">{RELEASE_SUMMARY}</p>
      {/if}
    </div>

    <div class="wn-body">
      {#each highlights as h (h.action)}
        {@const Icon = ICONS[h.icon]}
        <div class="wn-item">
          <div class="wn-icon"><Icon size={18} /></div>
          <div class="wn-text">
            <div class="wn-item-title">{h.title}</div>
            <div class="wn-item-desc">{h.text}</div>
          </div>
          {#if done[h.action]}
            <span class="wn-done"><Check size={14} /> Done</span>
          {:else}
            <button class="wn-action" onclick={() => act(h.action)}>{h.actionLabel}</button>
          {/if}
        </div>
      {/each}
    </div>

    <div class="wn-actions">
      <button class="wn-cta" onclick={onClose}>Got it</button>
      <button
        class="wn-link"
        onclick={() => openUrl(`https://github.com/rudi-q/glucose_media_player/releases/tag/v${version}`).catch(console.error)}
      >
        Full release notes <ExternalLink size={12} />
      </button>
    </div>
  </div>
</div>

<style>
  .wn-overlay {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.85);
    backdrop-filter: blur(16px);
    -webkit-backdrop-filter: blur(16px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 2050;
  }

  .wn-modal:focus {
    outline: none;
  }

  .wn-modal {
    background: rgba(20, 20, 20, 0.98);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 16px;
    width: 90%;
    max-width: 600px;
    max-height: 88vh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.8);
  }

  .wn-header {
    text-align: center;
    padding: 2rem 2.25rem 1.25rem;
    border-bottom: 1px solid rgba(255, 255, 255, 0.07);
  }

  .wn-badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    border-radius: 12px;
    margin-bottom: 0.875rem;
    color: var(--color-accent);
    background: rgba(255, 255, 255, 0.05);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  h2 {
    font-size: 1.375rem;
    font-weight: 600;
    color: #fff;
    margin: 0 0 0.5rem;
  }

  .wn-summary {
    font-size: 0.875rem;
    line-height: 1.55;
    color: var(--color-text-muted);
    margin: 0;
  }

  .wn-body {
    padding: 1.25rem 2.25rem;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 0.625rem;
  }

  .wn-item {
    display: flex;
    align-items: center;
    gap: 0.875rem;
    padding: 0.875rem 1rem;
    border-radius: 12px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid rgba(255, 255, 255, 0.06);
  }

  .wn-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    height: 36px;
    border-radius: 10px;
    background: rgba(255, 255, 255, 0.05);
    color: var(--color-accent);
  }

  .wn-text {
    flex: 1;
    min-width: 0;
  }

  .wn-item-title {
    font-size: 0.9rem;
    font-weight: 600;
    color: #fff;
    margin-bottom: 0.15rem;
  }

  .wn-item-desc {
    font-size: 0.78rem;
    line-height: 1.45;
    color: rgba(255, 255, 255, 0.55);
  }

  .wn-action {
    flex-shrink: 0;
    font: inherit;
    font-size: 0.75rem;
    font-weight: 500;
    padding: 0.4rem 0.75rem;
    border-radius: 8px;
    border: 1px solid rgba(255, 255, 255, 0.15);
    background: rgba(255, 255, 255, 0.06);
    color: #fff;
    cursor: pointer;
    white-space: nowrap;
  }

  .wn-action:hover {
    background: rgba(255, 255, 255, 0.12);
  }

  .wn-done {
    flex-shrink: 0;
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    font-size: 0.75rem;
    color: #7ee2a8;
  }

  .wn-actions {
    padding: 1.25rem 2.25rem 1.5rem;
    border-top: 1px solid rgba(255, 255, 255, 0.07);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.75rem;
  }

  .wn-cta {
    width: 100%;
    padding: 0.8rem 2rem;
    background: var(--color-accent);
    color: #fff;
    border: none;
    border-radius: 10px;
    font-size: 0.9375rem;
    font-weight: 600;
    cursor: pointer;
  }

  .wn-cta:hover {
    opacity: 0.85;
  }

  .wn-link {
    display: inline-flex;
    align-items: center;
    gap: 0.3rem;
    background: none;
    border: none;
    color: var(--color-text-subtle);
    font-size: 0.8rem;
    cursor: pointer;
    text-decoration: underline;
    text-underline-offset: 3px;
  }
</style>
