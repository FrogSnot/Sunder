<script lang="ts">
  import { pause, resume, stop } from "../ipc/bridge";
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

  let hasTrack = $derived(player.currentTrack !== null);
</script>

<footer class="player" class:visible={hasTrack}>
  {#if player.currentTrack}
    <ProgressBar />

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
        <button class="ctrl-btn" onclick={togglePlay} aria-label={player.isPlaying ? "Pause" : "Play"}>
          {#if player.isBuffering}
            <div class="ctrl-spinner"></div>
          {:else if player.isPlaying}
            <svg viewBox="0 0 24 24" fill="currentColor"><rect x="6" y="4" width="4" height="16" rx="1"/><rect x="14" y="4" width="4" height="16" rx="1"/></svg>
          {:else}
            <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
          {/if}
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
    transition: transform 300ms cubic-bezier(0.4, 0, 0.2, 1);
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
    transition: background var(--transition), transform var(--transition);
  }

  .ctrl-btn:hover {
    background: var(--accent-light);
    transform: scale(1.05);
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
</style>
