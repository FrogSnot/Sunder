<script lang="ts">
  import { onMount } from "svelte";
  import { getExplore, playTrack } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { exploreCache } from "../state/explore.svelte";
  import { toastState } from "../state/toast.svelte";
  import { downloads } from "../state/downloads.svelte";
  import ContextMenu from "./ContextMenu.svelte";
  import WormText from "./WormText.svelte";
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
      toastState.add(`Failed to load explore: ${e}`, "error");
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

  function fallbackInitial(track: Track): string {
    return (track.title || track.artist || "?").trim().slice(0, 1).toUpperCase();
  }

  function handleShelfWheel(e: WheelEvent) {
    if (!e.shiftKey) return;
    if (Math.abs(e.deltaY) <= Math.abs(e.deltaX)) return;

    const shelf = e.currentTarget as HTMLElement;
    const atStart = shelf.scrollLeft <= 0 && e.deltaY < 0;
    const atEnd = shelf.scrollLeft + shelf.clientWidth >= shelf.scrollWidth - 1 && e.deltaY > 0;
    if (atStart || atEnd) return;

    shelf.scrollLeft += e.deltaY;
    e.preventDefault();
  }

  async function handlePlay(track: Track) {
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
</script>

<ContextMenu bind:this={ctxMenu} />

{#if loading}
  <div class="loading">
    <div class="loading-header">
      <div class="eq-loader">
        <div class="eq-bar"></div>
        <div class="eq-bar"></div>
        <div class="eq-bar"></div>
        <div class="eq-bar"></div>
      </div>
      <p>Discovering music...</p>
    </div>
    <div class="skeleton-grid">
      {#each Array(10) as _, i}
        <div class="skeleton-card" style="--i: {i}">
          <div class="skeleton-art"></div>
          <div class="skeleton-line wide"></div>
          <div class="skeleton-line"></div>
        </div>
      {/each}
    </div>
  </div>
{:else if sections.length === 0}
  <div class="empty-state">
    <p class="empty-title"><WormText text="Nothing here yet" /></p>
    <p class="empty-sub">Search and play some tracks to get personalized recommendations</p>
  </div>
{:else}
  <div class="explore">
    {#each sections as section}
      <section class="section">
        <div class="section-header">
          <h2 class="section-title">{section.title}</h2>
        </div>
        <div class="card-grid" onwheel={handleShelfWheel}>
          {#each section.tracks as track, i (track.id)}
            <button
              class="card"
              class:active={isActive(track)}
              onclick={() => handlePlay(track)}
              oncontextmenu={(e) => handleContext(e, track)}
              aria-label={`Play ${track.title} by ${track.artist}`}
              style="--i: {i}"
            >
              <span class="art-shell">
                {#if track.thumbnail}
                  <img class="card-thumb" src={track.thumbnail} alt="" loading="lazy" />
                {:else}
                  <span class="art-fallback" aria-hidden="true">{fallbackInitial(track)}</span>
                {/if}
                <span class="art-shade" aria-hidden="true"></span>
                <span class="play-puck" aria-hidden="true"><span class="play-triangle"></span></span>
                {#if downloads.isDownloaded(track.id)}
                  <span class="dl-badge" aria-label="Downloaded" title="Available offline">
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12"/></svg>
                  </span>
                {/if}
                <span class="card-duration">{formatDuration(track.duration_secs)}</span>
              </span>
              <div class="card-info">
                <span class="card-title">{track.title}</span>
                <span class="card-artist">{track.artist}</span>
              </div>
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
    gap: 24px;
    color: var(--text-muted);
    animation: viewEnter 500ms var(--ease-out-expo);
  }

  .loading-header {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--text-secondary);
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

  .skeleton-grid {
    display: flex;
    gap: 18px;
    overflow: hidden;
  }

  .skeleton-card {
    display: flex;
    flex-direction: column;
    flex: 0 0 180px;
    gap: 10px;
    animation: itemSlideUp 350ms var(--ease-out-expo) backwards;
    animation-delay: calc(min(var(--i, 0), 10) * 30ms);
  }

  .skeleton-art,
  .skeleton-line {
    background: linear-gradient(90deg, var(--bg-elevated), var(--bg-overlay), var(--bg-elevated));
    background-size: 200% 100%;
    animation: shimmer 1.5s ease-in-out infinite;
  }

  .skeleton-art {
    aspect-ratio: 1;
    border-radius: var(--radius);
  }

  .skeleton-line {
    height: 10px;
    width: 62%;
    border-radius: 999px;
  }

  .skeleton-line.wide { width: 88%; }

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

  .empty-sub { font-size: 0.85rem; }

  .explore {
    display: flex;
    flex-direction: column;
    gap: 38px;
    animation: viewEnter 400ms var(--ease-out-expo);
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .section-header {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .section-title {
    font-size: 1.1rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .card-grid {
    display: flex;
    gap: 18px;
    overflow-x: auto;
    overflow-y: hidden;
    scroll-snap-type: x proximity;
    scrollbar-width: thin;
    scrollbar-color: var(--bg-overlay) transparent;
    scroll-padding-inline: 28px;
    padding: 4px 28px 14px;
    margin: 0 -28px;
  }

  .card-grid::-webkit-scrollbar {
    height: 6px;
  }

  .card-grid::-webkit-scrollbar-track {
    background: transparent;
  }

  .card-grid::-webkit-scrollbar-thumb {
    background: var(--bg-overlay);
    border-radius: 999px;
  }

  .card-grid::-webkit-scrollbar-thumb:hover {
    background: var(--text-muted);
  }

  .card {
    display: flex;
    flex-direction: column;
    flex: 0 0 184px;
    gap: 10px;
    min-width: 0;
    padding: 10px;
    border-radius: var(--radius);
    border: 1px solid transparent;
    background: rgba(255, 255, 255, 0.02);
    transition: background 180ms ease, border-color 180ms ease, transform 180ms ease;
    text-align: left;
    width: 184px;
    scroll-snap-align: start;
    animation: itemSlideUp 350ms var(--ease-out-expo) backwards;
    animation-delay: calc(min(var(--i, 0), 12) * 35ms);
  }

  .card:hover,
  .card:focus-visible {
    background: var(--bg-elevated);
    border-color: var(--bg-overlay);
    transform: translateY(-2px);
    outline: none;
  }

  .card.active {
    background: rgba(212, 160, 23, 0.08);
    border-color: rgba(212, 160, 23, 0.45);
    position: relative;
  }

  .art-shell {
    position: relative;
    display: block;
    width: 100%;
    aspect-ratio: 1;
    overflow: hidden;
    border-radius: var(--radius);
    background: var(--bg-overlay);
    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.28);
  }

  .card-thumb {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
    transition: transform 300ms var(--ease-out-expo);
  }

  .card:hover .card-thumb,
  .card:focus-visible .card-thumb {
    transform: scale(1.04);
  }

  .art-fallback {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 100%;
    height: 100%;
    color: var(--accent-light);
    font-size: 3rem;
    font-weight: 800;
    background:
      linear-gradient(135deg, rgba(212, 160, 23, 0.28), rgba(76, 175, 80, 0.14)),
      var(--bg-overlay);
  }

  .art-shade {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgba(0, 0, 0, 0.02) 35%, rgba(0, 0, 0, 0.72) 100%);
    opacity: 0.72;
    pointer-events: none;
  }

  .play-puck {
    position: absolute;
    right: 10px;
    bottom: 10px;
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--accent);
    color: #111;
    box-shadow: 0 8px 20px rgba(0, 0, 0, 0.35);
    opacity: 0;
    transform: translateY(8px) scale(0.94);
    transition: opacity 180ms ease, transform 180ms var(--ease-out-expo);
  }

  .card:hover .play-puck,
  .card:focus-visible .play-puck,
  .card.active .play-puck {
    opacity: 1;
    transform: translateY(0) scale(1);
  }

  .play-triangle {
    width: 0;
    height: 0;
    margin-left: 3px;
    border-top: 8px solid transparent;
    border-bottom: 8px solid transparent;
    border-left: 12px solid currentColor;
  }

  .card-info {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .card-title {
    font-size: 0.92rem;
    font-weight: 650;
    line-height: 1.25;
    color: var(--text-primary);
    display: -webkit-box;
    line-clamp: 2;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    min-height: 2.3em;
  }

  .card-artist {
    font-size: 0.8rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .card-duration {
    position: absolute;
    left: 10px;
    bottom: 12px;
    padding: 3px 7px;
    border-radius: var(--radius-sm);
    background: rgba(0, 0, 0, 0.55);
    font-size: 0.8rem;
    color: var(--text-primary);
    font-variant-numeric: tabular-nums;
  }

  .dl-badge {
    position: absolute;
    top: 8px;
    right: 8px;
    width: 22px;
    height: 22px;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: var(--success);
    color: #0e1a0e;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.45);
  }

  .dl-badge svg {
    width: 13px;
    height: 13px;
  }

  @media (max-width: 700px) {
    .explore { gap: 30px; }

    .card-grid,
    .skeleton-grid {
      gap: 14px;
    }

    .card-grid {
      scroll-padding-inline: 18px;
      padding-inline: 18px;
      margin-inline: -18px;
    }

    .card,
    .skeleton-card {
      flex-basis: 148px;
      width: 148px;
    }

    .card { padding: 8px; }
    .play-puck { width: 34px; height: 34px; }
    .art-fallback { font-size: 2.4rem; }
  }
</style>
