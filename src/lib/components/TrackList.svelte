<script lang="ts">
  import { playTrack } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { searchState } from "../state/search.svelte";
  import ContextMenu from "./ContextMenu.svelte";
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
    try {
      await playTrack(track);
    } catch (e) {
      console.error("play failed:", e);
    }
  }

  function isActive(track: Track): boolean {
    return player.currentTrack?.id === track.id;
  }

  function handleContext(e: MouseEvent, track: Track) {
    ctxMenu.open(e, track);
  }
</script>

<ContextMenu bind:this={ctxMenu} />

{#if tracks.length === 0}
  <div class="empty-state">
    <p class="empty-title">Search for something</p>
    <p class="empty-sub">Results will appear here</p>
  </div>
{:else}
  <div class="track-list">
    {#each tracks as track (track.id)}
      <button
        class="track-row"
        class:active={isActive(track)}
        onclick={() => handlePlay(track)}
        oncontextmenu={(e) => handleContext(e, track)}
      >
        <img
          class="thumb"
          src={track.thumbnail || ""}
          alt=""
          loading="lazy"
        />
        <div class="track-info">
          <span class="track-title">{track.title}</span>
          <span class="track-artist">{track.artist}</span>
        </div>
        <span class="track-duration">{formatDuration(track.duration_secs)}</span>
      </button>
    {/each}
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
  }

  .track-row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    border-radius: var(--radius);
    transition: background var(--transition);
    text-align: left;
    width: 100%;
  }

  .track-row:hover {
    background: var(--bg-elevated);
  }

  .track-row.active {
    background: var(--bg-elevated);
    border-left: 3px solid var(--accent);
  }

  .thumb {
    width: 48px;
    height: 48px;
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
</style>
