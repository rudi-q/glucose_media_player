<script lang="ts">
  import { X } from 'lucide-svelte';
  import { subtitleStyleStore, activeSubtitleStyle } from '$lib/subtitle/subtitleStyleStore';
  import { SUBTITLE_PRESETS, type SubtitleStyle } from '$lib/subtitle/subtitlePresets';

  let { onClose }: { onClose: () => void } = $props();

  const FONT_FAMILIES = [
    { label: 'Inter', value: "'Inter Variable', Inter, system-ui, sans-serif" },
    { label: 'Georgia', value: "'Georgia', 'Times New Roman', serif" },
    { label: 'Courier', value: "'Courier New', Courier, monospace" },
    { label: 'Arial', value: "Arial, Helvetica, sans-serif" },
  ];

  const TEXT_COLORS = [
    { label: 'White', value: '#ffffff' },
    { label: 'Cream', value: '#f5e6c8' },
    { label: 'Yellow', value: '#ffd700' },
    { label: 'Cyan', value: '#00ffc8' },
    { label: 'Pink', value: '#ff80ab' },
    { label: 'Orange', value: '#ff9500' },
  ];

  function previewTextStyle(preset: SubtitleStyle): string {
    let style = `font-family: ${preset.fontFamily}; font-weight: ${preset.fontWeight}; color: ${preset.color}; letter-spacing: ${preset.letterSpacing}; font-size: 11px; line-height: 1.4;`;
    if (preset.textShadow !== 'none') {
      style += ` text-shadow: ${preset.textShadow};`;
    }
    if (preset.textStroke !== 'none') {
      const strokeColor = preset.color === '#000000' ? '#fff' : '#000';
      style += ` -webkit-text-stroke: 1px ${strokeColor};`;
    }
    return style;
  }

  function previewBgStyle(preset: SubtitleStyle): string {
    if (preset.backgroundType === 'none') return '';
    let style = `background-color: ${preset.backgroundColor};`;
    if (preset.backgroundType === 'frosted') {
      style += ' backdrop-filter: blur(8px); -webkit-backdrop-filter: blur(8px);';
    }
    return style;
  }
</script>

<div class="style-panel" role="dialog" aria-label="Subtitle style settings">
  <div class="panel-header">
    <span class="panel-title">Subtitle Style</span>
    <button class="close-btn" onclick={onClose} aria-label="Close"><X size={14} /></button>
  </div>

  <div class="preset-grid">
    {#each SUBTITLE_PRESETS as preset}
      <button
        class="preset-card"
        class:active={$subtitleStyleStore.presetId === preset.id}
        onclick={() => subtitleStyleStore.setPreset(preset.id)}
        title={preset.name}
      >
        <div
          class="preset-preview"
          class:pill={preset.backgroundType === 'pill'}
          class:box={preset.backgroundType === 'box'}
          class:stripe={preset.backgroundType === 'stripe'}
          class:frosted={preset.backgroundType === 'frosted'}
          style={previewBgStyle(preset)}
        >
          <span style={previewTextStyle(preset)}>Hello</span>
        </div>
        <span class="preset-name">{preset.name}</span>
      </button>
    {/each}
  </div>

  <div class="panel-divider"></div>

  <div class="customizations">
    <div class="control-row">
      <span class="control-label">Size</span>
      <div class="slider-row">
        <input
          type="range"
          min="14"
          max="42"
          step="1"
          value={$activeSubtitleStyle.fontSize}
          oninput={(e) => subtitleStyleStore.customize({ fontSize: +e.currentTarget.value })}
        />
        <span class="slider-value">{$activeSubtitleStyle.fontSize}px</span>
      </div>
    </div>

    <div class="control-row">
      <span class="control-label">Position</span>
      <div class="slider-row">
        <input
          type="range"
          min="4"
          max="90"
          step="1"
          value={$activeSubtitleStyle.position}
          oninput={(e) => subtitleStyleStore.customize({ position: +e.currentTarget.value })}
        />
        <span class="slider-value">{$activeSubtitleStyle.position}%</span>
      </div>
    </div>

    <div class="control-row">
      <span class="control-label">Color</span>
      <div class="color-swatches">
        {#each TEXT_COLORS as tc}
          <button
            class="color-swatch"
            class:active={$activeSubtitleStyle.color === tc.value}
            style="background-color: {tc.value};"
            title={tc.label}
            onclick={() => subtitleStyleStore.customize({ color: tc.value })}
          ></button>
        {/each}
        <label class="color-picker-label" title="Custom color">
          <input
            type="color"
            class="color-picker-input"
            value={$activeSubtitleStyle.color}
            oninput={(e) => subtitleStyleStore.customize({ color: e.currentTarget.value })}
          />
          <span class="color-picker-plus">+</span>
        </label>
      </div>
    </div>

    <div class="control-row">
      <span class="control-label">Font</span>
      <div class="font-buttons">
        {#each FONT_FAMILIES as ff}
          <button
            class="font-btn"
            class:active={$activeSubtitleStyle.fontFamily === ff.value}
            style="font-family: {ff.value};"
            onclick={() => subtitleStyleStore.customize({ fontFamily: ff.value })}
          >{ff.label}</button>
        {/each}
      </div>
    </div>
  </div>

  <div class="panel-divider"></div>

  <button class="reset-btn" onclick={() => subtitleStyleStore.reset()}>Reset to defaults</button>
</div>

<style>
  .style-panel {
    position: fixed;
    bottom: 6.5rem;
    right: 1rem;
    width: 320px;
    z-index: 500;
    background: rgba(14, 14, 16, 0.94);
    backdrop-filter: blur(24px);
    -webkit-backdrop-filter: blur(24px);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 14px;
    padding: 1rem;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.7);
    color: #fff;
    font-size: 0.8rem;
    font-family: inherit;
  }

  .panel-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 0.5rem;
  }

  .panel-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.9);
  }

  .close-btn {
    background: none;
    border: none;
    color: #fff;
    opacity: 0.6;
    cursor: pointer;
    padding: 4px;
    border-radius: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    transition: opacity 0.15s;
  }

  .close-btn:hover {
    opacity: 1;
  }

  .preset-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 0.5rem;
    margin: 0.75rem 0;
  }

  .preset-card {
    background: rgba(255, 255, 255, 0.04);
    border: 1.5px solid transparent;
    border-radius: 8px;
    padding: 0.5rem 0.25rem;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    transition: border-color 0.15s, background 0.15s;
  }

  .preset-card:hover {
    border-color: rgba(255, 255, 255, 0.15);
  }

  .preset-card.active {
    border-color: rgba(255, 255, 255, 0.5);
    background: rgba(255, 255, 255, 0.08);
  }

  .preset-card:focus-visible {
    outline: 2px solid rgba(255, 255, 255, 0.8);
    outline-offset: 2px;
  }

  .preset-preview {
    height: 36px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(0, 0, 0, 0.6);
    border-radius: 4px;
    width: 100%;
    overflow: hidden;
    position: relative;
  }

  .preset-preview.pill {
    border-radius: 999px;
    padding: 0 8px;
  }

  .preset-preview.stripe {
    border-radius: 0;
    width: 100%;
  }

  .preset-preview.frosted {
    border: 1px solid rgba(255, 255, 255, 0.15);
  }

  .preset-name {
    font-size: 0.65rem;
    color: rgba(255, 255, 255, 0.6);
    text-align: center;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
  }

  .panel-divider {
    height: 1px;
    background: rgba(255, 255, 255, 0.07);
    margin: 0.5rem 0;
  }

  .customizations {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
  }

  .control-row {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .control-label {
    font-size: 0.72rem;
    color: rgba(255, 255, 255, 0.5);
    min-width: 52px;
    flex-shrink: 0;
  }

  .slider-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    flex: 1;
  }

  .slider-row input[type='range'] {
    flex: 1;
    appearance: none;
    -webkit-appearance: none;
    height: 3px;
    background: rgba(255, 255, 255, 0.15);
    border-radius: 2px;
    outline: none;
  }

  .slider-row input[type='range']::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 14px;
    height: 14px;
    background: #fff;
    border-radius: 50%;
    cursor: pointer;
  }

  .slider-row input[type='range']::-moz-range-thumb {
    appearance: none;
    width: 14px;
    height: 14px;
    background: #fff;
    border-radius: 50%;
    cursor: pointer;
    border: none;
  }

  .slider-value {
    font-size: 0.7rem;
    color: rgba(255, 255, 255, 0.5);
    min-width: 36px;
    text-align: right;
  }

  .color-swatches {
    display: flex;
    gap: 0.3rem;
    flex-wrap: wrap;
    flex: 1;
    align-items: center;
  }

  .color-swatch {
    width: 18px;
    height: 18px;
    border-radius: 50%;
    border: 2px solid transparent;
    cursor: pointer;
    padding: 0;
    transition: border-color 0.15s;
  }

  .color-swatch.active {
    border-color: #fff;
  }

  .color-picker-label {
    position: relative;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    background: rgba(255, 255, 255, 0.1);
    cursor: pointer;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .color-picker-input {
    position: absolute;
    inset: 0;
    opacity: 0;
    cursor: pointer;
    width: 100%;
    height: 100%;
    border: none;
    padding: 0;
  }

  .color-picker-plus {
    font-size: 14px;
    color: rgba(255, 255, 255, 0.6);
    line-height: 1;
    pointer-events: none;
  }

  .font-buttons {
    display: flex;
    gap: 0.3rem;
    flex: 1;
    flex-wrap: wrap;
  }

  .font-btn {
    padding: 0.2em 0.5em;
    font-size: 0.7rem;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    background: rgba(255, 255, 255, 0.05);
    color: rgba(255, 255, 255, 0.7);
    cursor: pointer;
    transition: border-color 0.15s, background 0.15s, color 0.15s;
  }

  .font-btn.active {
    border-color: rgba(255, 255, 255, 0.5);
    background: rgba(255, 255, 255, 0.12);
    color: #fff;
  }

  .reset-btn {
    width: 100%;
    padding: 0.45rem;
    font-size: 0.75rem;
    color: rgba(255, 255, 255, 0.4);
    background: none;
    border: none;
    cursor: pointer;
    text-align: center;
    border-radius: 6px;
    transition: color 0.15s, background 0.15s;
  }

  .reset-btn:hover {
    color: rgba(255, 255, 255, 0.7);
    background: rgba(255, 255, 255, 0.05);
  }

  @media (max-width: 360px) {
    .style-panel {
      width: calc(100% - 1rem);
      right: 0.5rem;
    }
  }
</style>
