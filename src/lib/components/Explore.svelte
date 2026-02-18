<script lang="ts">
  import { onMount } from "svelte";
  import { getExplore, playTrack } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { exploreCache } from "../state/explore.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import type { Track } from "../types";

  let sections = $derived(exploreCache.sections);
  let loading = $derived(exploreCache.loading);
  let ctxMenu: ReturnType<typeof ContextMenu>;

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

  function handleContext(e: MouseEvent, track: Track) {
    ctxMenu.open(e, track);
  }
</script>

<ContextMenu bind:this={ctxMenu} />

{#if loading}
  <div class="loading">
    <div class="eq-loader">
      <div class="eq-bar"></div>
      <div class="eq-bar"></div>
      <div class="eq-bar"></div>
      <div class="eq-bar"></div>
    </div>
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
          {#each section.tracks as track, i (track.id)}
            <button
              class="card"
              class:active={isActive(track)}
              onclick={() => handlePlay(track)}
              oncontextmenu={(e) => handleContext(e, track)}
              style="--i: {i}"
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
    animation: viewEnter 500ms var(--ease-out-expo);
  }

  .eq-loader {
    display: flex;
    align-items: flex-end;
    gap: 4px;
    height: 32px;
  }

  .eq-bar {
    width: 4px;
    height: 100%;
    background: var(--accent);
    border-radius: 2px;
    transform-origin: bottom;
    animation: eqBounce 1s ease-in-out infinite;
  }

  .eq-bar:nth-child(1) { animation-delay: 0ms; }
  .eq-bar:nth-child(2) { animation-delay: 200ms; }
  .eq-bar:nth-child(3) { animation-delay: 400ms; }
  .eq-bar:nth-child(4) { animation-delay: 150ms; }

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
    animation: float 4s ease-in-out infinite;
  }

  .empty-sub { font-size: 0.85rem; }

  .explore {
    display: flex;
    flex-direction: column;
    gap: 32px;
    animation: viewEnter 400ms var(--ease-out-expo);
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
    transition: background 200ms ease, transform 200ms ease, box-shadow 200ms ease;
    text-align: left;
    width: 100%;
    animation: itemSlideUp 350ms var(--ease-out-expo) backwards;
    animation-delay: calc(min(var(--i, 0), 12) * 35ms);
  }

  .card:hover {
    background: var(--bg-elevated);
    transform: translateY(-1px);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.2);
  }

  .card:active {
    transform: scale(0.99);
  }

  .card.active {
    background: var(--bg-elevated);
    border-left: 3px solid var(--accent);
    animation: glowPulse 3s ease-in-out infinite;
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
