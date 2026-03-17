<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import SearchBar from "./lib/components/SearchBar.svelte";
  import TrackList from "./lib/components/TrackList.svelte";
  import Explore from "./lib/components/Explore.svelte";
  import PlaylistView from "./lib/components/PlaylistView.svelte";
  import QueueView from "./lib/components/QueueView.svelte";
  import Player from "./lib/components/Player.svelte";
  import Toast from "./lib/components/Toast.svelte";
  import LyricsView from "./lib/components/LyricsView.svelte";
  import { 
    initProgressListener,
    pause,
    resume,
    seek,
    setVolume,
    playNext,
    playPrev
  } from "./lib/ipc/bridge";
  import { player } from "./lib/state/player.svelte";
  import { nav } from "./lib/state/nav.svelte";
  import { config } from "./lib/state/config.svelte";
  import { setEqEnabled, setEqGains } from "./lib/ipc/bridge";

  let cleanup: (() => void) | undefined;

  // TODO: read from persistent config once PR #10 (settings) is merged
  const seekStep = 5;
  const volStep = 0.05;

  onMount(() => {
    (async () => {
      cleanup = initProgressListener();
      window.addEventListener("keydown", handleKeyDown);

      // Initial config load and synchronization
      await config.load();
      player.volume = config.current.volume;
      player.eqEnabled = config.current.eq_enabled;
      player.eqGains = [...config.current.eq_gains];

      // Sync backend EQ state with loaded config if needed
      await setVolume(player.volume);
      await setEqEnabled(player.eqEnabled);
      await setEqGains(player.eqGains);
    })();

    return () => {
      cleanup?.();
      window.removeEventListener("keydown", handleKeyDown);
    };
  });

  async function handleKeyDown(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (target.tagName.toLowerCase() === "input") {
      const type = (target as HTMLInputElement).type;
      if (type === "text" || type === "search" || type === "password") return;
    }
    if (target.tagName.toLowerCase() === "textarea") return;

    switch (e.key.toLowerCase()) {
      case " ":
        e.preventDefault();
        if (player.isPlaying) {
          await pause();
        } else {
          await resume();
        }
        break;
      case "arrowleft":
        e.preventDefault();
        await seek(Math.max(0, player.currentTime - seekStep));
        break;
      case "arrowright":
        e.preventDefault();
        await seek(player.currentTime + seekStep);
        break;
      case "arrowup":
        e.preventDefault();
        const newVolUp = Math.min(1, player.volume + volStep);
        await setVolume(newVolUp);
        break;
      case "arrowdown":
        e.preventDefault();
        const newVolDown = Math.max(0, player.volume - volStep);
        await setVolume(newVolDown);
        break;
      case "n":
        e.preventDefault();
        await playNext();
        break;
      case "p":
        e.preventDefault();
        await playPrev();
        break;
      case "f":
        e.preventDefault();
        nav.activeTab = "search";
        setTimeout(() => {
          document.querySelector<HTMLInputElement>(".search-bar input")?.focus();
        }, 50);
        break;
    }
  }
</script>

<main class="app-shell">
  <Sidebar />

  <div class="main-area">
    <section class="content">
      {#if nav.activeTab === "search"}
        <div class="search-section">
          <SearchBar />
          <TrackList />
        </div>
      {:else if nav.activeTab === "explore"}
        <Explore />
      {:else if nav.activeTab === "queue"}
        <QueueView />
      {:else}
        <PlaylistView />
      {/if}
    </section>
  </div>

  <Player />
  <Toast />
  <LyricsView />
</main>

<style>
  .app-shell {
    display: flex;
    flex-direction: row;
    height: 100vh;
    width: 100vw;
    background: var(--bg-base);
    overflow: hidden;
  }

  .main-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    height: 100vh;
    overflow: hidden;
  }

  .content {
    flex: 1;
    overflow-y: auto;
    padding: 24px 24px 120px;
  }

  .search-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
</style>
