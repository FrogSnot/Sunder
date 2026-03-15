<script lang="ts">
  import { lyricsState } from "../state/lyrics.svelte.ts";
  import { player } from "../state/player.svelte.ts";
  import { fade, scale } from "svelte/transition";

  function close() {
    lyricsState.visible = false;
  }

  function snapToCurrent() {
    lyricsState.autoScroll = true;
    const activeIdx = activeLine;
    if (activeIdx >= 0) {
      const el = document.getElementById(`lrc-line-${activeIdx}`);
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  }


  // Detect manual scroll starts
  let scrollTimeout: ReturnType<typeof setTimeout>;
  function onManualScroll() {
    if (lyricsState.autoScroll && lyricsState.visible) {
      lyricsState.autoScroll = false;
    }
  }

  let activeLine = $derived.by(() => {
    if (!lyricsState.synced || lyricsState.syncedLines.length === 0) return -1;
    const t = player.currentTime;
    
    // Binary search for efficiency since lines are sorted
    let low = 0;
    let high = lyricsState.syncedLines.length - 1;
    let idx = -1;

    while (low <= high) {
      const mid = Math.floor((low + high) / 2);
      if (lyricsState.syncedLines[mid].time <= t) {
        idx = mid;
        low = mid + 1;
      } else {
        high = mid - 1;
      }
    }
    return idx;
  });

  $effect(() => {
    if (activeLine >= 0 && lyricsState.visible && lyricsState.autoScroll) {
      const el = document.getElementById(`lrc-line-${activeLine}`);
      el?.scrollIntoView({ behavior: "smooth", block: "center" });
    }
  });
</script>

{#if lyricsState.visible}
  <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
  <div
    class="lyrics-overlay"
    transition:fade={{ duration: 200 }}
    onclick={close}
    role="presentation"
  >
    <!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
    <div
      class="lyrics-card"
      transition:scale={{ duration: 200, start: 0.95 }}
      onclick={(e) => e.stopPropagation()}
      role="dialog"
      aria-modal="true"
      aria-labelledby="lyrics-title"
      tabindex="-1"
    >
      <div class="lyrics-header">
        <div class="header-left">
          <h3 id="lyrics-title">Lyrics</h3>
          {#if lyricsState.source}
            <span class="source-badge">{lyricsState.source}</span>
          {/if}
        </div>
        <div class="header-right">
          {#if lyricsState.synced && lyricsState.content}
            <button
              class="mode-toggle"
              onclick={() => lyricsState.synced = !lyricsState.synced}
              title={lyricsState.synced ? "Show plain lyrics" : "Show synced lyrics"}
            >
              {lyricsState.synced ? "⏱ Synced" : "📄 Plain"}
            </button>
          {/if}
          <button class="close-btn" onclick={close} aria-label="Close lyrics">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
              <path d="M18 6L6 18M6 6l12 12" />
            </svg>
          </button>
        </div>
      </div>

      <div 
        class="lyrics-body" 
        onwheel={onManualScroll} 
        ontouchmove={onManualScroll}
        role="region"
        aria-label="Lyrics content"
      >
        {#if lyricsState.loading}
          <div class="status">
            <div class="status-content">
              <div class="loader"></div>
              <p>Fetching lyrics...</p>
            </div>
          </div>
        {:else if lyricsState.error}
          <div class="status error">
            <p>{lyricsState.error}</p>
          </div>
        {:else if lyricsState.synced && lyricsState.syncedLines.length > 0}
          <div class="synced-lyrics">
            {#each lyricsState.syncedLines as line, i}
              <button
                id="lrc-line-{i}"
                class="lrc-line"
                class:active={i === activeLine}
                class:past={i < activeLine}
                onclick={() => {
                  if (line.time >= 0) {
                    player.currentTime = line.time;
                    import("../ipc/bridge").then(m => m.seek(line.time));
                  }
                }}
                onkeydown={(e) => {
                  if (e.key === 'Enter' || e.key === ' ') {
                    if (line.time >= 0) {
                      player.currentTime = line.time;
                      import("../ipc/bridge").then(m => m.seek(line.time));
                    }
                  }
                }}
              >
                {line.text}
              </button>
            {/each}
          </div>
        {:else if lyricsState.content}
          <pre class="lyrics-text">{lyricsState.content}</pre>
        {:else}
          <div class="status">
            <p>No lyrics available.</p>
          </div>
        {/if}
      </div>

      {#if !lyricsState.autoScroll && lyricsState.synced && lyricsState.syncedLines.length > 0}
        <button 
          class="snap-btn" 
          onclick={snapToCurrent}
          transition:fade={{ duration: 150 }}
        >
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M12 5v14M5 12l7 7 7-7" />
          </svg>
          Back to current
        </button>
      {/if}
    </div>
  </div>
{/if}

<style>
  .lyrics-overlay {
    position: fixed;
    top: 0;
    left: 0;
    right: 0;
    bottom: 0;
    background: rgba(0, 0, 0, 0.7);
    backdrop-filter: blur(8px);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    padding: 24px;
  }

  .lyrics-card {
    background: var(--bg-elevated);
    border: 1px solid var(--bg-overlay);
    border-radius: var(--radius-lg);
    width: 100%;
    max-width: 600px;
    max-height: 80vh;
    display: flex;
    flex-direction: column;
    box-shadow: 0 20px 50px rgba(0, 0, 0, 0.5);
  }

  .lyrics-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 16px 24px;
    border-bottom: 1px solid var(--bg-overlay);
    gap: 12px;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 10px;
    flex: 1;
    min-width: 0;
  }

  .header-right {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .lyrics-header h3 {
    margin: 0;
    color: var(--accent);
    font-size: 1.1rem;
    white-space: nowrap;
  }

  .source-badge {
    font-size: 0.65rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    padding: 2px 7px;
    border-radius: 999px;
    background: var(--bg-overlay);
    color: var(--text-muted);
    white-space: nowrap;
  }

  .mode-toggle {
    font-size: 0.75rem;
    padding: 4px 10px;
    border-radius: var(--radius-sm);
    background: var(--bg-overlay);
    color: var(--accent);
    transition: background 150ms ease;
    white-space: nowrap;
  }

  .mode-toggle:hover {
    background: var(--accent-dim);
  }

  .close-btn {
    width: 32px;
    height: 32px;
    color: var(--text-muted);
    transition: color 150ms ease;
    flex-shrink: 0;
  }

  .close-btn:hover {
    color: var(--text-primary);
  }

  .close-btn svg {
    width: 100%;
    height: 100%;
  }

  .lyrics-body {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
    scrollbar-width: thin;
    scrollbar-color: var(--bg-overlay) transparent;
  }

  .lyrics-text {
    margin: 0;
    white-space: pre-wrap;
    font-family: inherit;
    font-size: 1.05rem;
    line-height: 1.8;
    color: var(--text-secondary);
    text-align: center;
  }

  /* Synced lyrics styles */
  .synced-lyrics {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    padding: 40px 0;
  }

  .lrc-line {
    font-size: 1.1rem;
    line-height: 1.6;
    color: var(--text-muted);
    text-align: center;
    transition: color 300ms ease, transform 300ms var(--ease-spring), font-size 300ms ease;
    padding: 4px 12px;
    border-radius: var(--radius-sm);
    opacity: 0.4;
    background: none;
    border: none;
    cursor: pointer;
    width: 100%;
    display: block;
  }

  .lrc-line.past {
    opacity: 0.5;
    color: var(--text-secondary);
  }

  .lrc-line.active {
    color: var(--accent);
    font-size: 1.35rem;
    font-weight: 600;
    opacity: 1;
    transform: scale(1.05);
    text-shadow: 0 0 20px rgba(224, 168, 32, 0.3);
  }

  .status {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 200px;
    color: var(--text-muted);
    gap: 16px;
  }

  .status.error {
    color: #ef4444;
  }

  .status-content {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 16px;
  }

  .snap-btn {
    position: absolute;
    bottom: 24px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--accent);
    color: black;
    padding: 10px 20px;
    border-radius: 999px;
    font-size: 0.9rem;
    font-weight: 700;
    display: flex;
    align-items: center;
    gap: 8px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
    transition: transform 200ms var(--ease-spring), background 200ms ease;
    z-index: 10;
  }

  .snap-btn:hover {
    transform: translateX(-50%) scale(1.05);
    background: #f0b830;
  }

  .snap-btn svg {
    width: 18px;
    height: 18px;
  }

  .loader {
    width: 32px;
    height: 32px;
    border: 3px solid var(--bg-overlay);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to { transform: rotate(360deg); }
  }
</style>
