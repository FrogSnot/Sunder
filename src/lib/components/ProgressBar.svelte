<script lang="ts">
  import { seek } from "../ipc/bridge";
  import { player } from "../state/player.svelte";

  let isDragging = $state(false);
  let hoverX = $state<number | null>(null);
  let barEl: HTMLDivElement;

  function handlePointerDown(e: PointerEvent) {
    isDragging = true;
    player.isSeeking = true;
    (e.target as HTMLElement).setPointerCapture(e.pointerId);
    updatePosition(e);
  }

  function handlePointerMove(e: PointerEvent) {
    const rect = barEl.getBoundingClientRect();
    hoverX = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    if (isDragging) updatePosition(e);
  }

  async function handlePointerUp() {
    if (isDragging) {
      isDragging = false;
      await seek(player.currentTime);
      setTimeout(() => { player.isSeeking = false; }, 200);
    }
  }

  function handlePointerLeave() {
    if (!isDragging) hoverX = null;
  }

  function updatePosition(e: PointerEvent) {
    const rect = barEl.getBoundingClientRect();
    const pct = Math.max(0, Math.min(1, (e.clientX - rect.left) / rect.width));
    player.currentTime = pct * player.duration;
  }

  let progressPct = $derived(player.progress * 100);
</script>

<div
  class="progress-bar"
  bind:this={barEl}
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
  onpointerleave={handlePointerLeave}
  role="slider"
  aria-valuenow={player.currentTime}
  aria-valuemin={0}
  aria-valuemax={player.duration}
  tabindex="0"
>
  <div class="track">
    <div class="fill" style="width: {progressPct}%"></div>
    {#if hoverX !== null}
      <div class="hover-indicator" style="left: {hoverX * 100}%"></div>
    {/if}
    <div class="thumb" style="left: {progressPct}%"></div>
  </div>
</div>

<style>
  .progress-bar {
    width: 100%;
    padding: 6px 0;
    cursor: pointer;
    touch-action: none;
  }

  .track {
    position: relative;
    height: 4px;
    background: var(--bg-overlay);
    border-radius: 2px;
    overflow: visible;
  }

  .fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-light));
    border-radius: 2px;
    transition: width 16ms linear;
  }

  .thumb {
    position: absolute;
    top: 50%;
    width: 12px;
    height: 12px;
    background: var(--accent-light);
    border-radius: 50%;
    transform: translate(-50%, -50%);
    opacity: 0;
    transition: opacity var(--transition);
    box-shadow: 0 0 6px rgba(212, 160, 23, 0.4);
  }

  .progress-bar:hover .thumb {
    opacity: 1;
  }

  .hover-indicator {
    position: absolute;
    top: 0;
    width: 2px;
    height: 100%;
    background: var(--text-muted);
    transform: translateX(-50%);
    pointer-events: none;
  }
</style>
