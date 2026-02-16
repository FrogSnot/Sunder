<script lang="ts">
  import { setVolume } from "../ipc/bridge";
  import { player } from "../state/player.svelte";

  let muted = $state(false);
  let premuteVolume = $state(0.8);

  function toggleMute() {
    if (muted) {
      muted = false;
      player.volume = premuteVolume;
      setVolume(premuteVolume);
    } else {
      premuteVolume = player.volume;
      muted = true;
      player.volume = 0;
      setVolume(0);
    }
  }

  function handleInput(e: Event) {
    const val = parseFloat((e.target as HTMLInputElement).value);
    player.volume = val;
    muted = val === 0;
    setVolume(val);
  }

  let volumeIcon = $derived(
    muted || player.volume === 0
      ? "muted"
      : player.volume < 0.5
        ? "low"
        : "high",
  );
</script>

<div class="volume-control">
  <button class="vol-btn" onclick={toggleMute} aria-label="Toggle mute">
    {#if volumeIcon === "muted"}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
        <line x1="23" y1="9" x2="17" y2="15" />
        <line x1="17" y1="9" x2="23" y2="15" />
      </svg>
    {:else if volumeIcon === "low"}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
        <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
      </svg>
    {:else}
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polygon points="11 5 6 9 2 9 2 15 6 15 11 19 11 5" />
        <path d="M19.07 4.93a10 10 0 0 1 0 14.14" />
        <path d="M15.54 8.46a5 5 0 0 1 0 7.07" />
      </svg>
    {/if}
  </button>
  <input
    type="range"
    min="0"
    max="1"
    step="0.01"
    value={player.volume}
    oninput={handleInput}
    class="vol-slider"
    aria-label="Volume"
  />
</div>

<style>
  .volume-control {
    display: flex;
    align-items: center;
    gap: 6px;
    min-width: 120px;
  }

  .vol-btn {
    width: 20px;
    height: 20px;
    color: var(--text-secondary);
    transition: color var(--transition);
  }

  .vol-btn:hover {
    color: var(--text-primary);
  }

  .vol-btn svg {
    width: 100%;
    height: 100%;
  }

  .vol-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 80px;
    height: 4px;
    background: var(--bg-overlay);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }

  .vol-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
  }
</style>
