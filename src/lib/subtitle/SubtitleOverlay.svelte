<script lang="ts">
  import { ArrowUpDown, X } from 'lucide-svelte';
  import { activeSubtitleStyle } from '$lib/subtitle/subtitleStyleStore';
  import { DEFAULT_SECONDARY_SCALE } from '$lib/subtitle/subtitlePresets';
  import type { VttCue } from '$lib/subtitle/vttParser';
  import {
    choosePlacement,
    preloadFaceDetection,
    scoreVideoFrame,
    type SubtitlePlacement,
  } from '$lib/subtitle/smartPosition';

  // What the hover chip shows about a line.
  export interface SubtitleLineInfo {
    label: string;
    detail: string;
    // Timing shift in seconds; the cues passed in are already shifted by it.
    offset: number;
  }

  let {
    primaryCues,
    secondaryCues = null,
    currentTime,
    enabled,
    videoElement,
    placeholder = null,
    smartPosition = false,
    primaryInfo = null,
    secondaryInfo = null,
    interactive = false,
    onSwap,
    onNudge,
    onResetSync,
    onRemove,
    onLineClick,
    onHoverChange,
  }: {
    // The big line.
    primaryCues: VttCue[] | null;
    // The small line, shown under the big one.
    secondaryCues?: VttCue[] | null;
    currentTime: number;
    enabled: boolean;
    videoElement: HTMLVideoElement | undefined;
    // Shown when no cue is active, e.g. while live subtitles are still being generated.
    placeholder?: string | null;
    // Move cues to the top while the bottom of the picture is bright or busy.
    smartPosition?: boolean;
    // Set for each line that has a track; the hover chip names it.
    primaryInfo?: SubtitleLineInfo | null;
    secondaryInfo?: SubtitleLineInfo | null;
    // Whether the lines take the mouse (while the player controls show).
    interactive?: boolean;
    onSwap?: () => void;
    onNudge?: (line: 0 | 1, seconds: number) => void;
    onResetSync?: (line: 0 | 1) => void;
    onRemove?: (line: 0 | 1) => void;
    // A click on the subtitle text itself, which would otherwise have reached the video.
    onLineClick?: () => void;
    // Whether the mouse rests on a line or its chip, so the player keeps its controls
    // (and with them the chip) showing meanwhile.
    onHoverChange?: (hovering: boolean) => void;
  } = $props();

  // The line the mouse is on. A line can vanish under the mouse when its cue ends, which
  // fires no mouseleave, so that also ends the hover.
  let hoveredLine = $state<0 | 1 | null>(null);
  $effect(() => {
    if (
      !enabled ||
      !interactive ||
      (hoveredLine === 0 && activeCues.length === 0) ||
      (hoveredLine === 1 && activeSecondary.length === 0)
    ) {
      hoveredLine = null;
    }
  });
  let reportedHover = false;
  $effect(() => {
    const hovering = hoveredLine !== null;
    if (hovering !== reportedHover) {
      reportedHover = hovering;
      onHoverChange?.(hovering);
    }
  });

  const SYNC_STEP = 0.25;

  let activeCues = $derived((primaryCues ?? []).filter(c => currentTime >= c.start && currentTime < c.end));
  let activeSecondary = $derived(
    (secondaryCues ?? []).filter(c => currentTime >= c.start && currentTime < c.end)
  );
  let showPlaceholder = $derived(activeCues.length === 0 && activeSecondary.length === 0 && !!placeholder);
  // With two tracks on, the small line keeps its space while it has nothing to show, so
  // the big line does not move each time a small cue comes and goes.
  let dual = $derived(!!primaryInfo && !!secondaryInfo);
  let style = $derived($activeSubtitleStyle);
  let secondaryScale = $derived(style.secondaryScale ?? DEFAULT_SECONDARY_SCALE);

  function formatOffset(seconds: number): string {
    if (seconds === 0) return 'Sync 0.00s';
    return `${seconds > 0 ? '+' : '−'}${Math.abs(seconds).toFixed(2)}s`;
  }

  // Smart position: decided once when a new set of cues appears, never while one is shown.
  // Cues stay hidden while the frame is scored (usually 20-50 ms with face detection), so
  // they do not appear at the bottom and then jump.
  let placement = $state<SubtitlePlacement>('bottom');
  // Distance from the edge in smart mode, as a percentage: the presets' standard position.
  // The Position slider only applies to fixed placement.
  const SMART_EDGE_OFFSET = 8;
  let deciding = $state(false);
  // What a placement decision is made for: the main lines while any are showing, else the
  // secondary (original) lines, which can show on their own. Secondary lines changing under
  // a main line must not start a new decision, which would hide or move the main line while
  // it is on screen. The text counts too, since live subtitles can replace a line's text
  // while keeping its timing.
  let cueKey = $derived(
    (activeCues.length > 0 ? activeCues : activeSecondary).map(c => `${c.start}|${c.end}|${c.text}`).join('\n')
  );
  let decideToken = 0;
  // Development only: what the last decision saw, shown in a corner of the video.
  let smartDebug = $state<string | null>(null);

  $effect(() => {
    const video = videoElement;
    if (!smartPosition || !video) return;
    const preload = () => preloadFaceDetection(video);
    preload();
    video.addEventListener('loadedmetadata', preload);
    return () => video.removeEventListener('loadedmetadata', preload);
  });

  $effect(() => {
    const key = cueKey;
    const video = videoElement;
    if (!smartPosition) {
      placement = 'bottom';
      deciding = false;
      return;
    }
    if (!key || !video) {
      deciding = false;
      return;
    }
    const token = ++decideToken;
    deciding = true;
    const started = performance.now();
    // Never hold a cue back for long, even if the frame cannot be read. A result that
    // arrives after this is dropped, so it cannot move the cue while it is shown.
    const fallback = setTimeout(() => {
      if (token !== decideToken) return;
      decideToken++;
      deciding = false;
    }, 200);
    scoreVideoFrame(video).then(scores => {
      if (token !== decideToken) return;
      if (scores) {
        const next = choosePlacement(placement, scores.bottom, scores.top);
        if (import.meta.env.DEV) {
          const band = (b: typeof scores.bottom) =>
            `${b.busy.toFixed(2)} (light ${b.brightness.toFixed(2)}, detail ${b.edges.toFixed(2)}, text ${b.textRow.toFixed(2)}, faces ${b.faces})`;
          smartDebug = `${next.toUpperCase()} | bottom ${band(scores.bottom)} | top ${band(scores.top)} | ${(performance.now() - started).toFixed(0)} ms`;
        }
        placement = next;
      }
      clearTimeout(fallback);
      deciding = false;
    });
    // A new cue, a switch to Fixed or a new video makes this result stale.
    return () => {
      clearTimeout(fallback);
      if (token === decideToken) decideToken++;
    };
  });

  // Track the exact displayed video content rect within .video-container.
  // Needed because cinematic mode constrains the video element (max-width/max-height)
  // and fullscreen mode letterboxes inside the element via object-fit: contain.
  let contentRect = $state<{ left: number; top: number; width: number; height: number } | null>(null);

  $effect(() => {
    const el = videoElement;
    if (!el) { contentRect = null; return; }

    function compute() {
      if (!el) return;
      const parent = el.parentElement;
      if (!parent) return;

      const containerRect = parent.getBoundingClientRect();
      const videoRect = el.getBoundingClientRect();

      const elLeft = videoRect.left - containerRect.left;
      const elTop = videoRect.top - containerRect.top;
      const elW = videoRect.width;
      const elH = videoRect.height;

      // No video track (audio-only): cover the full container
      if (!el.videoWidth || !el.videoHeight) {
        contentRect = { left: 0, top: 0, width: containerRect.width, height: containerRect.height };
        return;
      }

      // Compute the object-fit: contain content rect within the video element
      const videoAspect = el.videoWidth / el.videoHeight;
      const elAspect = elW / elH;
      let contentW: number, contentH: number, offsetX: number, offsetY: number;

      if (videoAspect > elAspect) {
        contentW = elW;
        contentH = elW / videoAspect;
        offsetX = 0;
        offsetY = (elH - contentH) / 2;
      } else {
        contentH = elH;
        contentW = elH * videoAspect;
        offsetX = (elW - contentW) / 2;
        offsetY = 0;
      }

      contentRect = { left: elLeft + offsetX, top: elTop + offsetY, width: contentW, height: contentH };
    }

    const ro = new ResizeObserver(compute);
    const parent = el.parentElement;
    if (parent) ro.observe(parent);
    ro.observe(el);
    el.addEventListener('loadedmetadata', compute);
    compute();

    return () => {
      ro.disconnect();
      el.removeEventListener('loadedmetadata', compute);
    };
  });

  let overlayPositionStyle = $derived(
    contentRect
      ? `left: ${contentRect.left}px; top: ${contentRect.top}px; width: ${contentRect.width}px; height: ${contentRect.height}px`
      : 'left: 0; right: 0; top: 0; bottom: 0'
  );

  const allowedCueTags = new Set(['B', 'I', 'U', 'BR']);

  function escapeHtml(text: string): string {
    return text
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;')
      .replace(/"/g, '&quot;')
      .replace(/'/g, '&#39;');
  }

  function serializeCueNode(node: Node): string {
    if (node.nodeType === Node.TEXT_NODE) {
      return escapeHtml(node.textContent ?? '');
    }

    if (node.nodeType !== Node.ELEMENT_NODE) {
      return '';
    }

    const element = node as HTMLElement;
    const tagName = element.tagName;
    const children = Array.from(element.childNodes).map(serializeCueNode).join('');

    if (!allowedCueTags.has(tagName)) {
      return children;
    }

    if (tagName === 'BR') {
      return '<br>';
    }

    const normalizedTag = tagName.toLowerCase();
    return `<${normalizedTag}>${children}</${normalizedTag}>`;
  }

  function sanitize(text: string): string {
    if (typeof document === 'undefined') {
      return escapeHtml(text).replace(/\n/g, '<br>');
    }
    const template = document.createElement('template');
    template.innerHTML = text.replace(/\n/g, '<br>');
    return Array.from(template.content.childNodes).map(serializeCueNode).join('');
  }

  function cueTextStyle(s: typeof style, secondary = false): string {
    const scale = secondary ? secondaryScale : 1;
    const color = secondary && s.secondaryColor ? s.secondaryColor : s.color;
    let css = `font-family: ${s.fontFamily}; font-size: ${s.fontSize * scale}px; font-weight: ${s.fontWeight}; color: ${color}; line-height: ${s.lineHeight}; letter-spacing: ${s.letterSpacing};`;
    if (s.textShadow !== 'none') css += ` text-shadow: ${s.textShadow};`;
    if (s.textStroke !== 'none') css += ` -webkit-text-stroke: ${s.textStroke};`;
    return css;
  }

  function cueWrapperStyle(s: typeof style): string {
    let css = '';
    if (s.backgroundType !== 'none') css += ` background-color: ${s.backgroundColor};`;
    if (s.backgroundType === 'frosted') css += ` backdrop-filter: blur(16px); -webkit-backdrop-filter: blur(16px);`;
    if (s.backgroundType === 'stripe') css += ` width: 100%;`;
    return css.trim();
  }
</script>

{#if import.meta.env.DEV && smartPosition && smartDebug}
  <div class="smart-debug" style={overlayPositionStyle}>
    <span>{smartDebug}</span>
  </div>
{/if}

{#snippet chip(line: 0 | 1, info: SubtitleLineInfo)}
  <!-- Shortcuts for this line; everything here is also in the subtitles menu. -->
  <div class="line-chip" role="toolbar" aria-label="{info.label} subtitles">
    <span class="chip-label">{info.label}<span class="chip-detail"> · {info.detail}</span></span>
    {#if dual}
      <span class="chip-sep"></span>
      <button
        class="chip-btn"
        title="Swap big and small (Alt+C)"
        aria-label="Swap big and small lines"
        onclick={(e) => { e.stopPropagation(); onSwap?.(); }}
      ><ArrowUpDown size={14} /></button>
    {/if}
    <span class="chip-sep"></span>
    <button
      class="chip-btn"
      title="Show {SYNC_STEP}s earlier"
      aria-label="Show {SYNC_STEP} seconds earlier"
      onclick={(e) => { e.stopPropagation(); onNudge?.(line, -SYNC_STEP); }}
    >−</button>
    <button
      class="chip-btn chip-sync"
      class:shifted={info.offset !== 0}
      title={info.offset !== 0 ? 'Reset timing' : 'Timing of this line'}
      onclick={(e) => { e.stopPropagation(); onResetSync?.(line); }}
    >{formatOffset(info.offset)}</button>
    <button
      class="chip-btn"
      title="Show {SYNC_STEP}s later"
      aria-label="Show {SYNC_STEP} seconds later"
      onclick={(e) => { e.stopPropagation(); onNudge?.(line, SYNC_STEP); }}
    >+</button>
    <span class="chip-sep"></span>
    <button
      class="chip-btn"
      title="Turn off this line"
      aria-label="Turn off {info.label} subtitles"
      onclick={(e) => { e.stopPropagation(); onRemove?.(line); }}
    ><X size={14} /></button>
  </div>
{/snippet}

{#snippet cueBox(cue: VttCue, secondary: boolean)}
  <div
    class="cue-wrapper"
    class:secondary
    class:pill={style.backgroundType === 'pill'}
    class:box={style.backgroundType === 'box'}
    class:stripe={style.backgroundType === 'stripe'}
    class:frosted={style.backgroundType === 'frosted'}
    style={cueWrapperStyle(style)}
  >
    <span class="cue-text" style={cueTextStyle(style, secondary)}>
      {@html sanitize(cue.text)}
    </span>
  </div>
{/snippet}

{#if enabled && (activeCues.length > 0 || activeSecondary.length > 0 || showPlaceholder)}
  <div class="subtitle-overlay" style={overlayPositionStyle}>
    <div
      class="subtitle-cues"
      class:deciding
      style="{placement === 'top' ? 'top' : 'bottom'}: {smartPosition ? SMART_EDGE_OFFSET : style.position}%"
    >
      {#if activeCues.length > 0}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div
          class="subtitle-line big"
          class:interactive={interactive && !!primaryInfo}
          onclick={() => onLineClick?.()}
          onmouseenter={() => (hoveredLine = 0)}
          onmouseleave={() => { if (hoveredLine === 0) hoveredLine = null; }}
        >
          {#if interactive && primaryInfo}{@render chip(0, primaryInfo)}{/if}
          <div class="line-cues" aria-live="polite" aria-atomic="true">
            {#each activeCues as cue, i (`${i}-${cue.start}-${cue.end}-${cue.text}`)}
              {@render cueBox(cue, false)}
            {/each}
          </div>
        </div>
      {/if}
      {#if activeSecondary.length > 0}
        <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
        <div
          class="subtitle-line small"
          class:interactive={interactive && !!secondaryInfo}
          onclick={() => onLineClick?.()}
          onmouseenter={() => (hoveredLine = 1)}
          onmouseleave={() => { if (hoveredLine === 1) hoveredLine = null; }}
        >
          {#if interactive && secondaryInfo}{@render chip(1, secondaryInfo)}{/if}
          <div class="line-cues" aria-live="polite" aria-atomic="true">
            {#each activeSecondary as cue, i (`s-${i}-${cue.start}-${cue.end}-${cue.text}`)}
              {@render cueBox(cue, true)}
            {/each}
          </div>
        </div>
      {:else if dual && activeCues.length > 0}
        <!-- The small line's space, held while it has nothing to show. -->
        <!-- Same classes and text style as a real small cue, so it takes the same height
             with each background style's padding and border. -->
        <div class="reserved" aria-hidden="true">
          {@render cueBox({ start: 0, end: 0, text: ' ' }, true)}
        </div>
      {/if}
      {#if showPlaceholder}
        <div class="cue-wrapper placeholder">
          <span class="cue-text">{placeholder}</span>
        </div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .subtitle-overlay {
    position: absolute;
    pointer-events: none;
    z-index: 10;
    overflow: hidden;
  }

  .subtitle-cues {
    position: absolute;
    left: 0;
    right: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    padding: 0 6%;
  }

  .smart-debug {
    position: absolute;
    pointer-events: none;
    z-index: 11;
  }

  .smart-debug span {
    position: absolute;
    left: 8px;
    top: 8px;
    padding: 2px 6px;
    border-radius: 4px;
    background: rgba(0, 0, 0, 0.7);
    color: #7fffb0;
    font: 11px/1.4 ui-monospace, Consolas, monospace;
  }

  .subtitle-cues.deciding {
    visibility: hidden;
  }

  .cue-wrapper {
    display: inline-flex;
    align-items: center;
    max-width: 80%;
    text-align: center;
  }

  .cue-wrapper.pill { border-radius: 6px; padding: 0.2em 0.6em; }
  .cue-wrapper.box { border-radius: 4px; padding: 0.25em 0.7em; }
  .cue-wrapper.stripe { padding: 0.3em 1em; max-width: 100%; width: 100%; border-radius: 0; justify-content: center; }
  .cue-wrapper.frosted { border-radius: 8px; padding: 0.25em 0.7em; border: 1px solid rgba(255,255,255,0.15); }
  .cue-text { display: block; text-align: center; }
  .cue-wrapper.secondary { opacity: 0.85; }
  .reserved {
    display: flex;
    justify-content: center;
    width: 100%;
    visibility: hidden;
  }

  /* One subtitle line: its cues, plus the chip shown while the mouse rests on them. */
  .subtitle-line {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    width: 100%;
  }
  .line-cues {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    width: 100%;
  }
  .subtitle-line.interactive .cue-wrapper {
    pointer-events: auto;
    cursor: default;
  }
  .line-chip {
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 2px;
    padding: 3px;
    border-radius: 9px;
    background: rgba(18, 18, 22, 0.88);
    backdrop-filter: blur(14px);
    -webkit-backdrop-filter: blur(14px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5);
    color: rgba(255, 255, 255, 0.9);
    font: 500 12px/1 'Inter Variable', Inter, system-ui, sans-serif;
    white-space: nowrap;
    pointer-events: auto;
    z-index: 2;
    opacity: 0;
    visibility: hidden;
    /* Hides at once; shows after a short rest, so a mouse passing over does not flash it. */
    transition: opacity 0.15s ease, visibility 0s linear 0.15s;
  }
  .subtitle-line.big .line-chip { bottom: 100%; margin-bottom: 6px; }
  .subtitle-line.small .line-chip { top: 100%; margin-top: 6px; }
  /* Bridges the gap to the chip, so moving the mouse onto it keeps it open. */
  .line-chip::after {
    content: '';
    position: absolute;
    left: 0;
    right: 0;
    height: 8px;
  }
  .subtitle-line.big .line-chip::after { top: 100%; }
  .subtitle-line.small .line-chip::after { bottom: 100%; }
  .subtitle-line.interactive:has(.cue-wrapper:hover) .line-chip,
  .line-chip:hover,
  .line-chip:focus-within {
    opacity: 1;
    visibility: visible;
    transition: opacity 0.15s ease 0.3s, visibility 0s linear 0.3s;
  }
  .chip-label {
    padding: 0 8px 0 6px;
    max-width: 260px;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .chip-detail { color: rgba(255, 255, 255, 0.55); }
  .chip-sep {
    width: 1px;
    height: 16px;
    margin: 0 2px;
    background: rgba(255, 255, 255, 0.12);
  }
  .chip-btn {
    display: grid;
    place-items: center;
    min-width: 26px;
    height: 26px;
    padding: 0 6px;
    border: 0;
    border-radius: 6px;
    background: transparent;
    color: rgba(255, 255, 255, 0.7);
    font: inherit;
    cursor: pointer;
  }
  .chip-btn:hover { background: rgba(255, 255, 255, 0.1); color: #fff; }
  .chip-btn:focus-visible { outline: 2px solid var(--color-accent, #a78bfa); outline-offset: 1px; }
  .chip-sync {
    min-width: 72px;
    font-variant-numeric: tabular-nums;
    color: #fff;
  }
  .chip-sync.shifted { color: #f2b45c; }
  .cue-wrapper.placeholder {
    border-radius: 6px;
    padding: 0.2em 0.6em;
    background-color: rgba(0, 0, 0, 0.5);
    color: rgba(255, 255, 255, 0.75);
    font-size: 0.95rem;
    font-style: italic;
  }
</style>
