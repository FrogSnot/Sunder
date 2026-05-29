<script lang="ts">
  import { toastState } from "../state/toast.svelte";
  import { fly, scale } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { quintOut } from "svelte/easing";
</script>

<div class="toast-container">
  {#each toastState.toasts as toast (toast.id)}
    <div
      class="toast {toast.type}"
      in:fly={{ x: 24, duration: 360, easing: quintOut }}
      out:scale={{ start: 0.92, duration: 220, easing: quintOut }}
      animate:flip={{ duration: 260, easing: quintOut }}
    >
      <span class="icon" aria-hidden="true">
        {#if toast.type === "error"}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="9"></circle>
            <line x1="12" y1="8" x2="12" y2="12.5"></line>
            <line x1="12" y1="16" x2="12.01" y2="16"></line>
          </svg>
        {:else}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="20 6 9 17 4 12"></polyline>
          </svg>
        {/if}
      </span>
      <div class="message">{toast.message}</div>
      <button class="close" onclick={() => toastState.remove(toast.id)} aria-label="Dismiss">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <line x1="18" y1="6" x2="6" y2="18"></line>
          <line x1="6" y1="6" x2="18" y2="18"></line>
        </svg>
      </button>
    </div>
  {/each}
</div>

<style>
  .toast-container {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 100%;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 11px 12px 11px 14px;
    width: 100%;
    border-radius: var(--radius-lg);
    background: color-mix(in srgb, var(--bg-elevated) 92%, transparent);
    backdrop-filter: blur(18px) saturate(150%);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.07));
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.02) inset;
    pointer-events: auto;
  }

  .toast.error {
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
  }

  .icon {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    flex-shrink: 0;
    border-radius: 9px;
    background: color-mix(in srgb, var(--success) 18%, transparent);
    color: var(--success);
  }

  .toast.error .icon {
    background: color-mix(in srgb, var(--error) 18%, transparent);
    color: var(--error);
  }

  .icon svg {
    width: 17px;
    height: 17px;
  }

  .message {
    font-size: 0.82rem;
    font-weight: 500;
    color: var(--text-primary);
    line-height: 1.35;
    word-break: break-word;
    flex-grow: 1;
  }

  .close {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 5px;
    display: grid;
    place-items: center;
    border-radius: 8px;
    flex-shrink: 0;
    transition: background 0.16s, color 0.16s;
  }

  .close:hover {
    background: var(--hover-overlay);
    color: var(--text-primary);
  }

  .close svg {
    width: 15px;
    height: 15px;
  }
</style>
