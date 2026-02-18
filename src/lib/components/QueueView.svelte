<script lang="ts">
  import { playTrack } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import type { Track } from "../types";

  let ctxMenu: ReturnType<typeof ContextMenu>;

  let queue = $derived(player.queue);
  let currentIndex = $derived(player.queueIndex);

  let dragFrom = $state(-1);
  let dragOver = $state(-1);
  let dragging = $state(false);

  function formatDuration(secs: number): string {
    if (!secs) return "--:--";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  async function handlePlay(index: number) {
    const track = player.playFromQueue(index);
    if (track) {
      try { await playTrack(track); } catch (e) { console.error("play:", e); }
    }
  }

  function isActive(index: number): boolean {
    return index === currentIndex && player.currentTrack?.id === queue[index]?.id;
  }

  function handleRemove(index: number) {
    player.removeFromQueue(index);
  }

  function handleShuffle() {
    player.shuffle();
  }

  function handleClear() {
    player.clearQueue();
  }

  function handleContext(e: MouseEvent, track: Track) {
    ctxMenu.open(e, track);
  }

  function onPointerDown(e: PointerEvent, index: number) {
    e.preventDefault();
    dragFrom = index;
    dragOver = index;
    dragging = true;
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    const el = document.elementFromPoint(e.clientX, e.clientY);
    if (!el) return;
    const row = el.closest('.track-row') as HTMLElement | null;
    if (!row || !row.dataset.idx) return;
    const idx = parseInt(row.dataset.idx, 10);
    if (!isNaN(idx)) dragOver = idx;
  }

  function onPointerUp() {
    if (!dragging) return;
    if (dragFrom >= 0 && dragOver >= 0 && dragFrom !== dragOver) {
      player.moveInQueue(dragFrom, dragOver);
    }
    dragFrom = -1;
    dragOver = -1;
    dragging = false;
  }
</script>

<ContextMenu bind:this={ctxMenu} />

<div class="queue-view">
  <div class="queue-header">
    <h2 class="queue-title">Queue</h2>
    {#if queue.length > 0}
      <div class="queue-actions">
        <button class="action-btn" onclick={handleShuffle} aria-label="Shuffle queue">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="16 3 21 3 21 8" /><line x1="4" y1="20" x2="21" y2="3" />
            <polyline points="21 16 21 21 16 21" /><line x1="15" y1="15" x2="21" y2="21" />
            <line x1="4" y1="4" x2="9" y2="9" />
          </svg>
          Shuffle
        </button>
        <button class="action-btn clear" onclick={handleClear} aria-label="Clear queue">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <polyline points="3 6 5 6 21 6" />
            <path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" />
          </svg>
          Clear
        </button>
      </div>
    {/if}
  </div>

  {#if queue.length === 0}
    <div class="empty-state">
      <p class="empty-title">Queue is empty</p>
      <p class="empty-sub">Right-click tracks to add them to the queue</p>
    </div>
  {:else}
    <div class="queue-info">
      <span>{queue.length} track{queue.length === 1 ? "" : "s"}</span>
    </div>
    <div class="track-list">
      {#each queue as track, i (track.id + "-" + i)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="track-row"
          class:active={isActive(i)}
          class:drag-over={dragging && dragOver === i && dragFrom !== i}
          class:dragging={dragging && dragFrom === i}
          data-idx={i}
        >
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <span
            class="drag-handle"
            onpointerdown={(e) => onPointerDown(e, i)}
            onpointermove={onPointerMove}
            onpointerup={onPointerUp}
          >
            <svg viewBox="0 0 24 24" fill="currentColor"><circle cx="9" cy="6" r="1.5"/><circle cx="15" cy="6" r="1.5"/><circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/><circle cx="9" cy="18" r="1.5"/><circle cx="15" cy="18" r="1.5"/></svg>
          </span>
          <span class="track-num">{i + 1}</span>
          <button
            class="track-play"
            onclick={() => handlePlay(i)}
            oncontextmenu={(e) => handleContext(e, track)}
          >
            <img class="thumb" src={track.thumbnail || ""} alt="" loading="lazy" />
            <div class="track-info">
              <span class="track-title">{track.title}</span>
              <span class="track-artist">{track.artist}</span>
            </div>
            <span class="track-duration">{formatDuration(track.duration_secs)}</span>
          </button>
          <button class="remove-btn" onclick={() => handleRemove(i)} aria-label="Remove from queue">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
            </svg>
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .queue-view {
    display: flex;
    flex-direction: column;
    gap: 8px;
    animation: viewEnter 350ms var(--ease-out-expo);
  }

  .queue-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;
  }

  .queue-title {
    font-size: 1.3rem;
    font-weight: 700;
    color: var(--text-primary);
    border-left: 3px solid var(--accent);
    padding-left: 10px;
  }

  .queue-actions {
    display: flex;
    gap: 8px;
  }

  .action-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--text-secondary);
    background: var(--bg-elevated);
    border-radius: var(--radius);
    transition: background 200ms ease, color 200ms ease, transform 150ms var(--ease-spring);
  }

  .action-btn:hover {
    background: var(--bg-overlay);
    color: var(--text-primary);
    transform: scale(1.03);
  }

  .action-btn:active {
    transform: scale(0.97);
  }

  .action-btn svg { width: 14px; height: 14px; }

  .action-btn.clear:hover { color: var(--error); }

  .queue-info {
    font-size: 0.8rem;
    color: var(--text-muted);
    padding: 0 4px;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 40vh;
    color: var(--text-muted);
    animation: viewEnter 500ms var(--ease-out-expo);
  }

  .empty-title {
    font-size: 1.1rem;
    color: var(--text-secondary);
    margin-bottom: 4px;
    animation: float 4s ease-in-out infinite;
  }

  .empty-sub { font-size: 0.85rem; }

  .track-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: viewEnter 350ms var(--ease-out-expo);
  }

  .track-row {
    display: flex;
    align-items: center;
    gap: 4px;
    opacity: 1;
    background: var(--bg-base);
    border-radius: var(--radius);
    transition: background 200ms ease, opacity 150ms ease, transform 200ms ease;
  }

  .track-row:hover {
    transform: translateY(-1px);
  }

  .track-row.dragging {
    opacity: 0.3;
  }

  .track-row.drag-over {
    border-top: 2px solid var(--accent);
    margin-top: -2px;
  }

  .drag-handle {
    width: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    cursor: grab;
    flex-shrink: 0;
    opacity: 0.4;
    transition: opacity var(--transition), color var(--transition);
    touch-action: none;
  }

  .drag-handle:active { cursor: grabbing; }
  .track-row:hover .drag-handle { opacity: 1; }
  .drag-handle svg { width: 12px; height: 12px; }

  .track-row.active {
    background: var(--bg-elevated);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius);
    animation: glowPulse 3s ease-in-out infinite;
  }

  .track-num {
    width: 28px;
    text-align: center;
    font-size: 0.8rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .track-play {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 10px;
    border-radius: var(--radius);
    transition: background var(--transition);
    text-align: left;
  }

  .track-play:hover { background: var(--bg-elevated); }

  .thumb {
    width: 40px;
    height: 40px;
    border-radius: var(--radius-sm);
    object-fit: cover;
    background: var(--bg-overlay);
    flex-shrink: 0;
  }

  .track-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .track-title {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track-artist {
    font-size: 0.8rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track-duration {
    font-size: 0.8rem;
    color: var(--text-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .remove-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    transition: color 200ms ease, transform 150ms ease;
    flex-shrink: 0;
  }

  .remove-btn:hover {
    color: var(--error);
    transform: scale(1.15);
  }
  .remove-btn svg { width: 14px; height: 14px; }
</style>
