<script lang="ts">
  import { onMount } from "svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import SearchBar from "./lib/components/SearchBar.svelte";
  import TrackList from "./lib/components/TrackList.svelte";
  import Explore from "./lib/components/Explore.svelte";
  import PlaylistView from "./lib/components/PlaylistView.svelte";
  import QueueView from "./lib/components/QueueView.svelte";
  import DownloadsView from "./lib/components/DownloadsView.svelte";
  import Player from "./lib/components/Player.svelte";
  import Toast from "./lib/components/Toast.svelte";
  import LyricsView from "./lib/components/LyricsView.svelte";
  import FocusView from "./lib/components/FocusView.svelte";
  import DownloadActivity from "./lib/components/DownloadActivity.svelte";
  import { 
    initProgressListener,
    pause,
    resume,
    seek,
    setVolume,
    playNext,
    playPrev,
    playTrack,
    restoreQueue,
    loadDownloads
  } from "./lib/ipc/bridge";
  import { player } from "./lib/state/player.svelte";
  import { nav } from "./lib/state/nav.svelte";
  import { config } from "./lib/state/config.svelte";
  import { lyricsState } from "./lib/state/lyrics.svelte";

  let cleanup: (() => void) | undefined;

  // TODO: read from persistent config once PR #10 (settings) is merged
  const seekStep = 5;
  const volStep = 0.05;

  onMount(() => {
    cleanup = initProgressListener();
    config.load().then(() => restoreQueue());
    loadDownloads();
    window.addEventListener("keydown", handleKeyDown);
    return () => {
      cleanup?.();
      window.removeEventListener("keydown", handleKeyDown);
    };
  });

  async function handleKeyDown(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    if (target.tagName.toLowerCase() === "input") {
      const type = (target as HTMLInputElement).type;
      if (type === "text" || type === "search" || type === "password" || type === "range") return;
    }
    if (target.tagName.toLowerCase() === "textarea") return;
    switch (e.key.toLowerCase()) {
      case " ":
        e.preventDefault();
        if (player.isPlaying) {
          await pause();
        } else if (player.currentTrack && player.playbackState === "idle") {
          await playTrack(player.currentTrack);
        } else {
          await resume();
        }
        break;
      case "arrowleft":
        e.preventDefault();
        if (player.currentTrack && player.duration > 0) {
          const newTime = Math.max(0, player.currentTime - seekStep);
          player.currentTime = newTime;
          await seek(newTime);
        }
        break;
      case "arrowright":
        e.preventDefault();
        if (player.currentTrack && player.duration > 0) {
          const newTime = Math.min(player.currentTime + seekStep, player.duration);
          player.currentTime = newTime;
          await seek(newTime);
        }
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
        await playNext(true);
        break;
      case "p":
        e.preventDefault();
        await playPrev(true);
        break;
      case "f":
        e.preventDefault();
        nav.activeTab = "search";
        setTimeout(() => {
          document.querySelector<HTMLInputElement>(".search-bar input")?.focus();
        }, 50);
        break;
      case "v":
        e.preventDefault();
        nav.toggleFocus(!!player.currentTrack);
        break;
    }
  }
</script>

<main class="app-shell">
  {#if !nav.focusMode}
    <Sidebar />
  {/if}

  {#if !nav.focusMode}
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
        {:else if nav.activeTab === "downloads"}
          <DownloadsView />
        {:else}
          <PlaylistView />
        {/if}
      </section>
    </div>
  {/if}

  <FocusView />
  <Player />
  <LyricsView />
  <div class="notify-stack" class:lyrics-shifted={lyricsState.visible}>
    <Toast />
    <DownloadActivity />
  </div>
</main>

<style>
  .notify-stack {
    position: fixed;
    right: 18px;
    bottom: calc(var(--player-height, 92px) + 16px);
    width: 320px;
    max-width: calc(100vw - 36px);
    display: flex;
    flex-direction: column;
    gap: 10px;
    z-index: 70;
    pointer-events: none;
    transition: right 280ms var(--ease-out-expo);
  }

  .notify-stack.lyrics-shifted {
    right: calc(340px + 18px);
  }

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
