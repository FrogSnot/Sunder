<script lang="ts">
  import { toastState } from "../state/toast.svelte";
  import { fade, fly } from "svelte/transition";
</script>

<div class="toast-container">
  {#each toastState.toasts as toast (toast.id)}
    <div
      class="toast {toast.type}"
      in:fly={{ y: 20, duration: 300 }}
      out:fade={{ duration: 200 }}
    >
      <div class="icon">
        {#if toast.type === "error"}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <circle cx="12" cy="12" r="10"></circle>
            <line x1="12" y1="8" x2="12" y2="12"></line>
            <line x1="12" y1="16" x2="12.01" y2="16"></line>
          </svg>
        {:else}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M22 11.08V12a10 10 0 1 1-5.93-9.14"></path>
            <polyline points="22 4 12 14.01 9 11.01"></polyline>
          </svg>
        {/if}
      </div>
      <div class="message">{toast.message}</div>
      <button class="close" onclick={() => toastState.remove(toast.id)} aria-label="Close">
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
    position: fixed;
    bottom: 2rem;
    right: 2rem;
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
    z-index: 9999;
    pointer-events: none;
  }

  .toast {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    padding: 0.75rem 1rem;
    border-radius: 0.5rem;
    background-color: var(--surface-1);
    border: 1px solid var(--border-color);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.5);
    pointer-events: auto;
    max-width: 400px;
    animation: slide-up 0.3s cubic-bezier(0.16, 1, 0.3, 1);
  }

  .toast.error {
    border-left: 4px solid var(--accent-color, #ff4d4d);
  }

  .toast.error .icon {
    color: var(--accent-color, #ff4d4d);
  }

  .toast.info .icon {
    color: #4CAF50;
  }

  .icon {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 20px;
    flex-shrink: 0;
  }

  .message {
    font-size: 0.875rem;
    color: var(--text-color);
    line-height: 1.4;
    word-break: break-word;
    flex-grow: 1;
  }

  .close {
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0.25rem;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 0.25rem;
    transition: background-color 0.2s, color 0.2s;
  }

  .close:hover {
    background-color: var(--surface-2);
    color: var(--text-color);
  }

  .close svg {
    width: 16px;
    height: 16px;
  }
</style>
