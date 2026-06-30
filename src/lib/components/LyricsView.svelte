<script lang="ts">
  import { lyricsState, type LyricsSearchStage } from "../state/lyrics.svelte";
  import { player } from "../state/player.svelte";
  import { nav } from "../state/nav.svelte";
  import { fetchLyrics } from "../ipc/bridge";
  import WormText from "./WormText.svelte";

  let lyricsContainer = $state<HTMLDivElement | undefined>();

  const stageOrder: LyricsSearchStage[] = ["cache", "lrclib", "lrclib-search", "lyrics-ovh", "subtitles"];
  let currentStageIdx = $derived(stageOrder.indexOf(lyricsState.searchStage));

  // When the panel becomes visible and we have a track, fetch lyrics if not already loaded
  $effect(() => {
    if (lyricsState.visible && player.currentTrack) {
      const t = player.currentTrack;
      if (lyricsState.trackId !== t.id) {
        fetchLyrics(t.id, t.artist, t.title, t.duration_secs);
      }
    }
  });

  // Compute active lyric line index once (not per-line in template)
  let activeLineIdx = $derived.by(() => {
    if (!lyricsState.synced || !lyricsState.visible) return -1;
    return lyricsState.syncedLines.findLastIndex((l) => l.time <= player.currentTime);
  });

  // Auto-scroll to current synced line
  $effect(() => {
    if (!lyricsState.synced || !lyricsState.visible) return;
    const idx = activeLineIdx;
    if (idx >= 0 && lyricsContainer) {
      const el = lyricsContainer.children[idx] as HTMLElement | undefined;
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  });
</script>

<aside
  class="lyrics-panel"
  class:open={lyricsState.visible}
  class:focus-hidden={nav.focusMode}
>
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

  {#if lyricsState.loading}
    <div class="lyrics-progress" role="status" aria-live="polite">
      <div class="progress-bar">
        {#each stageOrder as stage, i}
          <div
            class="segment"
            class:filled={i <= currentStageIdx}
            class:current={i === currentStageIdx}
            data-stage={stage}
          ></div>
        {/each}
      </div>
      <span class="stage-label">
        {#if lyricsState.searchStage === "cache"}
          Checking cache…
        {:else if lyricsState.searchStage === "lrclib" || lyricsState.searchStage === "lrclib-search"}
          Searching LRCLIB…
        {:else if lyricsState.searchStage === "lyrics-ovh"}
          Trying Lyrics.ovh…
        {:else if lyricsState.searchStage === "subtitles"}
          Checking YouTube subtitles…
        {:else}
          Searching…
        {/if}
      </span>
    </div>
  {/if}

  <div class="lyrics-body" bind:this={lyricsContainer}>
    {#if lyricsState.loading}
      <p class="lyrics-status"><WormText text="Searching for lyrics" /></p>
    {:else if lyricsState.error}
      <p class="lyrics-status">{lyricsState.error}</p>
    {:else if lyricsState.synced}
      {#each lyricsState.syncedLines as line, i}
        <p class="lyric-line" class:active={i === activeLineIdx}>{line.text || "\u00A0"}</p>
      {/each}
    {:else if lyricsState.content}
      <pre class="plain-lyrics">{lyricsState.content}</pre>
    {:else}
      <p class="lyrics-status">No lyrics available</p>
    {/if}
  </div>
</aside>

<style>
  .lyrics-panel {
    width: 0;
    min-width: 0;
    height: calc(100vh - var(--player-height));
    overflow: hidden;
    background: var(--bg-surface);
    border-left: 1px solid var(--bg-overlay);
    display: flex;
    flex-direction: column;
    transition: width 280ms var(--ease-out-expo);
    will-change: width;
    contain: layout style;
  }

  .lyrics-panel.open {
    width: 340px;
    min-width: 340px;
  }

  .lyrics-panel.focus-hidden {
    display: none;
  }

  @media (prefers-reduced-motion: reduce) {
    .lyrics-panel { transition: none; }
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

  .lyrics-progress {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--bg-overlay);
  }

  .progress-bar {
    display: flex;
    gap: 4px;
    height: 4px;
  }

  .segment {
    flex: 1;
    background: var(--bg-overlay);
    border-radius: 2px;
    transition: background 200ms ease;
  }

  .segment.filled {
    background: var(--accent);
  }

  .segment.current {
    background: var(--accent);
    opacity: 0.6;
    animation: lyric-stage-pulse 1.2s ease-in-out infinite;
  }

  @keyframes lyric-stage-pulse {
    0%, 100% { opacity: 0.6; }
    50% { opacity: 1; }
  }

  @media (prefers-reduced-motion: reduce) {
    .segment.current { animation: none; opacity: 1; }
  }

  .stage-label {
    font-size: 0.75rem;
    color: var(--text-secondary);
    text-align: center;
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
    padding: 4px 0;
    font-size: 0.9rem;
    color: var(--text-muted);
    transition: color 300ms ease, font-size 300ms ease;
    line-height: 1.6;
    will-change: color;
  }

  .lyric-line.active {
    color: var(--accent);
    font-size: 1rem;
    font-weight: 500;
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
