<script lang="ts">
  import { onMount } from "svelte";
  import type { DragReorder } from "../util/dragReorder.svelte";

  let { reorder }: { reorder: DragReorder } = $props();

  let ghostEl: HTMLElement | null = $state(null);

  $effect(() => {
    reorder.attachGhost(ghostEl);
    return () => reorder.attachGhost(null);
  });

  let item = $derived(reorder.getGhostItem());
</script>

{#if item}
  <div class="drag-ghost" bind:this={ghostEl} aria-hidden="true">
    <div class="ghost-handle">
      <svg viewBox="0 0 24 24" fill="currentColor"><circle cx="9" cy="6" r="1.5"/><circle cx="15" cy="6" r="1.5"/><circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/><circle cx="9" cy="18" r="1.5"/><circle cx="15" cy="18" r="1.5"/></svg>
    </div>
    <div class="ghost-info">
      <div class="ghost-title">{item.title}</div>
      <div class="ghost-artist">{item.artist}</div>
    </div>
  </div>
{/if}

<style>
  .drag-ghost {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 9999;
    pointer-events: none;
    will-change: transform;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 14px;
    min-width: 240px;
    max-width: 360px;
    background: var(--bg-overlay);
    border: 1px solid var(--accent-dim);
    border-radius: var(--radius);
    box-shadow: 0 12px 32px rgba(0, 0, 0, 0.45), 0 0 0 1px var(--accent-dim);
    color: var(--text-primary);
    font-size: 0.85rem;
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    animation: ghostEnter 120ms ease-out;
  }

  @keyframes ghostEnter {
    from { opacity: 0; }
  }

  .ghost-handle {
    color: var(--accent);
    display: flex;
    align-items: center;
  }
  .ghost-handle svg { width: 14px; height: 14px; }

  .ghost-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
    flex: 1;
  }

  .ghost-title {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ghost-artist {
    color: var(--text-secondary);
    font-size: 0.75rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  :global(body.is-dragging) {
    cursor: grabbing !important;
  }
</style>
