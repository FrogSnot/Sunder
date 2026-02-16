<script lang="ts">
  import { onMount } from "svelte";
  import SearchBar from "./lib/components/SearchBar.svelte";
  import TrackList from "./lib/components/TrackList.svelte";
  import Player from "./lib/components/Player.svelte";
  import { initProgressListener } from "./lib/ipc/bridge";

  let cleanup: (() => void) | undefined;

  onMount(() => {
    cleanup = initProgressListener();
    return () => cleanup?.();
  });
</script>

<main class="app-shell">
  <header class="top-bar">
    <div class="brand">
      <span class="brand-icon">♦</span>
      <span class="brand-name">Sunder</span>
    </div>
    <SearchBar />
  </header>

  <section class="content">
    <TrackList />
  </section>


  <Player />
</main>

<style>
  .app-shell {
    display: flex;
    flex-direction: column;
    height: 100%;
    background: var(--bg-base);
  }

  .top-bar {
    display: flex;
    align-items: center;
    gap: 24px;
    padding: 12px 24px;
    background: var(--bg-surface);
    border-bottom: 1px solid var(--bg-overlay);
    flex-shrink: 0;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .brand-icon {
    font-size: 1.4rem;
    color: var(--accent);
  }

  .brand-name {
    font-size: 1.1rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-primary);
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }
</style>
