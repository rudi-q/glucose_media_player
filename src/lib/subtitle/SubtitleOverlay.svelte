<script lang="ts">
  import { activeSubtitleStyle } from '$lib/subtitle/subtitleStyleStore';
  import { parseVtt } from '$lib/subtitle/vttParser';

  let { vttContent, currentTime, enabled, videoElement }: {
    vttContent: string | null;
    currentTime: number;
    enabled: boolean;
    videoElement: HTMLVideoElement | undefined;
  } = $props();

  let cues = $derived(vttContent ? parseVtt(vttContent) : []);
  let activeCues = $derived(cues.filter(c => currentTime >= c.start && currentTime < c.end));
  let style = $derived($activeSubtitleStyle);

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

  function cueTextStyle(s: typeof style): string {
    let css = `font-family: ${s.fontFamily}; font-size: ${s.fontSize}px; font-weight: ${s.fontWeight}; color: ${s.color}; line-height: ${s.lineHeight}; letter-spacing: ${s.letterSpacing};`;
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

{#if enabled && activeCues.length > 0}
  <div
    class="subtitle-overlay"
    style={overlayPositionStyle}
    aria-live="polite"
    aria-atomic="true"
  >
    <div class="subtitle-cues" style="bottom: {style.position}%">
      {#each activeCues as cue (`${cue.start}-${cue.end}-${cue.text}`)}
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
</style>
