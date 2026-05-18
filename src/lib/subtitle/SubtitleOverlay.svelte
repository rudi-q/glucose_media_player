<script lang="ts">
  import { activeSubtitleStyle } from '$lib/subtitle/subtitleStyleStore';
  import { parseVtt } from '$lib/subtitle/vttParser';

  let { vttContent, currentTime, enabled }: {
    vttContent: string | null;
    currentTime: number;
    enabled: boolean;
  } = $props();

  let cues = $derived(vttContent ? parseVtt(vttContent) : []);
  let activeCues = $derived(cues.filter(c => currentTime >= c.start && currentTime < c.end));
  let style = $derived($activeSubtitleStyle);

  function sanitize(text: string): string {
    const cleaned = text.replace(/<(?!\/?(?:b|i|u|br)\b)[^>]*>/gi, '');
    return cleaned.replace(/\n/g, '<br>');
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
  <div class="subtitle-overlay" style="bottom: {style.position}%" aria-live="polite" aria-atomic="true">
    {#each activeCues as cue (cue.start)}
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
{/if}

<style>
  .subtitle-overlay {
    position: absolute;
    left: 0; right: 0;
    display: flex; flex-direction: column; align-items: center;
    gap: 0.3rem;
    pointer-events: none;
    z-index: 10;
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
