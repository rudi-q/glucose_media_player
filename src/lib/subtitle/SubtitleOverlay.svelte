<script lang="ts">
  import { activeSubtitleStyle } from '$lib/subtitle/subtitleStyleStore';
  import { parseVtt, type VttCue } from '$lib/subtitle/vttParser';
  import { choosePlacement, scoreVideoFrame, type SubtitlePlacement } from '$lib/subtitle/smartPosition';

  let { vttContent, currentTime, enabled, videoElement, liveCues = null, secondaryCues = null, placeholder = null, smartPosition = false }: {
    vttContent: string | null;
    currentTime: number;
    enabled: boolean;
    videoElement: HTMLVideoElement | undefined;
    // Live subtitles replace the loaded track while active.
    liveCues?: VttCue[] | null;
    // Shown smaller under the main cues, e.g. the original lines of translated subtitles.
    secondaryCues?: VttCue[] | null;
    // Shown when no cue is active, e.g. while live subtitles are still being generated.
    placeholder?: string | null;
    // Move cues to the top while the bottom of the picture is bright or busy.
    smartPosition?: boolean;
  } = $props();

  let cues = $derived(liveCues ?? (vttContent ? parseVtt(vttContent) : []));
  let activeCues = $derived(cues.filter(c => currentTime >= c.start && currentTime < c.end));
  let activeSecondary = $derived(
    (secondaryCues ?? []).filter(c => currentTime >= c.start && currentTime < c.end)
  );
  let showPlaceholder = $derived(activeCues.length === 0 && activeSecondary.length === 0 && !!placeholder);
  let style = $derived($activeSubtitleStyle);

  // Smart position: decided once when a new set of cues appears, never while one is shown.
  // Cues stay hidden for the few milliseconds the frame takes to score, so they do not
  // appear at the bottom and then jump.
  let placement = $state<SubtitlePlacement>('bottom');
  let deciding = $state(false);
  let cueKey = $derived(activeCues.map(c => `${c.start}|${c.end}`).join(','));
  let decideToken = 0;
  // Development only: what the last decision saw, shown in a corner of the video.
  let smartDebug = $state<string | null>(null);

  $effect(() => {
    const key = cueKey;
    const video = videoElement;
    if (!smartPosition) {
      placement = 'bottom';
      deciding = false;
      return;
    }
    if (!key || !video) return;
    const token = ++decideToken;
    deciding = true;
    const started = performance.now();
    // Never hold a cue back for long, even if the frame cannot be read.
    const fallback = setTimeout(() => {
      if (token === decideToken) deciding = false;
    }, 150);
    scoreVideoFrame(video).then(scores => {
      if (token !== decideToken) return;
      if (scores) {
        const next = choosePlacement(placement, scores.bottom, scores.top);
        if (import.meta.env.DEV) {
          const band = (b: typeof scores.bottom) =>
            `${b.busy.toFixed(2)} (light ${b.brightness.toFixed(2)}, detail ${b.edges.toFixed(2)}, text ${b.textRow.toFixed(2)})`;
          smartDebug = `${next.toUpperCase()} | bottom ${band(scores.bottom)} | top ${band(scores.top)} | ${(performance.now() - started).toFixed(0)} ms`;
        }
        placement = next;
      }
      clearTimeout(fallback);
      deciding = false;
    });
    return () => clearTimeout(fallback);
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

  // Secondary lines are this much smaller than the main ones.
  const SECONDARY_SCALE = 0.72;

  function cueTextStyle(s: typeof style, scale = 1): string {
    let css = `font-family: ${s.fontFamily}; font-size: ${s.fontSize * scale}px; font-weight: ${s.fontWeight}; color: ${s.color}; line-height: ${s.lineHeight}; letter-spacing: ${s.letterSpacing};`;
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

{#if enabled && (activeCues.length > 0 || activeSecondary.length > 0 || showPlaceholder)}
  <div
    class="subtitle-overlay"
    style={overlayPositionStyle}
    aria-live="polite"
    aria-atomic="true"
  >
    <div
      class="subtitle-cues"
      class:deciding
      style="{placement === 'top' ? 'top' : 'bottom'}: {style.position}%"
    >
      {#each activeCues as cue, i (`${i}-${cue.start}-${cue.end}-${cue.text}`)}
        <div
          class="cue-wrapper"
          class:pill={style.backgroundType === 'pill'}
          class:box={style.backgroundType === 'box'}
          class:stripe={style.backgroundType === 'stripe'}
          class:frosted={style.backgroundType === 'frosted'}
          style={cueWrapperStyle(style)}
        >
          <span class="cue-text" style={cueTextStyle(style)}>
            {@html sanitize(cue.text)}
          </span>
        </div>
      {/each}
      {#each activeSecondary as cue, i (`s-${i}-${cue.start}-${cue.end}-${cue.text}`)}
        <div
          class="cue-wrapper secondary"
          class:pill={style.backgroundType === 'pill'}
          class:box={style.backgroundType === 'box'}
          class:stripe={style.backgroundType === 'stripe'}
          class:frosted={style.backgroundType === 'frosted'}
          style={cueWrapperStyle(style)}
        >
          <span class="cue-text" style={cueTextStyle(style, SECONDARY_SCALE)}>
            {@html sanitize(cue.text)}
          </span>
        </div>
      {/each}
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
  .cue-wrapper.placeholder {
    border-radius: 6px;
    padding: 0.2em 0.6em;
    background-color: rgba(0, 0, 0, 0.5);
    color: rgba(255, 255, 255, 0.75);
    font-size: 0.95rem;
    font-style: italic;
  }
</style>
