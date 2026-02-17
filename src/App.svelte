<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import SearchBar from "./lib/components/SearchBar.svelte";
  import TrackList from "./lib/components/TrackList.svelte";
  import Explore from "./lib/components/Explore.svelte";
  import PlaylistView from "./lib/components/PlaylistView.svelte";
  import Player from "./lib/components/Player.svelte";
  import { initProgressListener } from "./lib/ipc/bridge";
  import { nav } from "./lib/state/nav.svelte";

  let cleanup: (() => void) | undefined;

  onMount(() => {
    cleanup = initProgressListener();
    return () => cleanup?.();
  });
</script>

<main class="app-shell">
  <Sidebar />

  <div class="main-area">
    <section class="content">
      {#if nav.activeTab === "search"}
        <SearchBar />
        <TrackList />
      {:else if nav.activeTab === "explore"}
        <Explore />
      {:else}
        <PlaylistView />
      {/if}
    </section>

    <Player />
  </div>
</main>

<style>
  .app-shell {
    display: flex;
    height: 100%;
    background: var(--bg-base);
  }

  .main-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 24px;
  }
</style>
