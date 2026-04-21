<script lang="ts">
  import { Loader2 } from "lucide-svelte"; // Refined variants
  import type { Snippet } from "svelte";

  interface Props {
    variant?: 'primary' | 'secondary' | 'outline' | 'white';
    size?: 'sm' | 'md' | 'lg';
    disabled?: boolean;
    loading?: boolean;
    type?: 'button' | 'submit' | 'reset';
    class?: string;
    title?: string;
    onclick?: (e: MouseEvent) => void;
    children?: Snippet;
  }

  let { 
    variant = 'primary', 
    size = 'md', 
    disabled = false, 
    loading = false, 
    type = 'button',
    class: className = '',
    title = '',
    onclick,
    children
  }: Props = $props();
</script>

<button
  {type}
  class="btn {variant} {size} {className}"
  disabled={disabled || loading}
  {onclick}
  {title}
>
  {#if loading}
    <Loader2 size={size === 'sm' ? 14 : 16} class="spinner" />
  {/if}
  {#if children}
    {@render children()}
  {/if}
</button>

<style>
  .btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
    font-weight: 500;
    cursor: pointer;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    white-space: nowrap;
    border: 1px solid transparent;
    outline: none;
    line-height: 1;
    box-sizing: border-box;
  }

  .btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
    transform: none !important;
    box-shadow: none !important;
  }

  /* Sizes */
  .sm {
    height: 32px;
    padding: 0 1rem;
    font-size: 0.75rem;
    border-radius: 8px;
    min-width: 80px;
  }

  .md {
    height: 40px;
    padding: 0 1.25rem;
    font-size: 0.8125rem;
    border-radius: 10px;
    min-width: 100px;
  }

  .lg {
    height: 48px;
    padding: 0 1.75rem;
    font-size: 0.9375rem;
    border-radius: 14px;
    min-width: 140px;
  }

  /* Variants */
  .primary {
    background: #C065B6;
    color: #fff;
  }

  .primary:hover:not(:disabled) {
    background: #a855a0;
    box-shadow: 0 4px 12px rgba(192, 101, 182, 0.3);
  }

  .secondary {
    background: rgba(255, 255, 255, 0.05);
    color: #fff;
    border-color: rgba(255, 255, 255, 0.1);
  }

  .secondary:hover:not(:disabled) {
    background: rgba(255, 255, 255, 0.1);
    border-color: rgba(255, 255, 255, 0.2);
  }

  .white {
    background: #fff;
    color: #000;
  }

  .white:hover:not(:disabled) {
    background: #f0f0f0;
    transform: translateY(-1.5px);
    box-shadow: 0 4px 15px rgba(255, 255, 255, 0.2);
  }

  .outline {
    background: rgba(192, 101, 182, 0.08);
    color: #C065B6;
    border-color: rgba(192, 101, 182, 0.3);
  }

  .outline:hover:not(:disabled) {
    background: rgba(192, 101, 182, 0.15);
    border-color: rgba(192, 101, 182, 0.5);
  }

  .btn:active:not(:disabled) {
    transform: translateY(0);
  }

  .btn :global(.spinner) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
