<script lang="ts">
  import { pause, resume, stop, playTrack, search } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import VolumeControl from "./VolumeControl.svelte";

  async function togglePlay() {
    if (player.isPlaying) {
      await pause();
    } else {
      await resume();
    }
  }

  async function handlePrev() {
    const prev = player.prevTrack();
    if (prev) await playTrack(prev);
  }

  async function handleNext() {
    const next = player.nextTrack();
    if (next) await playTrack(next);
  }

  function handleShuffle() {
    player.shuffle();
  }

  async function findAlternative() {
    const failed = player.failedTrack;
    if (!failed || player.findingAlt) return;
    player.findingAlt = true;
    player.downloadStage = "finding";
    try {
      const query = `${failed.title} ${failed.artist}`;
      const result = await search(query);
      const alt = result.tracks.find((t) => t.id !== failed.id);
      if (alt) {
        player.failedTrack = null;
        player.findingAlt = false;
        await playTrack(alt);
      } else {
        player.downloadStage = "no-alt";
        player.findingAlt = false;
      }
    } catch (_) {
      player.downloadStage = "no-alt";
      player.findingAlt = false;
    }
  }

  function dismissError() {
    player.failedTrack = null;
    player.downloadStage = "";
    player.lastError = "";
  }

  let hasTrack = $derived(player.currentTrack !== null);
</script>

<footer class="player" class:visible={hasTrack}>
  {#if player.currentTrack}
    <ProgressBar />

    {#if player.downloadStage === "error" || player.downloadStage === "finding" || player.downloadStage === "no-alt"}
      <div class="error-banner">
        <div class="error-banner-left">
          <svg class="dl-error-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="15" y1="9" x2="9" y2="15"/><line x1="9" y1="9" x2="15" y2="15"/></svg>
          <div class="error-banner-text">
            <span class="error-main">Track unavailable</span>
            {#if player.downloadStage === "finding"}
              <span class="error-sub">Searching for alternative...</span>
            {:else if player.downloadStage === "no-alt"}
              <span class="error-sub">No alternative found</span>
            {:else if player.hasNext}
              <span class="error-sub">Auto-skipping in a few seconds</span>
            {:else}
              <span class="error-sub">No more tracks in queue</span>
            {/if}
          </div>
        </div>
        <div class="error-banner-actions">
          {#if player.downloadStage === "finding"}
            <div class="dl-spinner"></div>
          {:else if player.downloadStage !== "no-alt"}
            <button class="error-btn alt-btn" onclick={findAlternative}>
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/></svg>
              Find Alternative
            </button>
          {/if}
          <button class="error-btn dismiss-btn" onclick={dismissError} aria-label="Dismiss">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
          </button>
        </div>
      </div>
    {:else if player.isBuffering && player.downloadStage}
      <div class="download-status">
        {#if player.downloadStage === "downloading"}
          <div class="dl-bar">
            <div class="dl-fill" style="width: {player.downloadPercent}%"></div>
          </div>
          <span class="dl-text">Downloading {Math.round(player.downloadPercent)}%</span>
        {:else if player.downloadStage === "converting"}
          <div class="dl-spinner"></div>
          <span class="dl-text">Converting audio...</span>
        {:else if player.downloadStage === "extracting"}
          <div class="dl-spinner"></div>
          <span class="dl-text">Fetching stream info...</span>
        {:else}
          <div class="dl-spinner"></div>
          <span class="dl-text">Preparing...</span>
        {/if}
      </div>
    {/if}

    <div class="player-body">
      <div class="now-playing">
        <img
          class="np-thumb"
          src={player.currentTrack.thumbnail || ""}
          alt=""
        />
        <div class="np-info">
          <span class="np-title">{player.currentTrack.title}</span>
          <span class="np-artist">{player.currentTrack.artist}</span>
        </div>
      </div>

      <div class="controls">
        <button class="ctrl-btn ctrl-sm" onclick={handleShuffle} aria-label="Shuffle" class:active-toggle={player.shuffled} disabled={player.queue.length < 2}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="16 3 21 3 21 8" /><line x1="4" y1="20" x2="21" y2="3" />
            <polyline points="21 16 21 21 16 21" /><line x1="15" y1="15" x2="21" y2="21" />
            <line x1="4" y1="4" x2="9" y2="9" />
          </svg>
        </button>
        <button class="ctrl-btn ctrl-sm" onclick={handlePrev} aria-label="Previous" disabled={!player.hasPrev}>
          <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="19 20 9 12 19 4 19 20"/><line x1="5" y1="4" x2="5" y2="20" stroke="currentColor" stroke-width="2"/></svg>
        </button>
        <button class="ctrl-btn" onclick={togglePlay} aria-label={player.isPlaying ? "Pause" : "Play"}>
          {#if player.isBuffering}
            <div class="ctrl-spinner"></div>
          {:else if player.isPlaying}
            <svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/></svg>
          {:else}
            <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
          {/if}
        </button>
        <button class="ctrl-btn ctrl-sm" onclick={handleNext} aria-label="Next" disabled={!player.hasNext}>
          <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="5 4 15 12 5 20 5 4"/><line x1="19" y1="4" x2="19" y2="20" stroke="currentColor" stroke-width="2"/></svg>
        </button>
        <button class="ctrl-btn ctrl-sm" onclick={stop} aria-label="Stop">
          <svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="6" width="12" height="12" rx="2"/></svg>
        </button>
      </div>

      <div class="right-section">
        <span class="time-display">
          {player.formattedTime} / {player.formattedDuration}
        </span>
        <VolumeControl />
      </div>
    </div>
  {/if}
</footer>

<style>
  .player {
    position: fixed;
    bottom: 0;
    left: 0;
    right: 0;
    background: var(--bg-surface);
    border-top: 1px solid var(--bg-overlay);
    transform: translateY(100%);
    transition: transform 450ms var(--ease-out-expo);
    z-index: 100;
  }

  .player.visible {
    transform: translateY(0);
  }

  .player-body {
    display: flex;
    align-items: center;
    padding: 8px 24px 12px;
    gap: 24px;
  }

  .now-playing {
    display: flex;
    align-items: center;
    gap: 12px;
    flex: 1;
    min-width: 0;
  }

  .np-thumb {
    width: 44px;
    height: 44px;
    border-radius: var(--radius-sm);
    object-fit: cover;
    background: var(--bg-overlay);
    flex-shrink: 0;
    box-shadow: 0 0 12px rgba(212, 160, 23, 0.2);
    transition: box-shadow 300ms ease;
  }

  .np-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .np-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .np-artist {
    font-size: 0.75rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 12px;
    flex-shrink: 0;
  }

  .ctrl-btn {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    border-radius: 50%;
    background: var(--accent);
    color: #121212;
    transition: background 150ms ease, transform 150ms var(--ease-spring);
  }

  .ctrl-btn:hover {
    background: var(--accent-light);
    transform: scale(1.08);
  }

  .ctrl-btn:active {
    transform: scale(0.92);
  }

  .ctrl-btn svg {
    width: 18px;
    height: 18px;
  }

  .ctrl-btn.ctrl-sm {
    width: 32px;
    height: 32px;
    background: var(--bg-overlay);
    color: var(--text-secondary);
  }

  .ctrl-btn.ctrl-sm:hover {
    background: var(--bg-elevated);
    color: var(--text-primary);
    transform: scale(1.08);
  }

  .ctrl-btn.ctrl-sm:active {
    transform: scale(0.9);
  }

  .ctrl-btn.ctrl-sm:disabled {
    opacity: 0.3;
    cursor: default;
    pointer-events: none;
  }

  .ctrl-btn.ctrl-sm.active-toggle {
    color: var(--accent);
  }

  .ctrl-btn.ctrl-sm svg {
    width: 14px;
    height: 14px;
  }

  .ctrl-spinner {
    width: 18px;
    height: 18px;
    border: 2px solid rgba(18, 18, 18, 0.3);
    border-top-color: #121212;
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .right-section {
    display: flex;
    align-items: center;
    gap: 16px;
    flex: 1;
    justify-content: flex-end;
  }

  .time-display {
    font-size: 0.75rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .download-status {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 24px 4px;
  }

  .dl-bar {
    flex: 1;
    height: 3px;
    background: var(--bg-overlay);
    border-radius: 2px;
    overflow: hidden;
  }

  .dl-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent) 0%, var(--accent-light) 50%, var(--accent) 100%);
    background-size: 200% 100%;
    border-radius: 2px;
    transition: width 200ms ease;
    animation: shimmer 2s ease-in-out infinite;
  }

  .dl-spinner {
    width: 12px;
    height: 12px;
    border: 2px solid var(--bg-overlay);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
    flex-shrink: 0;
  }

  .dl-text {
    font-size: 0.7rem;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .error-banner {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 6px 24px;
    background: rgba(224, 108, 117, 0.08);
    border-bottom: 1px solid rgba(224, 108, 117, 0.15);
    animation: toastSlide 300ms var(--ease-out-expo);
  }

  .error-banner-left {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .error-banner-text {
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .error-main {
    font-size: 0.8rem;
    font-weight: 600;
    color: #e06c75;
  }

  .error-sub {
    font-size: 0.7rem;
    color: var(--text-muted);
  }

  .error-banner-actions {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .error-btn {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 5px 10px;
    border-radius: var(--radius);
    font-size: 0.75rem;
    font-weight: 500;
    transition: background 150ms ease, transform 150ms var(--ease-spring);
  }

  .error-btn:hover { transform: scale(1.05); }
  .error-btn:active { transform: scale(0.95); }
  .error-btn svg { width: 13px; height: 13px; }

  .alt-btn {
    background: var(--accent-dim);
    color: var(--accent-light);
  }
  .alt-btn:hover { background: var(--accent); color: #121212; }

  .dismiss-btn {
    color: var(--text-muted);
    padding: 5px;
  }
  .dismiss-btn:hover { color: var(--text-primary); }

  .dl-error-icon {
    width: 14px;
    height: 14px;
    color: #e06c75;
    flex-shrink: 0;
  }
</style>
