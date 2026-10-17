<script lang="ts">
  import { onMount } from "svelte";
  import { fly } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { quintOut } from "svelte/easing";
  import { playTrack, createPlaylist, addToPlaylist } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { nav } from "../state/nav.svelte";
  import { toastState } from "../state/toast.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import DragGhost from "./DragGhost.svelte";
  import WormText from "./WormText.svelte";
  import TrackArt from "./TrackArt.svelte";
  import { DragReorder } from "../util/dragReorder.svelte";
  import type { Track } from "../types";

  let ctxMenu: ReturnType<typeof ContextMenu>;

  const ROW_HEIGHT = 56;
  const OVERSCAN = 10;

  let queue = $derived(player.queue);
  let currentIndex = $derived(player.queueIndex);

  let nowPlaying = $derived(currentIndex >= 0 && currentIndex < queue.length ? queue[currentIndex] : null);
  let upNext = $derived(queue.slice(currentIndex + 1));
  let played = $derived(currentIndex > 0 ? queue.slice(0, currentIndex) : []);

  const reorder = new DragReorder({
    getList: () => queue,
    onReorder: (from, to) => player.moveInQueue(from, to),
    getScrollContainer: () => document.querySelector(".content") as HTMLElement | null,
    rowSelector: ".drag-row",
  });

  let scrollContainer = $state<HTMLElement | null>(null);
  let scrollTop = $state(0);
  let viewportHeight = $state(600);

  function onScroll() {
    if (!scrollContainer) return;
    scrollTop = scrollContainer.scrollTop;
    viewportHeight = scrollContainer.clientHeight;
  }

  onMount(() => {
    const el = document.querySelector(".content") as HTMLElement | null;
    if (el) {
      scrollContainer = el;
      viewportHeight = el.clientHeight;
      scrollTop = el.scrollTop;
      el.addEventListener("scroll", onScroll, { passive: true });
    }
    return () => {
      el?.removeEventListener("scroll", onScroll);
      reorder.destroy();
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
    if (reorder.busy() || reorder.justDragged()) return;
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
    if (reorder.busy() || reorder.justDragged()) return;
    ctxMenu.open(e, track);
  }

  /**
   * Live projection of post-drop row numbers while a drag is active: the
   * existing .track-num previews the projected order. Built from the
   * engine's targetPosition - the same _commitIndex math that produces
   * onReorder's `to` - so the preview can never diverge from the drop.
   * Section numbering follows the projected order (played = array prefix,
   * up-next = after the now-playing track), matching what moveInQueue will
   * produce. Null when idle or when the drag sits on its own slot, so rows
   * show their plain ri + 1.
   */
  let dragProjection = $derived.by(() => {
    if (!reorder.dragging || reorder.dragFrom < 0 || reorder.targetPosition < 1) return null;
    const from = reorder.dragFrom;
    const to = Math.min(Math.max(reorder.targetPosition - 1, 0), queue.length - 1);
    if (to === from) return null;
    const order: number[] = [];
    for (let i = 0; i < queue.length; i++) if (i !== from) order.push(i);
    order.splice(to, 0, from);
    // mirror moveInQueue's queueIndex adjustment
    let pc = currentIndex;
    if (currentIndex === from) pc = to;
    else if (from < currentIndex && to >= currentIndex) pc = currentIndex - 1;
    else if (from > currentIndex && to <= currentIndex) pc = currentIndex + 1;
    const nums = new Map<number, number>();
    for (let pj = 0; pj < order.length; pj++) {
      const qi = order[pj];
      if (qi === currentIndex) continue; // now playing has no number
      nums.set(qi, pj < pc ? pj + 1 : pj - pc);
    }
    return nums;
  });

  let live = $state("");
  const KEY_HINT =
    "Use arrow keys to move, Enter to drop, Escape to cancel.";

  function onHandleKeydown(e: KeyboardEvent, idx: number): void {
    const msg = reorder.onHandleKeydown(e, idx);
    if (msg) live = msg;
  }

  let upNextKeyIdx = $state(0);
  let playedKeyIdx = $state(0);

  function handleKeyActivate(list: "upNext" | "played", idx: number): void {
    if (reorder.busy() || reorder.justDragged()) return;
    if (list === "upNext") handlePlay(currentIndex + 1 + idx);
    else handlePlay(idx);
  }

  function dropHint(idx: number): string {
    return `Drop before ${idx + 1}. ${KEY_HINT}`;
  }
</script>

<ContextMenu bind:this={ctxMenu} />
<DragGhost {reorder} />

<div class="queue-view">
  <div class="sr-only" role="status" aria-live="assertive">{live}</div>
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
          {#if player.playbackContext}
            {@const ctx = player.playbackContext}
            <button
              class="ctx-chip"
              class:ctx-suspended={player.contextSuspended}
              onclick={(e) => {
                e.stopPropagation();
                nav.jumpToPlaylist(ctx.playlistId, ctx.playlistName);
              }}
              title={player.contextSuspended
                ? "Paused - resumes from " + ctx.playlistName + " after this track - click to open"
                : "Queue continues from " + ctx.playlistName + " - click to open"}
              aria-label={player.contextSuspended
                ? "Playlist " + ctx.playlistName + " paused, resumes after this track. Open playlist."
                : "Playing from playlist " + ctx.playlistName + ". Open playlist."}
            >
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                <path d="M9 18V5l12-2v13" />
                <circle cx="6" cy="18" r="3" />
                <circle cx="18" cy="16" r="3" />
              </svg>
              Playing from {ctx.playlistName}
            </button>
          {/if}
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
            class="track-row drag-row"
            class:is-dragging={reorder.isDragging(queueIdx)}
            class:is-key-drag={reorder.isDragging(queueIdx)}
            class:drop-before={reorder.dragOver === queueIdx && reorder.dropPosition === "before" && !reorder.isDragging(queueIdx)}
            class:drop-after={reorder.dragOver === queueIdx && reorder.dropPosition === "after" && !reorder.isDragging(queueIdx)}
            data-idx={queueIdx}
            data-track-id={track.id}
            style="--i: {ri}"
            onpointerdown={(e) => reorder.onPointerDown(e, queueIdx)}
            onpointercancel={() => reorder.onPointerCancel()}
            oncontextmenu={(e) => { e.preventDefault(); handleContext(e, track); }}
            animate:flip={{ duration: 260, easing: quintOut }}
          >
            <button
              class="drag-handle handle-btn"
              tabindex={upNextKeyIdx === ri ? 0 : -1}
              onfocus={() => (upNextKeyIdx = ri)}
              onkeydown={(e) => onHandleKeydown(e, queueIdx)}
              aria-label="Reorder {track.title}. Position {ri + 1} of {upNext.length}. Press Enter to pick up."
              data-handle="true"
            >
              <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="9" cy="6" r="1.5"/><circle cx="15" cy="6" r="1.5"/><circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/><circle cx="9" cy="18" r="1.5"/><circle cx="15" cy="18" r="1.5"/></svg>
            </button>
            <span class="track-num" onclick={(e) => { e.stopPropagation(); handleKeyActivate("upNext", ri); }} onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); handleKeyActivate("upNext", ri); } }} role="button" tabindex="-1">{dragProjection?.get(queueIdx) ?? ri + 1}</span>
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
            <button class="remove-btn drag-skip" onclick={() => handleRemove(queueIdx)} aria-label="Remove from queue">
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
            class="track-row drag-row played-row"
            class:is-dragging={reorder.isDragging(ri)}
            class:is-key-drag={reorder.isDragging(ri)}
            class:drop-before={reorder.dragOver === ri && reorder.dropPosition === "before" && !reorder.isDragging(ri)}
            class:drop-after={reorder.dragOver === ri && reorder.dropPosition === "after" && !reorder.isDragging(ri)}
            data-idx={ri}
            data-track-id={track.id}
            style="--i: {ri}"
            onpointerdown={(e) => reorder.onPointerDown(e, ri)}
            onpointercancel={() => reorder.onPointerCancel()}
            oncontextmenu={(e) => { e.preventDefault(); handleContext(e, track); }}
            animate:flip={{ duration: 260, easing: quintOut }}
          >
            <button
              class="drag-handle handle-btn"
              tabindex={playedKeyIdx === ri ? 0 : -1}
              onfocus={() => (playedKeyIdx = ri)}
              onkeydown={(e) => onHandleKeydown(e, ri)}
              aria-label="Reorder {track.title}. Position {ri + 1} of {queue.length}. Press Enter to pick up."
              data-handle="true"
            >
              <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><circle cx="9" cy="6" r="1.5"/><circle cx="15" cy="6" r="1.5"/><circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/><circle cx="9" cy="18" r="1.5"/><circle cx="15" cy="18" r="1.5"/></svg>
            </button>
            <span class="track-num" onclick={(e) => { e.stopPropagation(); handleKeyActivate("played", ri); }} onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); handleKeyActivate("played", ri); } }} role="button" tabindex="-1">{dragProjection?.get(ri) ?? ri + 1}</span>
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
            <button class="remove-btn drag-skip" onclick={() => handleRemove(ri)} aria-label="Remove from queue">
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
    display: flex;
    align-items: center;
    gap: 8px;
    opacity: 1;
    background: var(--bg-base);
    border-radius: var(--radius);
    cursor: grab;
    transition: background 200ms ease;
    -webkit-user-select: none;
    user-select: none;
    -webkit-touch-callout: none;
    /* Entrance-only stagger: runs when the view mounts. Delayed rows fill
       backwards so they start hidden, and the cap keeps deep virtualized
       lists from stacking long delays. */
    animation: itemSlideUp 350ms var(--ease-out-expo) backwards;
    animation-delay: calc(min(var(--i, 0), 15) * 30ms);
  }

  .drag-row.is-dragging,
  .drag-row.is-key-drag {
    transition: none;
    cursor: grabbing;
  }

  .drag-row.is-key-drag {
    opacity: 0.35;
    outline: 2px solid var(--accent);
    outline-offset: 2px;
    z-index: 3;
  }

  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    padding: 0;
    margin: -1px;
    overflow: hidden;
    clip: rect(0, 0, 0, 0);
    white-space: nowrap;
    border: 0;
  }

  /* The handle is a real focusable button (keyboard reorder); undo the
     global .drag-handle sizing and button chrome. */
  .handle-btn {
    appearance: none;
    background: none;
    border: none;
    padding: 0;
    margin: 0;
    height: 40px;
    width: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    flex-shrink: 0;
    cursor: grab;
    border-radius: var(--radius-sm);
  }

  .handle-btn svg {
    width: 14px;
    height: 14px;
  }

  .handle-btn:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
    color: var(--accent);
  }

  .handle-btn:focus:not(:focus-visible) {
    outline: none;
  }

  /* Hover feedback on a CHILD: the row's own rect (measured by the drag
     engine) never transforms. */
  .track-row:hover .track-play {
    transform: translateX(2px);
  }

  .track-play {
    transition: transform 180ms var(--ease-out-expo), background var(--transition);
  }

  .track-row:active { cursor: grabbing; }

  .now-playing-card {
    background: var(--bg-elevated);
    border-radius: var(--radius);
    border-left: 3px solid var(--accent);
    animation: activePulse 2.4s ease-in-out infinite;
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

  .ctx-chip {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    align-self: flex-start;
    width: fit-content;
    padding: 2px 8px;
    margin-top: 2px;
    font-size: 0.7rem;
    font-weight: 500;
    color: var(--text-muted);
    background: var(--bg-overlay);
    border-radius: 999px;
    transition: color 150ms ease, background 150ms ease, transform 150ms ease, opacity 200ms ease;
  }

  /* Chain temporarily paused behind a one-off/foreign track: the chip stays
     mounted and dims instead of unmounting (no hide/show flicker). Same box,
     so nothing shifts. */
  .ctx-chip.ctx-suspended {
    opacity: 0.45;
    color: var(--text-muted);
  }

  .ctx-chip.ctx-suspended:hover {
    opacity: 0.8;
  }

  .ctx-chip svg {
    width: 10px;
    height: 10px;
    flex-shrink: 0;
  }

  .ctx-chip:hover {
    color: var(--accent);
    background: var(--bg-elevated);
    transform: scale(1.03);
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
    min-width: 0;
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
</style>
