<script lang="ts">
  import { onMount } from "svelte";
  import { playTrack, createPlaylist, addToPlaylist } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { toastState } from "../state/toast.svelte";
  import { fly } from "svelte/transition";
  import { flip } from "svelte/animate";
  import ContextMenu from "./ContextMenu.svelte";
  import WormText from "./WormText.svelte";
  import TrackArt from "./TrackArt.svelte";
  import type { Track } from "../types";

  let ctxMenu: ReturnType<typeof ContextMenu>;

  const ROW_HEIGHT = 56;
  const OVERSCAN = 10;
  const DRAG_THRESHOLD = 5;
  const LONG_PRESS_MS = 220;
  const AUTO_SCROLL_EDGE = 56;
  const AUTO_SCROLL_SPEED = 10;

  let queue = $derived(player.queue);
  let currentIndex = $derived(player.queueIndex);

  let nowPlaying = $derived(currentIndex >= 0 && currentIndex < queue.length ? queue[currentIndex] : null);
  let upNext = $derived(queue.slice(currentIndex + 1));
  let played = $derived(currentIndex > 0 ? queue.slice(0, currentIndex) : []);

  // Drag state
  let dragFrom = $state(-1);
  let dragOver = $state(-1);
  let dropPosition = $state<"before" | "after">("before");
  let dragging = $state(false);
  let dragX = $state(0);
  let dragY = $state(0);
  let justDragged = $state(false);

  // Non-reactive press tracking
  let pressIndex = -1;
  let pressStartX = 0;
  let pressStartY = 0;
  let pressActive = false;
  let pressTimer: ReturnType<typeof setTimeout> | null = null;
  let autoScrollRaf: number | null = null;

  // Virtual scroll state
  let scrollContainer = $state<HTMLElement | null>(null);
  let scrollTop = $state(0);
  let viewportHeight = $state(600);

  function onScroll() {
    if (!scrollContainer) return;
    scrollTop = scrollContainer.scrollTop;
    viewportHeight = scrollContainer.clientHeight;
  }

  onMount(() => {
    const el = document.querySelector('.content') as HTMLElement | null;
    if (el) {
      scrollContainer = el;
      viewportHeight = el.clientHeight;
      scrollTop = el.scrollTop;
      el.addEventListener('scroll', onScroll, { passive: true });
    }
    return () => {
      if (el) el.removeEventListener('scroll', onScroll);
      cleanupDrag();
    };
  });

  let upNextListEl = $state<HTMLElement | null>(null);
  let upNextOffset = $derived.by(() => {
    if (!upNextListEl || !scrollContainer) return 0;
    return upNextListEl.offsetTop;
  });

  let upNextSlice = $derived.by(() => {
    const total = upNext.length;
    if (total <= 50) return { start: 0, end: total };
    const relScroll = Math.max(0, scrollTop - upNextOffset);
    const start = Math.max(0, Math.floor(relScroll / ROW_HEIGHT) - OVERSCAN);
    const end = Math.min(total, Math.ceil((relScroll + viewportHeight) / ROW_HEIGHT) + OVERSCAN);
    return { start, end };
  });

  let playedListEl = $state<HTMLElement | null>(null);
  let playedOffset = $derived.by(() => {
    if (!playedListEl || !scrollContainer) return 0;
    return playedListEl.offsetTop;
  });

  let playedSlice = $derived.by(() => {
    const total = played.length;
    if (total <= 50) return { start: 0, end: total };
    const relScroll = Math.max(0, scrollTop - playedOffset);
    const start = Math.max(0, Math.floor(relScroll / ROW_HEIGHT) - OVERSCAN);
    const end = Math.min(total, Math.ceil((relScroll + viewportHeight) / ROW_HEIGHT) + OVERSCAN);
    return { start, end };
  });

  function formatDuration(secs: number): string {
    if (!secs) return "--:--";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  async function handlePlay(index: number) {
    if (dragging || justDragged) {
      justDragged = false;
      return;
    }
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

  let savingAsPlaylist = $state(false);
  let newPlaylistName = $state("");
  let saveInput = $state<HTMLInputElement | null>(null);
  let saving = $state(false);

  function startSaveAsPlaylist() {
    newPlaylistName = "My Queue";
    savingAsPlaylist = true;
    setTimeout(() => saveInput?.select(), 0);
  }

  function cancelSave() {
    savingAsPlaylist = false;
    newPlaylistName = "";
  }

  async function confirmSave() {
    const name = newPlaylistName.trim();
    if (!name || saving) return;
    saving = true;
    try {
      const playlist = await createPlaylist(name);
      for (const track of queue) {
        await addToPlaylist(playlist.id, track.id);
      }
      toastState.add(`Saved "${name}" with ${queue.length} tracks`, "info", 4000);
      savingAsPlaylist = false;
      newPlaylistName = "";
    } catch (e) {
      toastState.add(`Failed to save playlist: ${e}`, "error", 6000);
    } finally {
      saving = false;
    }
  }

  function handleContext(e: MouseEvent, track: Track) {
    ctxMenu.open(e, track);
  }

  // Drag and drop

  function onRowPointerDown(e: PointerEvent, index: number) {
    if (e.button !== 0 && e.pointerType === "mouse") return;
    const target = e.target as HTMLElement;
    if (target.closest('.remove-btn')) return;
    if (target.closest('button') && !target.closest('.drag-handle')) {
      // Allow button clicks (play) without starting drag
      return;
    }
    pressIndex = index;
    pressStartX = e.clientX;
    pressStartY = e.clientY;
    pressActive = true;
    if (pressTimer) clearTimeout(pressTimer);
    pressTimer = setTimeout(() => {
      if (pressActive && pressIndex === index) {
        startDrag(e, index);
      }
    }, LONG_PRESS_MS);
  }

  function startDrag(e: PointerEvent, index: number) {
    if (dragging) return;
    dragging = true;
    dragFrom = index;
    dragOver = index;
    dropPosition = "before";
    dragX = e.clientX;
    dragY = e.clientY;
    document.body.style.touchAction = "none";
    document.body.style.userSelect = "none";
    document.body.style.cursor = "grabbing";
    document.addEventListener("pointermove", onDocPointerMove);
    document.addEventListener("pointerup", onDocPointerUp);
    document.addEventListener("pointercancel", onDocPointerUp);
    document.addEventListener("keydown", onDocKeyDown);
  }

  function cleanupDrag() {
    if (pressTimer) {
      clearTimeout(pressTimer);
      pressTimer = null;
    }
    if (autoScrollRaf !== null) {
      cancelAnimationFrame(autoScrollRaf);
      autoScrollRaf = null;
    }
    document.removeEventListener("pointermove", onDocPointerMove);
    document.removeEventListener("pointerup", onDocPointerUp);
    document.removeEventListener("pointercancel", onDocPointerUp);
    document.removeEventListener("keydown", onDocKeyDown);
    document.body.style.touchAction = "";
    document.body.style.userSelect = "";
    document.body.style.cursor = "";
    pressActive = false;
    pressIndex = -1;
  }

  function onDocPointerMove(e: PointerEvent) {
    if (pressActive && !dragging) {
      const dx = e.clientX - pressStartX;
      const dy = e.clientY - pressStartY;
      if (Math.hypot(dx, dy) > DRAG_THRESHOLD) {
        if (pressTimer) {
          clearTimeout(pressTimer);
          pressTimer = null;
        }
        startDrag(e, pressIndex);
      }
      return;
    }
    if (!dragging) return;
    dragX = e.clientX;
    dragY = e.clientY;
    updateHover(e.clientX, e.clientY);
    scheduleAutoScroll();
  }

  function updateHover(x: number, y: number) {
    const els = document.elementsFromPoint(x, y);
    let row: HTMLElement | null = null;
    for (const el of els) {
      const candidate = (el as HTMLElement).closest?.('.track-row') as HTMLElement | null;
      if (candidate && candidate.dataset.idx) {
        row = candidate;
        break;
      }
    }
    if (!row || !row.dataset.idx) return;
    const idx = parseInt(row.dataset.idx, 10);
    if (isNaN(idx) || idx === dragFrom) {
      if (idx !== dragFrom) return;
      return;
    }
    dragOver = idx;
    const rect = row.getBoundingClientRect();
    const midY = rect.top + rect.height / 2;
    dropPosition = y < midY ? "before" : "after";
  }

  function scheduleAutoScroll() {
    if (autoScrollRaf !== null) return;
    autoScrollRaf = requestAnimationFrame(() => {
      autoScrollRaf = null;
      if (!dragging || !scrollContainer) return;
      const rect = scrollContainer.getBoundingClientRect();
      const mouseY = dragY;
      if (mouseY < rect.top + AUTO_SCROLL_EDGE) {
        const intensity = 1 - Math.min(1, (mouseY - rect.top) / AUTO_SCROLL_EDGE);
        scrollContainer.scrollTop -= AUTO_SCROLL_SPEED * intensity;
      } else if (mouseY > rect.bottom - AUTO_SCROLL_EDGE) {
        const intensity = 1 - Math.min(1, (rect.bottom - mouseY) / AUTO_SCROLL_EDGE);
        scrollContainer.scrollTop += AUTO_SCROLL_SPEED * intensity;
      }
    });
  }

  function onDocPointerUp() {
    const wasDragging = dragging;
    if (wasDragging && dragFrom >= 0 && dragOver >= 0 && dragFrom !== dragOver) {
      let target = dropPosition === "after" ? dragOver + 1 : dragOver;
      if (dragFrom < target) target -= 1;
      if (target !== dragFrom && target >= 0 && target < queue.length) {
        player.moveInQueue(dragFrom, target);
        justDragged = true;
        setTimeout(() => { justDragged = false; }, 50);
      } else if (target === dragFrom) {
        justDragged = true;
        setTimeout(() => { justDragged = false; }, 50);
      }
    } else if (wasDragging) {
      justDragged = true;
      setTimeout(() => { justDragged = false; }, 50);
    }
    dragging = false;
    dragFrom = -1;
    dragOver = -1;
    cleanupDrag();
  }

  function onDocKeyDown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      dragging = false;
      dragFrom = -1;
      dragOver = -1;
      cleanupDrag();
    }
  }

  function dropClass(idx: number): string {
    if (!dragging || dragFrom === idx || dragOver !== idx) return "";
    return dropPosition === "before" ? "drop-before" : "drop-after";
  }

  let ghostTrack = $derived(dragging && dragFrom >= 0 ? queue[dragFrom] : null);
</script>

<ContextMenu bind:this={ctxMenu} />

<div class="queue-view">
  <div class="queue-header">
    <h2 class="queue-title">Queue</h2>
    {#if queue.length > 0}
      <div class="queue-actions">
        {#if savingAsPlaylist}
          <div class="save-inline" in:fly={{ x: 8, duration: 150 }}>
            <input
              class="save-input"
              type="text"
              bind:value={newPlaylistName}
              bind:this={saveInput}
              placeholder="Playlist name"
              onkeydown={(e) => { if (e.key === 'Enter') confirmSave(); else if (e.key === 'Escape') cancelSave(); }}
            />
            <button class="action-btn save-confirm" onclick={confirmSave} disabled={saving} aria-label="Confirm">
              {#if saving}
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/></svg>
              {:else}
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
              {/if}
            </button>
            <button class="action-btn" onclick={cancelSave} aria-label="Cancel">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
            </button>
          </div>
        {:else}
          <button class="action-btn" onclick={startSaveAsPlaylist} aria-label="Save queue as playlist">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M19 21H5a2 2 0 01-2-2V5a2 2 0 012-2h11l5 5v11a2 2 0 01-2 2z"/>
              <polyline points="17 21 17 13 7 13 7 21"/>
              <polyline points="7 3 7 8 15 8"/>
            </svg>
            Save
          </button>
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
        {/if}
      </div>
    {/if}
  </div>

  {#if queue.length === 0}
    <div class="empty-state">
      <p class="empty-title"><WormText text="Queue is empty" /></p>
      <p class="empty-sub">Right-click tracks to add them to the queue</p>
    </div>
  {:else}
    {#if nowPlaying}
      <div class="section-label">Now Playing</div>
      {#key nowPlaying.id}
        <div class="now-playing-card" in:fly={{ y: -20, duration: 300 }}>
          <TrackArt track={nowPlaying} onplay={() => handlePlay(currentIndex)} active playing={player.isPlaying} size={48} />
          <button
            class="track-play np-track"
            onclick={() => handlePlay(currentIndex)}
            oncontextmenu={(e) => handleContext(e, nowPlaying)}
          >
            <div class="track-info">
              <span class="track-title np-title">{nowPlaying.title}</span>
              <span class="track-artist">{nowPlaying.artist}</span>
            </div>
            <span class="track-duration">{formatDuration(nowPlaying.duration_secs)}</span>
          </button>
        </div>
      {/key}
    {/if}

    {#if upNext.length > 0}
      <div class="section-label next-label">Next Up <span class="section-count">{upNext.length}</span></div>
      <div class="track-list" bind:this={upNextListEl}>
        {#if upNextSlice.start > 0}
          <div style="height: {upNextSlice.start * ROW_HEIGHT}px" aria-hidden="true"></div>
        {/if}
        {#each upNext.slice(upNextSlice.start, upNextSlice.end) as track, i (track.id)}
          {@const ri = upNextSlice.start + i}
          {@const queueIdx = currentIndex + 1 + ri}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="track-row {dropClass(queueIdx)}"
            class:dragging-row={dragging && dragFrom === queueIdx}
            data-idx={queueIdx}
            animate:flip={{ duration: 260 }}
            onpointerdown={(e) => onRowPointerDown(e, queueIdx)}
            oncontextmenu={(e) => { e.preventDefault(); handleContext(e, track); }}
          >
            <span class="drag-handle" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="currentColor"><circle cx="9" cy="6" r="1.5"/><circle cx="15" cy="6" r="1.5"/><circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/><circle cx="9" cy="18" r="1.5"/><circle cx="15" cy="18" r="1.5"/></svg>
            </span>
            <span class="track-num">{ri + 1}</span>
            <TrackArt {track} onplay={() => handlePlay(queueIdx)} size={40} />
            <button
              class="track-play"
              onclick={() => handlePlay(queueIdx)}
              oncontextmenu={(e) => handleContext(e, track)}
            >
              <div class="track-info">
                <span class="track-title">{track.title}</span>
                <span class="track-artist">{track.artist}</span>
              </div>
              <span class="track-duration">{formatDuration(track.duration_secs)}</span>
            </button>
            <button class="remove-btn" onclick={() => handleRemove(queueIdx)} aria-label="Remove from queue">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        {/each}
        {#if upNextSlice.end < upNext.length}
          <div style="height: {(upNext.length - upNextSlice.end) * ROW_HEIGHT}px" aria-hidden="true"></div>
        {/if}
      </div>
    {/if}

    {#if played.length > 0}
      <div class="section-label played-label">Previously Played</div>
      <div class="track-list played-list" bind:this={playedListEl}>
        {#if playedSlice.start > 0}
          <div style="height: {playedSlice.start * ROW_HEIGHT}px" aria-hidden="true"></div>
        {/if}
        {#each played.slice(playedSlice.start, playedSlice.end) as track, i (track.id)}
          {@const ri = playedSlice.start + i}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="track-row played-row {dropClass(ri)}"
            class:dragging-row={dragging && dragFrom === ri}
            data-idx={ri}
            animate:flip={{ duration: 260 }}
            onpointerdown={(e) => onRowPointerDown(e, ri)}
            oncontextmenu={(e) => { e.preventDefault(); handleContext(e, track); }}
          >
            <span class="drag-handle" aria-hidden="true">
              <svg viewBox="0 0 24 24" fill="currentColor"><circle cx="9" cy="6" r="1.5"/><circle cx="15" cy="6" r="1.5"/><circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/><circle cx="9" cy="18" r="1.5"/><circle cx="15" cy="18" r="1.5"/></svg>
            </span>
            <span class="track-num">{ri + 1}</span>
            <TrackArt {track} onplay={() => handlePlay(ri)} size={40} />
            <button
              class="track-play"
              onclick={() => handlePlay(ri)}
              oncontextmenu={(e) => handleContext(e, track)}
            >
              <div class="track-info">
                <span class="track-title">{track.title}</span>
                <span class="track-artist">{track.artist}</span>
              </div>
              <span class="track-duration">{formatDuration(track.duration_secs)}</span>
            </button>
            <button class="remove-btn" onclick={() => handleRemove(ri)} aria-label="Remove from queue">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        {/each}
        {#if playedSlice.end < played.length}
          <div style="height: {(played.length - playedSlice.end) * ROW_HEIGHT}px" aria-hidden="true"></div>
        {/if}
      </div>
    {/if}
  {/if}
</div>

{#if ghostTrack}
  <div
    class="drag-ghost"
    style="transform: translate3d({dragX + 14}px, {dragY + 14}px, 0)"
    aria-hidden="true"
  >
    <div class="ghost-handle">
      <svg viewBox="0 0 24 24" fill="currentColor"><circle cx="9" cy="6" r="1.5"/><circle cx="15" cy="6" r="1.5"/><circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/><circle cx="9" cy="18" r="1.5"/><circle cx="15" cy="18" r="1.5"/></svg>
    </div>
    <div class="ghost-info">
      <div class="ghost-title">{ghostTrack.title}</div>
      <div class="ghost-artist">{ghostTrack.artist}</div>
    </div>
  </div>
{/if}

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

  .save-inline {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .save-input {
    background: var(--bg-overlay);
    border: 1px solid var(--bg-overlay);
    border-radius: var(--radius);
    color: var(--text-primary);
    font-size: 0.8rem;
    padding: 5px 10px;
    outline: none;
    width: 160px;
    transition: border-color 200ms ease;
  }

  .save-input:focus {
    border-color: var(--accent-dim);
  }

  .save-confirm:hover { color: var(--accent); }

  .section-label {
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 12px 4px 6px;
  }

  .section-label .section-count {
    color: var(--text-muted);
    font-weight: 400;
    opacity: 0.6;
  }

  .next-label {
    margin-top: 4px;
  }

  .played-label {
    margin-top: 12px;
    opacity: 0.6;
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
  }

  .empty-sub { font-size: 0.85rem; }

  .track-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: viewEnter 350ms var(--ease-out-expo);
  }

  .track-row {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    opacity: 1;
    background: var(--bg-base);
    border-radius: var(--radius);
    cursor: grab;
    transition: background 200ms ease, opacity 150ms ease, transform 200ms ease;
    -webkit-user-select: none;
    user-select: none;
    -webkit-touch-callout: none;
  }

  .track-row:hover {
    transform: translateY(-1px);
  }

  .track-row:active { cursor: grabbing; }

  .track-row.dragging-row {
    opacity: 0.35;
    transform: scale(0.98);
  }

  .track-row.drop-before::before,
  .track-row.drop-after::after {
    content: "";
    position: absolute;
    left: 4px;
    right: 4px;
    height: 2px;
    background: var(--accent);
    border-radius: 2px;
    box-shadow: 0 0 10px var(--accent), 0 0 2px var(--accent);
    pointer-events: none;
    animation: dropPulse 1.2s ease-in-out infinite;
  }

  .track-row.drop-before::before { top: -2px; }
  .track-row.drop-after::after { bottom: -2px; }

  @keyframes dropPulse {
    0%, 100% { opacity: 0.85; }
    50% { opacity: 1; }
  }

  .drag-handle {
    width: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    flex-shrink: 0;
    opacity: 0.5;
    transition: opacity var(--transition), color var(--transition);
  }

  .track-row:hover .drag-handle { opacity: 1; color: var(--text-secondary); }
  .track-row.dragging-row .drag-handle { opacity: 1; color: var(--accent); }
  .drag-handle svg { width: 12px; height: 12px; }

  .now-playing-card {
    background: var(--bg-elevated);
    border-radius: var(--radius);
    border-left: 3px solid var(--accent);
    margin-bottom: 4px;
    transform-origin: top center;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 14px;
  }

  .np-track {
    padding: 0;
  }

  .np-title {
    color: var(--accent);
  }

  .played-list {
    opacity: 0.5;
  }

  .played-list:hover,
  .played-row:hover {
    opacity: 1;
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
    cursor: pointer;
  }

  .track-play:hover { background: var(--bg-elevated); }

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
    transition: color 200ms ease, transform 150ms ease, background 150ms ease;
    flex-shrink: 0;
  }

  .remove-btn:hover {
    color: var(--error);
    background: var(--bg-elevated);
    transform: scale(1.15);
  }
  .remove-btn svg { width: 14px; height: 14px; }

  .drag-ghost {
    position: fixed;
    top: 0;
    left: 0;
    z-index: 9999;
    pointer-events: none;
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
    from { opacity: 0; transform: translate3d(var(--gx, 0), var(--gy, 0), 0) scale(0.95); }
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
</style>
