<script lang="ts">
  import { listPlaylists, addToPlaylist, removeFromPlaylist, playlistsContainingTrack } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { nav } from "../state/nav.svelte";
  import type { Playlist, Track } from "../types";

  let visible = $state(false);
  let x = $state(0);
  let y = $state(0);
  let track = $state<Track | null>(null);
  let playlists = $state<Playlist[]>([]);
  let showPlaylists = $state(false);
  let toast = $state("");
  let trackPlaylistIds = $state<Set<number>>(new Set());

  let inQueue = $derived(track !== null && player.queue.some((t) => t.id === track!.id));
  let inPlaylist = $derived(nav.activePlaylistId !== null);

  export function open(e: MouseEvent, t: Track) {
    e.preventDefault();
    e.stopPropagation();
    track = t;
    showPlaylists = false;

    const vw = window.innerWidth;
    const vh = window.innerHeight;
    x = Math.min(e.clientX, vw - 200);
    y = Math.min(e.clientY, vh - 200);
    visible = true;
  }

  function close() {
    visible = false;
    showPlaylists = false;
  }

  function handlePlayNext() {
    if (!track) return;
    player.playNext(track);
    showToast("Playing next");
    close();
  }

  function handleAddToQueue() {
    if (!track) return;
    player.addToQueue(track);
    showToast("Added to queue");
    close();
  }

  async function expandPlaylists() {
    try {
      const [pls, ids] = await Promise.all([
        listPlaylists(),
        track ? playlistsContainingTrack(track.id) : Promise.resolve([]),
      ]);
      playlists = pls;
      trackPlaylistIds = new Set(ids);
    } catch (_) {}
    showPlaylists = true;
  }

  async function handleAdd(playlistId: number) {
    if (!track) return;
    try {
      await addToPlaylist(playlistId, track.id);
      showToast("Added to playlist");
    } catch (_) {
      showToast("Failed to add");
    }
    close();
  }

  function handleRemoveFromQueue() {
    if (!track) return;
    const idx = player.queue.findIndex((t) => t.id === track!.id);
    if (idx !== -1) player.removeFromQueue(idx);
    showToast("Removed from queue");
    close();
  }

  async function handleRemoveFromPlaylist() {
    if (!track || nav.activePlaylistId === null) return;
    try {
      await removeFromPlaylist(nav.activePlaylistId, track.id);
      showToast("Removed from playlist");
    } catch (_) {
      showToast("Failed to remove");
    }
    close();
  }

  function showToast(msg: string) {
    toast = msg;
    setTimeout(() => { toast = ""; }, 2000);
  }
</script>

<svelte:window onclick={close} oncontextmenu={() => { if (visible) close(); }} />

{#if visible}
  <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
  <div class="ctx-menu" style="left: {x}px; top: {y}px" onclick={(e) => e.stopPropagation()}>
    {#if !showPlaylists}
      <button class="ctx-item" onclick={handlePlayNext}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/><line x1="22" y1="3" x2="22" y2="21"/></svg>
        Play next
      </button>
      <button class="ctx-item" onclick={handleAddToQueue}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>
        Add to queue
      </button>
      <div class="ctx-divider"></div>
      <button class="ctx-item" onclick={expandPlaylists}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        Add to playlist
      </button>
      {#if inQueue || inPlaylist}
        <div class="ctx-divider"></div>
        {#if inQueue}
          <button class="ctx-item ctx-danger" onclick={handleRemoveFromQueue}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="18" y1="3" x2="6" y2="15"/></svg>
            Remove from queue
          </button>
        {/if}
        {#if inPlaylist}
          <button class="ctx-item ctx-danger" onclick={handleRemoveFromPlaylist}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/><line x1="18" y1="3" x2="6" y2="15"/></svg>
            Remove from playlist
          </button>
        {/if}
      {/if}
    {:else}
      <div class="ctx-header">
        <button class="ctx-back" onclick={() => { showPlaylists = false; }} aria-label="Back">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
        </button>
        <span>Choose playlist</span>
      </div>
      {#if playlists.length === 0}
        <span class="ctx-empty">No playlists yet</span>
      {:else}
        {#each playlists as p (p.id)}
          {@const alreadyIn = trackPlaylistIds.has(p.id)}
          <button class="ctx-item" class:ctx-muted={alreadyIn} onclick={() => { if (!alreadyIn) handleAdd(p.id); }} disabled={alreadyIn}>
            {#if alreadyIn}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="20 6 9 17 4 12"/></svg>
            {:else}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>
            {/if}
            {p.name}
          </button>
        {/each}
      {/if}
    {/if}
  </div>
{/if}

{#if toast}
  <div class="toast">{toast}</div>
{/if}

<style>
  .ctx-menu {
    position: fixed;
    z-index: 200;
    min-width: 180px;
    background: var(--bg-elevated);
    border: 1px solid var(--bg-overlay);
    border-radius: var(--radius);
    padding: 4px;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.5);
    animation: scaleIn 180ms var(--ease-out-expo);
    transform-origin: top left;
  }

  .ctx-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 8px 10px;
    font-size: 0.85rem;
    color: var(--text-primary);
    border-radius: var(--radius-sm);
    text-align: left;
    transition: background 150ms ease, transform 150ms ease;
  }

  .ctx-item:hover {
    background: var(--bg-overlay);
    transform: translateX(2px);
  }

  .ctx-item:active {
    transform: scale(0.98);
  }
  .ctx-item svg { width: 14px; height: 14px; flex-shrink: 0; color: var(--text-muted); }

  .ctx-item.ctx-danger { color: #e06c75; }
  .ctx-item.ctx-danger svg { color: #e06c75; }
  .ctx-item.ctx-danger:hover { background: rgba(224, 108, 117, 0.12); }

  .ctx-item.ctx-muted { color: var(--text-muted); opacity: 0.6; cursor: default; }
  .ctx-item.ctx-muted:hover { background: none; transform: none; }
  .ctx-item.ctx-muted svg { color: var(--success); }

  .ctx-divider {
    height: 1px;
    background: var(--bg-overlay);
    margin: 4px 8px;
  }

  .ctx-header {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 8px;
    font-size: 0.8rem;
    color: var(--text-secondary);
    border-bottom: 1px solid var(--bg-overlay);
    margin-bottom: 4px;
  }

  .ctx-back {
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-secondary);
    border-radius: var(--radius-sm);
  }

  .ctx-back:hover { color: var(--text-primary); }
  .ctx-back svg { width: 14px; height: 14px; }

  .ctx-empty {
    display: block;
    padding: 8px 10px;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .toast {
    position: fixed;
    bottom: 100px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--bg-elevated);
    border: 1px solid var(--bg-overlay);
    color: var(--text-primary);
    padding: 8px 16px;
    border-radius: var(--radius);
    font-size: 0.85rem;
    z-index: 250;
    box-shadow: 0 4px 16px rgba(0, 0, 0, 0.4);
    animation: toastSlide 300ms var(--ease-spring);
  }
</style>
