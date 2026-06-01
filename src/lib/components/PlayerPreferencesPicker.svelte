<script lang="ts" module>
  import type { Play } from "lucide-svelte";

  type IconComponent = typeof Play;

  export type PlayerPreferenceOption<T extends string = string> = {
    value: T;
    icon: IconComponent;
    label: string;
    description: string;
  };
</script>

<script lang="ts" generics="T extends string">
  let {
    id,
    label,
    description,
    value = $bindable(),
    options,
  }: {
    id: string;
    label: string;
    description: string;
    value: T;
    options: PlayerPreferenceOption<T>[];
  } = $props();

  const labelId = $derived(`${id}-label`);
</script>

<div class="settings-section">
  <h3 id={labelId}>{label}</h3>
  <p class="settings-description">{description}</p>
  <div class="mode-picker" role="radiogroup" aria-labelledby={labelId}>
    {#each options as option (option.value)}
      {@const Icon = option.icon}
      <button
        class="mode-option"
        class:active={value === option.value}
        role="radio"
        aria-checked={value === option.value}
        onclick={() => (value = option.value)}
      >
        <Icon size={18} />
        <span class="mode-label">{option.label}</span>
        <span class="mode-desc">{option.description}</span>
      </button>
    {/each}
  </div>
</div>

<style>
  .settings-section {
    padding: 1.5rem 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.06);
  }

  .settings-section:last-child {
    border-bottom: none;
  }

  .settings-section h3 {
    font-size: 0.95rem;
    font-weight: 600;
    color: #fff;
    margin: 0 0 0.5rem;
  }

  .settings-description {
    font-size: 0.8rem;
    color: var(--color-text-muted);
    margin-bottom: 1rem;
  }

  .mode-picker {
    display: flex;
    flex-direction: column;
    gap: 0.5rem;
  }

  .mode-option {
    display: flex;
    align-items: center;
    gap: 0.875rem;
    padding: 0.875rem 1rem;
    background: transparent;
    border: 1px solid var(--color-border);
    border-radius: 8px;
    color: var(--color-text-muted);
    cursor: pointer;
    text-align: left;
    transition: border-color 0.15s, background 0.15s, color 0.15s;
  }

  .mode-option:hover {
    border-color: var(--color-border-strong);
    color: var(--color-text);
    background: rgba(255, 255, 255, 0.03);
  }

  .mode-option.active {
    border-color: var(--color-accent-border);
    background: var(--color-accent-subtle);
    color: var(--color-text);
  }

  .mode-label {
    font-size: 0.875rem;
    font-weight: 500;
    flex: 1;
  }

  .mode-desc {
    font-size: 0.75rem;
    color: var(--color-text-subtle);
  }
</style>
