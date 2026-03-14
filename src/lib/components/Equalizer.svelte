<script lang="ts">
  import { setEqGains, setEqEnabled } from "../ipc/bridge";
  import { player } from "../state/player.svelte.ts";

  const BANDS = ["32", "64", "125", "250", "500", "1k", "2k", "4k", "8k", "16k"];

  const PRESETS: Record<string, number[]> = {
    Flat: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    "Bass Boost": [6, 5, 4, 2, 0, 0, 0, 0, 0, 0],
    "Treble Boost": [0, 0, 0, 0, 0, 0, 2, 4, 5, 6],
    Vocal: [-2, -1, 0, 2, 4, 4, 2, 0, -1, -2],
    Rock: [4, 3, 1, 0, -1, 0, 1, 3, 4, 4],
    Pop: [-1, 1, 3, 4, 3, 0, -1, -1, 1, 2],
    Jazz: [3, 2, 0, 1, -1, -1, 0, 1, 3, 3],
    Electronic: [4, 3, 1, 0, -2, -1, 0, 3, 4, 3],
    Classical: [3, 2, 1, 0, -1, -1, 0, 1, 2, 3],
  };

  function handleGainChange(index: number, e: Event) {
    const val = parseFloat((e.target as HTMLInputElement).value);
    const gains = [...player.eqGains];
    gains[index] = val;
    player.eqGains = gains;
    player.eqPreset = "Custom";
    setEqGains(gains);
  }

  function applyPreset(name: string) {
    const gains = PRESETS[name];
    if (!gains) return;
    player.eqGains = [...gains];
    player.eqPreset = name;
    setEqGains(gains);
  }

  function toggleEnabled() {
    player.eqEnabled = !player.eqEnabled;
    setEqEnabled(player.eqEnabled);
  }

  function formatGain(db: number): string {
    if (db === 0) return "0";
    return db > 0 ? `+${db}` : `${db}`;
  }
</script>

<div class="eq-panel">
  <div class="eq-header">
    <button
      class="eq-power"
      class:active={player.eqEnabled}
      onclick={toggleEnabled}
      aria-label="Toggle equalizer"
    >
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
        <path d="M18.36 6.64a9 9 0 1 1-12.73 0" />
        <line x1="12" y1="2" x2="12" y2="12" />
      </svg>
    </button>
    <select
      class="eq-preset"
      value={player.eqPreset}
      onchange={(e) => applyPreset((e.target as HTMLSelectElement).value)}
      disabled={!player.eqEnabled}
    >
      {#each Object.keys(PRESETS) as name}
        <option value={name}>{name}</option>
      {/each}
      {#if player.eqPreset === "Custom"}
        <option value="Custom">Custom</option>
      {/if}
    </select>
  </div>

  <div class="eq-body" class:disabled={!player.eqEnabled}>
    <div class="eq-scale">
      <span>+12</span>
      <span>0</span>
      <span>-12</span>
    </div>
    {#each BANDS as band, i}
      <div class="eq-band">
        <span class="eq-db">{formatGain(player.eqGains[i])}</span>
        <div class="eq-slider-wrap">
          <input
            type="range"
            min="-12"
            max="12"
            step="0.5"
            value={player.eqGains[i]}
            oninput={(e) => handleGainChange(i, e)}
            class="eq-slider"
            disabled={!player.eqEnabled}
            aria-label="{band} Hz"
          />
        </div>
        <span class="eq-freq">{band}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .eq-panel {
    padding: 10px 24px 8px;
    border-bottom: 1px solid var(--bg-overlay);
    animation: eqOpen 300ms var(--ease-out-expo);
    overflow: hidden;
  }

  @keyframes eqOpen {
    from {
      max-height: 0;
      opacity: 0;
      padding-top: 0;
      padding-bottom: 0;
    }
    to {
      max-height: 220px;
      opacity: 1;
    }
  }

  .eq-header {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
  }

  .eq-power {
    width: 22px;
    height: 22px;
    color: var(--text-muted);
    transition: color 150ms ease;
    flex-shrink: 0;
  }

  .eq-power:hover {
    color: var(--text-primary);
  }
  .eq-power.active {
    color: var(--accent);
  }
  .eq-power svg {
    width: 100%;
    height: 100%;
  }

  .eq-preset {
    background: var(--bg-overlay);
    color: var(--text-secondary);
    border: none;
    border-radius: var(--radius-sm);
    padding: 4px 8px;
    font-size: 0.75rem;
    cursor: pointer;
    outline: none;
  }
  .eq-preset:disabled {
    opacity: 0.4;
    cursor: default;
  }
  .eq-preset option {
    background: var(--bg-elevated);
  }

  .eq-body {
    display: flex;
    align-items: stretch;
    gap: 0;
    transition: opacity 200ms ease;
  }
  .eq-body.disabled {
    opacity: 0.35;
    pointer-events: none;
  }

  .eq-scale {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: flex-end;
    padding: 18px 8px 20px 0;
    font-size: 0.6rem;
    color: var(--text-muted);
    width: 28px;
    flex-shrink: 0;
  }

  .eq-band {
    display: flex;
    flex-direction: column;
    align-items: center;
    flex: 1;
    min-width: 0;
  }

  .eq-db {
    font-size: 0.6rem;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
    height: 16px;
    line-height: 16px;
  }

  .eq-slider-wrap {
    width: 28px;
    height: 100px;
    position: relative;
    display: flex;
    align-items: center;
    justify-content: center;
    margin: 2px 0;
  }

  .eq-slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100px;
    height: 28px;
    background: transparent;
    outline: none;
    cursor: pointer;
    transform: rotate(-90deg);
    transform-origin: center center;
  }

  .eq-slider::-webkit-slider-runnable-track {
    height: 4px;
    background: var(--bg-overlay);
    border-radius: 2px;
  }

  .eq-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 14px;
    height: 14px;
    margin-top: -5px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    transition: transform 150ms var(--ease-spring);
    box-shadow: 0 0 6px rgba(212, 160, 23, 0.3);
  }

  .eq-slider::-webkit-slider-thumb:hover {
    transform: scale(1.3);
  }

  .eq-slider:disabled {
    cursor: default;
  }

  .eq-freq {
    font-size: 0.6rem;
    color: var(--text-muted);
    height: 16px;
    line-height: 16px;
  }
</style>
