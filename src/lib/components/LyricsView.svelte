<script lang="ts">
  import { lyricsState } from "../state/lyrics.svelte";
  import { player } from "../state/player.svelte";
  import { fetchLyrics } from "../ipc/bridge";
  import WormText from "./WormText.svelte";

  let lyricsContainer = $state<HTMLDivElement | undefined>();

  // When the panel becomes visible and we have a track, fetch lyrics if not already loaded
  $effect(() => {
    if (lyricsState.visible && player.currentTrack) {
      const t = player.currentTrack;
      if (lyricsState.trackId !== t.id) {
        fetchLyrics(t.id, t.artist, t.title, t.duration_secs);
      }
    }
  });

  let activeIndex = $state(-1);
  let lastScrollIdx = $state(-1);
  
  // Update active index
  $effect(() => {
    if (!lyricsState.synced || !lyricsState.visible) {
      activeIndex = -1;
      return;
    }
    const positionSecs = player.currentTime;
    const idx = lyricsState.syncedLines.findLastIndex((l) => l.time <= positionSecs);
    if (idx !== activeIndex) {
      activeIndex = idx;
    }
  });

  // Auto-scroll to current synced line with debounced/change-only logic
  $effect(() => {
    if (activeIndex >= 0 && activeIndex !== lastScrollIdx && lyricsContainer) {
      const el = lyricsContainer.children[activeIndex] as HTMLElement | undefined;
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
      lastScrollIdx = activeIndex;
    }
  });
</script>

{#if lyricsState.visible}
  <aside class="lyrics-panel">
    <div class="lyrics-header">
      <span class="lyrics-title">Lyrics</span>
      {#if lyricsState.source}
        <span class="lyrics-source">{lyricsState.source}</span>
      {/if}
      <button class="close-btn" onclick={() => lyricsState.visible = false} aria-label="Close">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </div>

    <div class="lyrics-body" bind:this={lyricsContainer}>
      {#if lyricsState.loading}
        <p class="lyrics-status"><WormText text="Searching for lyrics" /></p>
      {:else if lyricsState.error}
        <p class="lyrics-status">{lyricsState.error}</p>
      {:else if lyricsState.synced}
        {#each lyricsState.syncedLines as line, i}
          <p 
            class="lyric-line" 
            class:active={i === activeIndex}
          >
            {line.text || "\u00A0"}
          </p>
        {/each}
      {:else if lyricsState.content}
        <pre class="plain-lyrics">{lyricsState.content}</pre>
      {:else}
        <p class="lyrics-status">No lyrics available</p>
      {/if}
    </div>
  </aside>
{/if}

<style>
  .lyrics-panel {
    position: fixed;
    right: 0;
    top: 0;
    bottom: var(--player-height);
    width: 340px;
    background: var(--bg-surface);
    border-left: 1px solid var(--bg-overlay);
    display: flex;
    flex-direction: column;
    z-index: 100;
    animation: slideIn 200ms ease;
  }

  @keyframes slideIn {
    from { transform: translateX(100%); opacity: 0; }
    to { transform: translateX(0); opacity: 1; }
  }

  .lyrics-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 16px;
    border-bottom: 1px solid var(--bg-overlay);
  }

  .lyrics-title {
    font-weight: 600;
    font-size: 0.95rem;
    color: var(--text-primary);
    flex: 1;
  }

  .lyrics-source {
    font-size: 0.75rem;
    color: var(--text-muted);
    background: var(--bg-overlay);
    padding: 2px 8px;
    border-radius: 10px;
  }

  .close-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    transition: color 150ms ease, background 150ms ease;
  }

  .close-btn:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }

  .close-btn svg {
    width: 16px;
    height: 16px;
  }

  .lyrics-body {
    flex: 1;
    overflow-y: auto;
    padding: 16px;
    scroll-behavior: smooth;
  }

  .lyrics-status {
    color: var(--text-muted);
    font-size: 0.85rem;
    text-align: center;
    padding: 32px 0;
  }

  .lyric-line {
    padding: 6px 0;
    font-size: 0.95rem;
    color: var(--text-muted);
    transition: color 400ms ease, transform 400ms var(--ease-out-expo), opacity 400ms ease, text-shadow 400ms ease;
    line-height: 1.6;
    transform-origin: left center;
    will-change: transform, color, opacity;
  }
  
  .lyric-line.active {
    color: var(--text-primary);
    transform: scale(1.1);
    font-weight: 600;
    opacity: 1;
    text-shadow: 0 0 16px rgba(212, 160, 23, 0.4);
  }

  .plain-lyrics {
    font-family: inherit;
    font-size: 0.9rem;
    color: var(--text-secondary);
    line-height: 1.8;
    white-space: pre-wrap;
    word-wrap: break-word;
  }
</style>
