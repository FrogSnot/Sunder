<script lang="ts">
  import { player } from "../state/player.svelte";
  import { nav } from "../state/nav.svelte";
  import WormText from "./WormText.svelte";

  let track = $derived(player.currentTrack);

  $effect(() => {
    if (nav.focusMode && !track) nav.focusMode = false;
  });
</script>

{#if nav.focusMode && track}
  <div class="focus-overlay">
    <div class="focus-bg" style="background-image: url({track.thumbnail || ''})"></div>
    <div class="focus-content">
      <img class="focus-art" src={track.thumbnail || ""} alt="" />
      <div class="focus-info">
        <span class="focus-title"><WormText text={track.title} /></span>
        <span class="focus-artist">{track.artist}</span>
      </div>
    </div>
  </div>
{/if}

<style>
  .focus-overlay {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: flex;
    align-items: center;
    justify-content: center;
    animation: fadeIn 400ms var(--ease-out-expo);
    contain: layout style;
  }

  .focus-bg {
    position: absolute;
    inset: -40px;
    background-size: cover;
    background-position: center;
    filter: blur(40px) brightness(0.3) saturate(1.4);
    transform: scale(1.2);
    will-change: filter, transform;
    image-rendering: auto;
  }

  .focus-content {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 24px;
    padding-bottom: 100px;
    animation: scaleIn 500ms var(--ease-out-expo);
  }

  .focus-art {
    width: min(55vw, 420px);
    height: min(55vw, 420px);
    border-radius: var(--radius-lg);
    object-fit: cover;
    box-shadow: 0 16px 64px rgba(0, 0, 0, 0.6), 0 0 40px rgba(212, 160, 23, 0.15);
  }

  .focus-info {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    max-width: 420px;
    text-align: center;
  }

  .focus-title {
    font-size: 1.3rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .focus-artist {
    font-size: 0.95rem;
    color: var(--text-secondary);
  }
</style>
