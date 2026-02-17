<script lang="ts">
  import { onMount } from "svelte";
  import { getExplore, playTrack } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { exploreCache } from "../state/explore.svelte";
  import type { Track } from "../types";

  let sections = $derived(exploreCache.sections);
  let loading = $derived(exploreCache.loading);

  onMount(async () => {
    if (!exploreCache.stale) return;
    exploreCache.loading = true;
    try {
      const data = await getExplore();
      exploreCache.sections = data.sections;
      exploreCache.loaded = true;
      exploreCache.fetchedAt = Date.now();
    } catch (e) {
      console.error("explore failed:", e);
    } finally {
      exploreCache.loading = false;
    }
  });

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
</script>

{#if loading}
  <div class="loading">
    <div class="spinner-lg"></div>
    <p>Discovering music...</p>
  </div>
{:else if sections.length === 0}
  <div class="empty-state">
    <p class="empty-title">Nothing here yet</p>
    <p class="empty-sub">Search and play some tracks to get personalized recommendations</p>
  </div>
{:else}
  <div class="explore">
    {#each sections as section}
      <section class="section">
        <h2 class="section-title">{section.title}</h2>
        <div class="card-grid">
          {#each section.tracks as track (track.id)}
            <button
              class="card"
              class:active={isActive(track)}
              onclick={() => handlePlay(track)}
            >
              <img class="card-thumb" src={track.thumbnail || ""} alt="" loading="lazy" />
              <div class="card-info">
                <span class="card-title">{track.title}</span>
                <span class="card-artist">{track.artist}</span>
              </div>
              <span class="card-duration">{formatDuration(track.duration_secs)}</span>
            </button>
          {/each}
        </div>
      </section>
    {/each}
  </div>
{/if}

<style>
  .loading {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 60vh;
    gap: 16px;
    color: var(--text-muted);
  }

  .spinner-lg {
    width: 32px;
    height: 32px;
    border: 3px solid var(--bg-overlay);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

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

  .empty-sub { font-size: 0.85rem; }

  .explore {
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .section-title {
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 12px;
  }

  .card-grid {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .card {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    border-radius: var(--radius);
    transition: background var(--transition);
    text-align: left;
    width: 100%;
  }

  .card:hover { background: var(--bg-elevated); }

  .card.active {
    background: var(--bg-elevated);
    border-left: 3px solid var(--accent);
  }

  .card-thumb {
    width: 48px;
    height: 48px;
    border-radius: var(--radius-sm);
    object-fit: cover;
    background: var(--bg-overlay);
    flex-shrink: 0;
  }

  .card-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .card-title {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card-artist {
    font-size: 0.8rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card-duration {
    font-size: 0.8rem;
    color: var(--text-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }
</style>
