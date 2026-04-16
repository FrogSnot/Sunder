<script lang="ts">
  import { pause, resume, stop, playTrack, playPrev, search, setSpeed, setDiscordRpc } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { config } from "../state/config.svelte";
  import ProgressBar from "./ProgressBar.svelte";
  import VolumeControl from "./VolumeControl.svelte";
  import Equalizer from "./Equalizer.svelte";
  import SleepTimer from "./SleepTimer.svelte";
  import { lyricsState } from "../state/lyrics.svelte";
  import { nav } from "../state/nav.svelte";

  let showMoreMenu = $state(false);
  let moreMenuRef = $state<HTMLElement | null>(null);

  async function togglePlay() {
    if (player.isPlaying) {
      await pause();
    } else if (player.currentTrack && player.playbackState === "idle") {
      await playTrack(player.currentTrack);
    } else {
      await resume();
    }
  }

  async function handlePrev() {
    await playPrev(true);
  }

  async function handleNext() {
    const next = player.nextTrack(true);
    if (next) await playTrack(next);
  }

  function handleShuffle() {
    player.shuffle();
  }

  function handleRepeat() {
    player.cycleRepeat();
  }

  function toggleMoreMenu(e: MouseEvent) {
    e.stopPropagation();
    showMoreMenu = !showMoreMenu;
    if (showMoreMenu) lyricsState.visible = false;
  }

  function closeMoreMenu() {
    showMoreMenu = false;
  }

  $effect(() => {
    if (showMoreMenu) {
      window.addEventListener("click", closeMoreMenu);
      return () => window.removeEventListener("click", closeMoreMenu);
    }
  });

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

  async function toggleDiscord() {
    const newState = !config.current.discord_rpc_enabled;
    config.current.discord_rpc_enabled = newState;
    await setDiscordRpc(newState, player.currentTrack && player.isPlaying ? player.currentTrack : undefined);
    config.save();
  }

  let hasTrack = $derived(player.currentTrack !== null);
</script>

<footer class="player" class:visible={hasTrack}>
  {#if player.currentTrack}
    <ProgressBar />

    {#if player.downloadStage === "error" || player.downloadStage === "finding" || player.downloadStage === "no-alt"}
      <div class="status-overlay">
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
      </div>
    {:else if player.isBuffering && player.downloadStage}
      <div class="status-overlay">
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
      </div>
    {/if}

    {#if player.showEq}
      <Equalizer />
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
        <button class="ctrl-btn ctrl-sm" onclick={handlePrev} aria-label="Previous" disabled={!player.hasPrev && player.currentTime <= 5}>
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
        <button
          class="ctrl-btn ctrl-sm"
          class:active-toggle={nav.focusMode}
          onclick={() => nav.toggleFocus(!!player.currentTrack)}
          aria-label="Focus View"
          title="Focus View (V)"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M15 3h6v6M9 21H3v-6M21 3l-7 7M3 21l7-7" />
          </svg>
        </button>
        <button
          class="ctrl-btn ctrl-sm"
          class:active-toggle={player.repeatMode !== "off"}
          onclick={handleRepeat}
          aria-label={player.repeatMode === "track" ? "Repeat Track" : player.repeatMode === "queue" ? "Repeat Queue" : "Repeat"}
          title={player.repeatMode === "track" ? "Repeat Track" : player.repeatMode === "queue" ? "Repeat Queue" : "Repeat"}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <polyline points="17 1 21 5 17 9" />
            <path d="M3 11V9a4 4 0 0 1 4-4h14" />
            <polyline points="7 23 3 19 7 15" />
            <path d="M21 13v2a4 4 0 0 1-4 4H3" />
            {#if player.repeatMode === "track"}
              <text x="12" y="14" font-size="7" text-anchor="middle" stroke="none" fill="currentColor" font-weight="bold">1</text>
            {/if}
          </svg>
        </button>
        <button
          class="ctrl-btn ctrl-sm"
          class:active-toggle={lyricsState.visible}
          onclick={() => { lyricsState.visible = !lyricsState.visible; if (lyricsState.visible) showMoreMenu = false; }}
          aria-label="Lyrics"
          title="Lyrics"
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
            <path d="M12 2a3 3 0 0 1 3 3v7a3 3 0 0 1-6 0V5a3 3 0 0 1 3-3zM19 10v2a7 7 0 0 1-14 0v-2M12 19v3M9 22h6" />
          </svg>
        </button>
        <div class="more-controls">
          <button
            class="ctrl-btn ctrl-sm"
            class:active-toggle={showMoreMenu || player.showEq || player.sleepTimerRemaining !== null}
            onclick={toggleMoreMenu}
            aria-label="Playback Controls"
            aria-haspopup="menu"
            aria-expanded={showMoreMenu}
            title="Playback Controls"
          >
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
              <circle cx="12" cy="5" r="1.5" fill="currentColor" /><circle cx="12" cy="12" r="1.5" fill="currentColor" /><circle cx="12" cy="19" r="1.5" fill="currentColor" />
            </svg>
          </button>
          {#if showMoreMenu}
            <div
              bind:this={moreMenuRef}
              class="more-menu"
              onclick={(e) => e.stopPropagation()}
              onkeydown={(e) => { if (e.key === "Escape") closeMoreMenu(); }}
              role="menu"
              tabindex="-1"
            >
              <div class="more-menu-header">Playback Controls</div>
              <button
                class="more-menu-item"
                class:active={player.showEq}
                onclick={() => { player.showEq = !player.showEq; closeMoreMenu(); }}
                role="menuitem"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <line x1="4" y1="21" x2="4" y2="14" /><line x1="4" y1="10" x2="4" y2="3" />
                  <line x1="12" y1="21" x2="12" y2="12" /><line x1="12" y1="8" x2="12" y2="3" />
                  <line x1="20" y1="21" x2="20" y2="16" /><line x1="20" y1="12" x2="20" y2="3" />
                  <line x1="1" y1="14" x2="7" y2="14" /><line x1="9" y1="8" x2="15" y2="8" /><line x1="17" y1="16" x2="23" y2="16" />
                </svg>
                <span>Equalizer</span>
                {#if player.eqEnabled}<span class="more-badge">ON</span>{/if}
              </button>
              <button
                class="more-menu-item"
                class:active={config.current.notifications_enabled}
                onclick={() => config.update({ notifications_enabled: !config.current.notifications_enabled })}
                role="menuitem"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M18 8A6 6 0 0 0 6 8c0 7-3 9-3 9h18s-3-2-3-9" />
                  <path d="M13.73 21a2 2 0 0 1-3.46 0" />
                </svg>
                <span>Notifications</span>
                <span class="more-badge">{config.current.notifications_enabled ? "ON" : "OFF"}</span>
              </button>
              <button
                class="more-menu-item"
                class:active={config.current.discord_rpc_enabled}
                onclick={toggleDiscord}
                role="menuitem"
              >
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                  <path d="M5.6 18.4a9 9 0 0 1 0-12.8" />
                  <path d="M8.5 15.5a5 5 0 0 1 0-7" />
                  <circle cx="12" cy="12" r="1" fill="currentColor" />
                  <path d="M15.5 8.5a5 5 0 0 1 0 7" />
                  <path d="M18.4 5.6a9 9 0 0 1 0 12.8" />
                </svg>
                <span>Discord Presence</span>
                <span class="more-badge">{config.current.discord_rpc_enabled ? "ON" : "OFF"}</span>
              </button>
              <div class="more-menu-divider"></div>
              <div class="speed-control">
                <div class="speed-header">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polygon points="13 2 3 14 12 14 11 22 21 10 12 10 13 2" />
                  </svg>
                  <span>Speed</span>
                  <button
                    class="speed-reset"
                    class:hidden={player.speed === 1.0}
                    onclick={() => setSpeed(1.0)}
                  >
                    Reset
                  </button>
                  <span class="more-badge">{player.speed.toFixed(2)}x</span>
                </div>
                <input
                  type="range"
                  min="0.25"
                  max="3"
                  step="0.05"
                  value={player.speed}
                  oninput={(e) => setSpeed(parseFloat((e.target as HTMLInputElement).value))}
                  class="speed-slider"
                  aria-label="Playback speed"
                />
                <div class="speed-marks">
                  <span>0.25x</span>
                  <span>3x</span>
                </div>
              </div>
              <div class="more-menu-divider"></div>
              <div class="more-menu-sleep">
                <SleepTimer />
              </div>
            </div>
          {/if}
        </div>
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

  .status-overlay {
    position: absolute;
    bottom: 100%;
    left: 0;
    right: 0;
    background: var(--bg-surface);
    border-top: 1px solid var(--bg-overlay);
    z-index: 10;
  }

  .download-status {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 24px;
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

  .more-controls {
    position: relative;
    display: flex;
    align-items: center;
  }

  .more-menu {
    position: absolute;
    bottom: calc(100% + 12px);
    right: 0;
    width: 200px;
    background: var(--bg-elevated);
    border: 1px solid var(--bg-overlay);
    border-radius: var(--radius);
    padding: 4px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    z-index: 200;
    animation: scaleIn 180ms var(--ease-out-expo);
    transform-origin: bottom right;
  }

  .more-menu-header {
    font-size: 0.7rem;
    font-weight: 600;
    color: var(--text-muted);
    padding: 6px 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .more-menu-item {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    font-size: 0.85rem;
    text-align: left;
    border-radius: var(--radius-sm);
    color: var(--text-primary);
    transition: all 150ms ease;
  }

  .more-menu-item:hover {
    background: var(--bg-overlay);
  }

  .more-menu-item.active {
    color: var(--accent);
  }

  .more-menu-item svg {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
  }

  .more-badge {
    margin-left: auto;
    font-size: 0.65rem;
    font-weight: 700;
    color: var(--accent);
    letter-spacing: 0.04em;
  }

  .more-menu-divider {
    height: 1px;
    background: var(--bg-overlay);
    margin: 4px 6px;
  }

  .speed-control {
    padding: 6px 10px;
  }

  .speed-header {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-primary);
    margin-bottom: 6px;
  }

  .speed-header svg {
    width: 16px;
    height: 16px;
    flex-shrink: 0;
    color: var(--text-secondary);
  }

  .speed-reset {
    margin-left: auto;
    background: none;
    border: 1px solid var(--border-primary);
    color: var(--text-secondary);
    font-size: 11px;
    padding: 1px 6px;
    border-radius: var(--radius-sm);
    cursor: pointer;
    transition: all 0.15s;
  }

  .speed-reset:hover {
    background: var(--bg-overlay);
    color: var(--text-primary);
  }

  .speed-reset.hidden {
    display: none;
  }

  .speed-slider {
    width: 100%;
    height: 4px;
    -webkit-appearance: none;
    appearance: none;
    background: var(--bg-overlay);
    border-radius: 2px;
    outline: none;
    cursor: pointer;
  }

  .speed-slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    appearance: none;
    width: 14px;
    height: 14px;
    border-radius: 50%;
    background: var(--accent);
    cursor: pointer;
    border: none;
  }

  .speed-marks {
    display: flex;
    justify-content: space-between;
    font-size: 10px;
    color: var(--text-secondary);
    margin-top: 2px;
    padding: 0 7px;
    opacity: 0.6;
  }

  .more-menu-sleep {
    padding: 4px 6px;
  }

  .more-menu-sleep :global(.sleep-timer) {
    width: 100%;
  }

  .more-menu-sleep :global(.timer-btn) {
    width: 100%;
    border-radius: var(--radius-sm);
    justify-content: flex-start;
    gap: 10px;
    padding: 8px 10px;
    height: auto;
  }

  .more-menu-sleep :global(.timer-btn svg) {
    width: 16px;
    height: 16px;
  }

  .more-menu-sleep :global(.timer-menu) {
    left: 0;
    right: auto;
    bottom: calc(100% + 4px);
  }
</style>
