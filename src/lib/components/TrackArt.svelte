<script lang="ts">
  import DownloadButton from "./DownloadButton.svelte";
  import { downloads } from "../state/downloads.svelte";
  import type { Track } from "../types";

  let {
    track,
    size = 48,
    onplay,
    active = false,
    playing = false,
    badge = true,
  }: {
    track: Track;
    size?: number;
    onplay?: (track: Track) => void;
    active?: boolean;
    playing?: boolean;
    badge?: boolean;
  } = $props();

  let badgeSize = $derived(Math.max(18, Math.round(size * 0.46)));
  let isActive = $derived(downloads.isActive(track.id));
  let downloaded = $derived(downloads.isDownloaded(track.id));
</script>

<div class="track-art" class:active style="--art: {size}px">
  <button
    class="ta-play"
    onclick={() => onplay?.(track)}
    disabled={!onplay}
    aria-label="Play {track.title}"
  >
    <img class="ta-thumb" src={track.thumbnail || ""} alt="" loading="lazy" />
    <span class="ta-overlay" class:show={active && playing}>
      {#if active && playing}
        <span class="ta-bars"><i></i><i></i><i></i></span>
      {:else}
        <svg viewBox="0 0 24 24" fill="currentColor" aria-hidden="true"><polygon points="6 4 20 12 6 20 6 4" /></svg>
      {/if}
    </span>
  </button>

  {#if badge}
    <span class="ta-badge" class:visible={downloaded || isActive}>
      <DownloadButton {track} size={badgeSize} />
    </span>
  {/if}
</div>

<style>
  .track-art {
    position: relative;
    width: var(--art);
    height: var(--art);
    flex-shrink: 0;
  }

  .ta-play {
    width: 100%;
    height: 100%;
    border-radius: var(--radius-sm);
    overflow: hidden;
    position: relative;
    display: block;
    cursor: pointer;
  }

  .ta-play:disabled {
    cursor: default;
  }

  .ta-thumb {
    width: 100%;
    height: 100%;
    object-fit: cover;
    background: var(--bg-overlay);
    display: block;
  }

  .ta-overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg-base) 55%, transparent);
    color: #fff;
    opacity: 0;
    transition: opacity 180ms ease;
  }

  .ta-overlay.show {
    opacity: 1;
  }

  .ta-play:hover .ta-overlay,
  .ta-play:focus-visible .ta-overlay {
    opacity: 1;
  }

  .ta-overlay svg {
    width: 42%;
    height: 42%;
    filter: drop-shadow(0 1px 3px rgba(0, 0, 0, 0.5));
  }

  .ta-bars {
    display: flex;
    align-items: flex-end;
    gap: 2px;
    height: 38%;
  }

  .ta-bars i {
    width: 3px;
    background: var(--accent-light);
    border-radius: 2px;
    animation: ta-eq 900ms ease-in-out infinite;
  }

  .ta-bars i:nth-child(1) { height: 60%; animation-delay: -200ms; }
  .ta-bars i:nth-child(2) { height: 100%; animation-delay: -500ms; }
  .ta-bars i:nth-child(3) { height: 75%; animation-delay: -100ms; }

  .ta-badge {
    position: absolute;
    right: -4px;
    bottom: -4px;
    border-radius: 50%;
    background: var(--bg-surface);
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
    display: grid;
    place-items: center;
    opacity: 0;
    transform: scale(0.8);
    transition: opacity 160ms ease, transform 160ms ease;
    z-index: 2;
  }

  .ta-badge.visible,
  .track-art:hover .ta-badge,
  .track-art:focus-within .ta-badge {
    opacity: 1;
    transform: scale(1);
  }

  @keyframes ta-eq {
    0%, 100% { transform: scaleY(0.4); }
    50% { transform: scaleY(1); }
  }
</style>
