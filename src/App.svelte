<script lang="ts">
  import { onMount } from "svelte";
  import { getCurrentWindow } from "@tauri-apps/api/window";
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
    loadDownloads
  } from "./lib/ipc/bridge";
  import { player } from "./lib/state/player.svelte";
  import { nav } from "./lib/state/nav.svelte";
  import { config } from "./lib/state/config.svelte";
  import { audioRecovery } from "./lib/state/audioRecovery.svelte";
  import { lyricsState } from "./lib/state/lyrics.svelte";

  let cleanup: (() => void) | undefined;
  let unlistenClose: (() => void) | undefined;

  /** Cap on how long a window close waits for the pending save. */
  const CLOSE_FLUSH_TIMEOUT_MS = 1500;

  /** Guards the close handler so destroy() cannot re-enter it. */
  let closing = false;

  // TODO: read from persistent config once PR #10 (settings) is merged
  const seekStep = 5;
  const volStep = 0.05;

  onMount(() => {
    cleanup = initProgressListener();
    config.load().catch(console.error);
    audioRecovery.init().catch(console.error);
    loadDownloads();
    window.addEventListener("keydown", handleKeyDown);
    initCloseFlush();
    return () => {
      cleanup?.();
      unlistenClose?.();
      window.removeEventListener("keydown", handleKeyDown);
    };
  });

  /**
   * Persist the debounced settings before the window closes. Registering the
   * listener can fail outside a Tauri shell (tests, headless), which must not
   * break the app, so the failure is logged and ignored.
   */
  async function initCloseFlush() {
    try {
      const appWindow = getCurrentWindow();
      unlistenClose = await appWindow.onCloseRequested(async (event) => {
        event.preventDefault();
        if (closing) return;
        closing = true;
        try {
          // Never wait indefinitely: a wedged save must not leave the user
          // with a window that cannot be closed.
          await Promise.race([
            config.flush(),
            new Promise((resolve) => setTimeout(resolve, CLOSE_FLUSH_TIMEOUT_MS)),
          ]);
        } catch (e) {
          console.error("Failed to flush settings before closing:", e);
        }
        try {
          // destroy() bypasses onCloseRequested, so this cannot loop.
          await appWindow.destroy();
        } catch (e) {
          // The close was refused (e.g. the ACL lacks allow-destroy, or the
          // window is already gone). Release the guard so a later attempt is
          // not silently swallowed, which would leave the window uncloseable.
          closing = false;
          console.error("Failed to close the window:", e);
        }
      });
    } catch (e) {
      console.error("Window close hook unavailable; settings flush on close disabled:", e);
    }
  }

  async function handleKeyDown(e: KeyboardEvent) {
    const target = e.target as HTMLElement;

    if (
      target instanceof HTMLInputElement ||
      target instanceof HTMLTextAreaElement ||
      target instanceof HTMLSelectElement ||
      target.isContentEditable
    ) {
      return;
    }

    if (e.ctrlKey || e.metaKey || e.altKey) return;

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
        if (!player.currentTrack || player.duration <= 0) break;
        e.preventDefault();
        const newTimeLeft = Math.max(0, player.currentTime - seekStep);
        player.currentTime = newTimeLeft;
        await seek(newTimeLeft);
        break;
      case "arrowright":
        if (!player.currentTrack || player.duration <= 0) break;
        e.preventDefault();
        const newTimeRight = Math.min(player.currentTime + seekStep, player.duration);
        player.currentTime = newTimeRight;
        await seek(newTimeRight);
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
    /* Virtualized lists swap real rows for spacers of equal height. Scroll
       anchoring would treat that swap as a content change and silently rewrite
       scrollTop, which the user sees as the list fighting the wheel. */
    overflow-anchor: none;
  }

  .search-section {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }
</style>
