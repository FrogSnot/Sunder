<script lang="ts">
  import { getDownloads, getDownloadsSize, getDownloadSizes, playTrack } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { downloads } from "../state/downloads.svelte";
  import { toastState } from "../state/toast.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import TrackArt from "./TrackArt.svelte";
  import WormText from "./WormText.svelte";
  import type { Track } from "../types";

  let ctxMenu: ReturnType<typeof ContextMenu>;
  let tracks = $state<Track[]>([]);
  let sizeBytes = $state(0);
  let sizes = $state<Map<string, number>>(new Map());

  // Refetch whenever the downloaded set changes (additions or removals).
  $effect(() => {
    void downloads.ids;
    getDownloads()
      .then((t) => { tracks = t; })
      .catch((e) => console.error("get downloads:", e));
    getDownloadsSize()
      .then((b) => { sizeBytes = b; })
      .catch((e) => console.error("get downloads size:", e));
    getDownloadSizes()
      .then((pairs) => { sizes = new Map(pairs); })
      .catch((e) => console.error("get download sizes:", e));
  });

  let totalLabel = $derived(
    tracks.length === 1 ? "1 track" : `${tracks.length} tracks`,
  );

  function formatSize(bytes: number): string {
    if (!bytes) return "0 MB";
    const mb = bytes / (1024 * 1024);
    if (mb >= 1024) return `${(mb / 1024).toFixed(1)} GB`;
    return `${mb < 10 ? mb.toFixed(1) : Math.round(mb)} MB`;
  }

  function formatDuration(secs: number): string {
    if (!secs) return "--:--";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function isActive(track: Track): boolean {
    return player.currentTrack?.id === track.id;
  }

  async function handlePlay(track: Track) {
    try {
      await playTrack(track);
    } catch (e) {
      toastState.add(`Failed to play track: ${e}`, "error");
    }
  }

  async function handlePlayAll() {
    if (tracks.length === 0) return;
    player.setQueue(tracks);
    const first = player.playFromQueue(0);
    if (first) await playTrack(first);
  }
</script>

<ContextMenu bind:this={ctxMenu} />

<div class="downloads-view">
  <div class="header">
    <div class="heading">
      <h2 class="title">Downloads</h2>
      {#if tracks.length > 0}
        <div class="hero-stats">
          <span class="stat">{totalLabel}</span>
          <span class="stat-dot" aria-hidden="true"></span>
          <span class="stat">{formatSize(sizeBytes)} on disk</span>
        </div>
      {/if}
    </div>
    {#if tracks.length > 0}
      <button class="play-all-btn" onclick={handlePlayAll} aria-label="Play all downloads">
        <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
        Play All
      </button>
    {/if}
  </div>

  {#if tracks.length === 0}
    <div class="empty-state">
      <p class="empty-title"><WormText text="No downloads yet" /></p>
      <p class="empty-sub">Use the download button on any track to save it for offline playback</p>
    </div>
  {:else}
    <div class="track-list">
      {#each tracks as track, i (track.id)}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="track-row"
          class:active={isActive(track)}
          style="--i: {i}"
          oncontextmenu={(e) => ctxMenu.open(e, track)}
        >
          <TrackArt {track} onplay={handlePlay} active={isActive(track)} playing={player.isPlaying} />
          <button class="track-play" onclick={() => handlePlay(track)}>
            <div class="track-info">
              <span class="track-title">{track.title}</span>
              <span class="track-artist">{track.artist}</span>
            </div>
            {#if sizes.get(track.id)}
              <span class="track-size">{formatSize(sizes.get(track.id) ?? 0)}</span>
            {/if}
            <span class="track-duration">{formatDuration(track.duration_secs)}</span>
          </button>
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .downloads-view {
    animation: viewEnter 350ms var(--ease-out-expo);
  }

  .header {
    display: flex;
    align-items: flex-end;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 20px;
  }

  .heading {
    display: flex;
    flex-direction: column;
    gap: 4px;
    flex: 1;
    min-width: 0;
  }

  .title {
    font-size: 1.5rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .hero-stats {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 0.85rem;
    color: var(--text-secondary);
  }

  .stat {
    font-variant-numeric: tabular-nums;
  }

  .stat-dot {
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--text-muted);
    flex-shrink: 0;
  }

  .play-all-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 8px 16px;
    background: var(--accent);
    color: #121212;
    border-radius: var(--radius);
    font-size: 0.85rem;
    font-weight: 600;
    flex-shrink: 0;
    z-index: 1;
    transition: transform 150ms ease, background 200ms ease;
  }

  .play-all-btn:hover {
    background: var(--accent-light);
  }

  .play-all-btn:active {
    transform: scale(0.97);
  }

  .play-all-btn svg {
    width: 16px;
    height: 16px;
  }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 55vh;
    color: var(--text-muted);
    text-align: center;
  }

  .empty-title {
    font-size: 1.2rem;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .empty-sub {
    font-size: 0.85rem;
    max-width: 320px;
  }

  .track-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .track-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 12px 10px 14px;
    border-radius: var(--radius);
    transition: background 200ms ease;
    width: 100%;
    animation: itemSlideUp 350ms var(--ease-out-expo) backwards;
    animation-delay: calc(min(var(--i, 0), 15) * 30ms);
  }

  .track-row:hover {
    background: var(--bg-elevated);
  }

  .track-row.active {
    background: var(--bg-elevated);
    border-left: 3px solid var(--accent);
  }

  .track-play {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 0;
    flex: 1;
    min-width: 0;
    text-align: left;
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

  .track-size {
    font-size: 0.72rem;
    color: var(--text-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--hover-overlay);
  }
</style>
