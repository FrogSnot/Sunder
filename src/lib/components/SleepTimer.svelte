<script lang="ts">
  import { player } from "../state/player.svelte";

  let showMenu = $state(false);

  const presets = [
    { label: "Off", value: 0 },
    { label: "1 min", value: 1 },
    { label: "15 min", value: 15 },
    { label: "30 min", value: 30 },
    { label: "45 min", value: 45 },
    { label: "60 min", value: 60 },
  ];

  function handleSelect(mins: number) {
    player.setSleepTimer(mins);
    showMenu = false;
  }

  function toggleMenu(e: MouseEvent) {
    e.stopPropagation();
    showMenu = !showMenu;
  }

  function closeMenu() {
    showMenu = false;
  }
</script>

<svelte:window onclick={closeMenu} />

<div class="sleep-timer">
  {#if showMenu}
    <div
      class="timer-menu"
      onclick={(e) => e.stopPropagation()}
      onkeydown={(e) => e.key === "Escape" && closeMenu()}
      role="menu"
      tabindex="-1"
    >
      <div class="menu-header">Sleep Timer</div>
      {#each presets as preset}
        <button
          class="menu-item"
          class:active={preset.value === 0 ? player.sleepTimerSetMinutes === null : player.sleepTimerSetMinutes === preset.value}
          onclick={() => handleSelect(preset.value)}
        >
          {preset.label}
        </button>
      {/each}
    </div>
  {/if}

  <button
    class="timer-btn"
    class:active={player.sleepTimerRemaining !== null}
    onclick={toggleMenu}
    aria-label="Sleep Timer"
    title={player.sleepTimerRemaining !== null ? `Sleep Timer: ${player.formattedSleepTimer}` : "Sleep Timer"}
  >
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
      <circle cx="12" cy="12" r="10" />
      <polyline points="12 6 12 12 16 14" />
    </svg>
    {#if player.sleepTimerRemaining !== null}
      <span class="timer-text">{player.formattedSleepTimer}</span>
    {/if}
  </button>
</div>

<style>
  .sleep-timer {
    position: relative;
    display: flex;
    align-items: center;
  }

  .timer-btn {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 32px;
    padding: 0 8px;
    border-radius: var(--radius-sm);
    color: var(--text-secondary);
    transition: all var(--transition);
  }

  .timer-btn:hover {
    background: var(--bg-overlay);
    color: var(--text-primary);
  }

  .timer-btn.active {
    color: var(--accent);
    background: var(--bg-overlay);
  }

  .timer-btn svg {
    width: 14px;
    height: 14px;
  }

  .timer-text {
    font-size: 0.75rem;
    font-variant-numeric: tabular-nums;
    font-weight: 500;
  }

  .timer-menu {
    position: absolute;
    bottom: calc(100% + 12px);
    right: 0;
    width: 140px;
    background: var(--bg-elevated);
    border: 1px solid var(--bg-overlay);
    border-radius: var(--radius);
    padding: 4px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    z-index: 200;
    animation: scaleIn 180ms var(--ease-out-expo);
    transform-origin: bottom right;
  }

  .menu-header {
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--text-muted);
    padding: 6px 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .menu-item {
    width: 100%;
    padding: 8px 10px;
    font-size: 0.85rem;
    text-align: left;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    transition: all 150ms ease;
  }

  .menu-item:hover {
    background: var(--bg-overlay);
    transform: translateX(4px);
  }

  .menu-item.active {
    color: var(--accent);
    font-weight: 600;
  }

  @keyframes scaleIn {
    from { opacity: 0; transform: scale(0.92) translateY(10px); }
    to { opacity: 1; transform: scale(1) translateY(0); }
  }
</style>
