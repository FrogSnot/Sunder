<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import { 
    isPermissionGranted, 
    requestPermission, 
  } from "@tauri-apps/plugin-notification";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import SearchBar from "./lib/components/SearchBar.svelte";
  import TrackList from "./lib/components/TrackList.svelte";
  import Explore from "./lib/components/Explore.svelte";
  import PlaylistView from "./lib/components/PlaylistView.svelte";
  import QueueView from "./lib/components/QueueView.svelte";
  import Player from "./lib/components/Player.svelte";
  import LyricsView from "./lib/components/LyricsView.svelte";
  import { initProgressListener } from "./lib/ipc/bridge";
  import { nav } from "./lib/state/nav.svelte.ts";
  import { player } from "./lib/state/player.svelte.ts";
  import { config } from "./lib/state/config.svelte.ts";
  import { 
    pause, 
    resume, 
    seek, 
    setVolume, 
    playNext, 
    playPrev,
    stop
  } from "./lib/ipc/bridge";

  let cleanup: (() => void) | undefined;

  function handleKeydown(e: KeyboardEvent) {
    // Ignore if typing in an input
    const target = e.target as HTMLElement;
    if (target.tagName === "INPUT" || target.tagName === "TEXTAREA" || target.isContentEditable) {
      if (e.key === "Escape") {
        target.blur();
      }
      return;
    }

    switch (e.key.toLowerCase()) {
      case " ":
        e.preventDefault();
        if (player.isPlaying) pause();
        else resume();
        break;
      case "arrowleft":
        e.preventDefault();
        seek(Math.max(0, player.currentTime - config.current.seek_step_secs));
        break;
      case "arrowright":
        e.preventDefault();
        seek(player.currentTime + config.current.seek_step_secs);
        break;
      case "arrowup":
        e.preventDefault();
        setVolume(Math.min(1, player.volume + config.current.volume_step));
        break;
      case "arrowdown":
        e.preventDefault();
        setVolume(Math.max(0, player.volume - config.current.volume_step));
        break;
      case "n":
        playNext();
        break;
      case "p":
        playPrev();
        break;
      case "f":
        e.preventDefault();
        nav.activeTab = "search";
        // Give it a tiny bit of time to render and then focus
        setTimeout(() => {
          document.querySelector<HTMLInputElement>(".search-bar input")?.focus();
        }, 50);
        break;
    }
  }

  onMount(() => {
    cleanup = initProgressListener();
    config.load();

    // Request notification permissions
    isPermissionGranted().then(granted => {
      if (!granted) {
        requestPermission();
      }
    });

    const unlistens = [
      listen("media-toggle", () => {
        if (player.isPlaying) pause();
        else resume();
      }),
      listen("media-next", () => playNext()),
      listen("media-previous", () => playPrev()),
      listen("media-stop", () => stop()),
    ];

    return () => {
      cleanup?.();
      Promise.all(unlistens).then(funs => funs.forEach(f => f()));
    };
  });
</script>

<svelte:window onkeydown={handleKeydown} />

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

    <Player />
  </div>

  <LyricsView />
</main>

<style>
  .app-shell {
    display: flex;
    flex-direction: row;
    height: 100%;
    width: 100%;
    background: var(--bg-base);
    overflow: hidden;
  }

  .main-area {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    height: 100%;
    overflow: hidden;
  }

  .content {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    padding: 24px 24px 120px;
  }

  .search-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
</style>
