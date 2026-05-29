<script lang="ts">
  import { downloads } from "../state/downloads.svelte";
  import { downloadTrack, deleteDownload } from "../ipc/bridge";
  import type { Track } from "../types";

  let { track, size = 32 }: { track: Track; size?: number } = $props();

  let downloaded = $derived(downloads.isDownloaded(track.id));
  let progress = $derived(downloads.getProgress(track.id));
  let active = $derived(downloads.isActive(track.id));

  const RADIUS = 9;
  const CIRC = 2 * Math.PI * RADIUS;
  let dashOffset = $derived(
    progress && progress.status === "downloading"
      ? CIRC * (1 - Math.min(progress.percent, 100) / 100)
      : CIRC * 0.75,
  );
  let indeterminate = $derived(
    !!progress && (progress.status === "queued" || progress.status === "converting"),
  );

  let label = $derived(
    downloaded ? "Remove download" : active ? "Downloading" : "Download for offline",
  );

  async function handleClick(e: MouseEvent) {
    e.stopPropagation();
    e.preventDefault();
    if (active) return;
    try {
      if (downloaded) {
        await deleteDownload(track.id);
      } else {
        downloads.register(track);
        await downloadTrack(track.id);
      }
    } catch {
      // download failures surface in the activity panel
    }
  }
</script>

<button
  class="dl-btn"
  class:downloaded
  class:active
  style="--sz: {size}px"
  onclick={handleClick}
  oncontextmenu={(e) => e.stopPropagation()}
  aria-label={label}
  title={label}
>
  {#if active}
    <svg class="ring" class:spin={indeterminate} viewBox="0 0 24 24" aria-hidden="true">
      <circle class="ring-track" cx="12" cy="12" r={RADIUS} />
      <circle
        class="ring-fill"
        cx="12"
        cy="12"
        r={RADIUS}
        stroke-dasharray={CIRC}
        stroke-dashoffset={dashOffset}
      />
    </svg>
    {#if progress?.status === "downloading"}
      <span class="pct">{Math.round(progress.percent)}</span>
    {/if}
  {:else if downloaded}
    <svg class="check" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <polyline points="20 6 9 17 4 12" />
    </svg>
  {:else}
    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
      <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
      <polyline points="7 10 12 15 17 10" />
      <line x1="12" y1="15" x2="12" y2="3" />
    </svg>
  {/if}
</button>

<style>
  .dl-btn {
    width: var(--sz);
    height: var(--sz);
    flex-shrink: 0;
    display: grid;
    place-items: center;
    position: relative;
    border-radius: 50%;
    color: var(--text-muted);
    transition: color 180ms ease, background 180ms ease, transform 150ms ease;
  }

  .dl-btn:hover {
    color: var(--accent);
    background: var(--bg-overlay);
  }

  .dl-btn:active {
    transform: scale(0.9);
  }

  .dl-btn.downloaded {
    color: var(--success);
  }

  .dl-btn.downloaded:hover {
    color: var(--error);
    background: color-mix(in srgb, var(--error) 14%, transparent);
  }

  .dl-btn.active {
    color: var(--accent);
    cursor: default;
  }

  .dl-btn svg:not(.ring) {
    width: 55%;
    height: 55%;
  }

  .check polyline {
    stroke-dasharray: 24;
    stroke-dashoffset: 24;
    animation: check-draw 420ms var(--ease-out-expo) forwards;
  }

  @keyframes check-draw {
    to {
      stroke-dashoffset: 0;
    }
  }

  .ring {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    transform: rotate(-90deg);
  }

  .ring-track {
    fill: none;
    stroke: var(--bg-overlay);
    stroke-width: 2.2;
  }

  .ring-fill {
    fill: none;
    stroke: var(--accent);
    stroke-width: 2.2;
    stroke-linecap: round;
    transition: stroke-dashoffset 220ms ease;
  }

  .ring.spin {
    animation: dl-spin 900ms linear infinite;
  }

  .pct {
    position: absolute;
    font-size: 0.5rem;
    font-weight: 700;
    color: var(--accent);
    font-variant-numeric: tabular-nums;
  }

  @keyframes dl-spin {
    to {
      transform: rotate(270deg);
    }
  }
</style>
