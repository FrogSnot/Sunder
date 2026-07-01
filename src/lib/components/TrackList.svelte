<script lang="ts">
  import { onMount } from "svelte";
  import { playTrack } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { searchState } from "../state/search.svelte";
  import { toastState } from "../state/toast.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import DragGhost from "./DragGhost.svelte";
  import WormText from "./WormText.svelte";
  import TrackArt from "./TrackArt.svelte";
  import { DragReorder } from "../util/dragReorder.svelte";
  import type { Track } from "../types";

  let tracks = $derived(searchState.results);
  let ctxMenu: ReturnType<typeof ContextMenu>;

  function formatDuration(secs: number): string {
    if (!secs) return "--:--";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  async function handlePlay(track: Track) {
    if (reorder.dragging || reorder.justDragged()) return;
    try {
      await playTrack(track);
    } catch (e) {
      console.error("play failed:", e);
      toastState.add(`Failed to play track: ${e}`, "error");
    }
  }

  function isActive(track: Track): boolean {
    return player.currentTrack?.id === track.id;
  }

  function handleContext(e: MouseEvent, track: Track) {
    ctxMenu.open(e, track);
  }

  const reorder = new DragReorder({
    getList: () => searchState.results,
    onReorder: (from, to) => {
      const moved = searchState.results.splice(from, 1)[0];
      searchState.results.splice(to, 0, moved);
      searchState.results = searchState.results;
    },
    getScrollContainer: () => document.querySelector(".content") as HTMLElement | null,
    rowSelector: ".drag-row",
  });

  onMount(() => () => reorder.destroy());
</script>

<ContextMenu bind:this={ctxMenu} />
<DragGhost {reorder} />

{#if tracks.length === 0}
  <div class="empty-state">
    <p class="empty-title"><WormText text="Search for something" /></p>
    <p class="empty-sub">Results will appear here</p>
  </div>
{:else}
  {#if searchState.resultSource}
    <div class="result-source-chip" aria-live="polite">
      {searchState.resultSource === "local" ? "Indexed" : "From YouTube"}
    </div>
  {/if}
  <div class="track-list">
    {#each tracks as track, i (track.id)}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="track-row drag-row"
        class:active={isActive(track)}
        class:is-dragging={reorder.isDragging(i)}
        class:drop-before={reorder.dragOver === i && reorder.dropPosition === "before" && reorder.dragFrom !== i}
        class:drop-after={reorder.dragOver === i && reorder.dropPosition === "after" && reorder.dragFrom !== i}
        data-idx={i}
        onpointerdown={(e) => reorder.onPointerDown(e, i)}
        onpointercancel={() => reorder.onPointerCancel()}
        oncontextmenu={(e) => handleContext(e, track)}
        style="--i: {i}"
      >
        <span class="drag-handle" aria-hidden="true">
          <svg viewBox="0 0 24 24" fill="currentColor"><circle cx="9" cy="6" r="1.5"/><circle cx="15" cy="6" r="1.5"/><circle cx="9" cy="12" r="1.5"/><circle cx="15" cy="12" r="1.5"/><circle cx="9" cy="18" r="1.5"/><circle cx="15" cy="18" r="1.5"/></svg>
        </span>
        <TrackArt
          {track}
          onplay={handlePlay}
          active={isActive(track)}
          playing={player.isPlaying}
        />
        <button class="track-play" onclick={() => handlePlay(track)}>
          <div class="track-info">
            <span class="track-title">{track.title}</span>
            <span class="track-artist">{track.artist}</span>
          </div>
          <span class="track-duration">{formatDuration(track.duration_secs)}</span>
        </button>
      </div>
    {/each}
    {#if searchState.hasMore}
      <button
        class="load-more"
        onclick={() => searchState.loadMore()}
        disabled={searchState.searching}
      >
        {searchState.searching ? "Loading..." : "Load more"}
      </button>
    {/if}
  </div>
{/if}

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 60vh;
    color: var(--text-muted);
    animation: viewEnter 500ms var(--ease-out-expo);
  }

  .empty-title {
    font-size: 1.2rem;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .empty-sub {
    font-size: 0.85rem;
  }

  .track-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: viewEnter 350ms var(--ease-out-expo);
  }

  .result-source-chip {
    font-size: 0.7rem;
    color: var(--text-secondary);
    background: var(--bg-overlay);
    padding: 3px 10px;
    border-radius: 999px;
    display: inline-block;
    align-self: flex-start;
    margin-bottom: 8px;
    letter-spacing: 0.02em;
  }

  .track-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 10px 14px;
    border-radius: var(--radius);
    transition: background 200ms ease;
    text-align: left;
    width: 100%;
    animation: itemSlideUp 350ms var(--ease-out-expo) backwards;
    animation-delay: calc(min(var(--i, 0), 15) * 30ms);
    cursor: grab;
    -webkit-user-select: none;
    user-select: none;
    -webkit-touch-callout: none;
  }

  .track-row:active { cursor: grabbing; }

  .track-play {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0;
    flex: 1;
    min-width: 0;
    text-align: left;
  }

  .track-row:hover {
    background: var(--bg-elevated);
  }

  .track-row.active {
    background: var(--bg-elevated);
    border-left: 3px solid var(--accent);
    position: relative;
  }

  .track-row.active::before {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: var(--radius);
    pointer-events: none;
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

  .load-more {
    display: block;
    width: 100%;
    margin-top: 8px;
    padding: 10px;
    background: var(--bg-elevated);
    border: none;
    border-radius: var(--radius);
    color: var(--text-secondary);
    font-size: 0.85rem;
    cursor: pointer;
    transition: background 200ms ease, color 200ms ease;
  }

  .load-more:hover:not(:disabled) {
    background: var(--bg-overlay);
    color: var(--text-primary);
  }

  .load-more:disabled {
    cursor: default;
    opacity: 0.6;
  }
</style>
