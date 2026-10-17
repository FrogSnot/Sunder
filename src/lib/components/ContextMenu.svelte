<script lang="ts">
  import { listPlaylists, addToPlaylist, removeFromPlaylist, playlistsContainingTrack, downloadTrack, deleteDownload } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { nav } from "../state/nav.svelte";
  import { toastState } from "../state/toast.svelte";
  import { downloads } from "../state/downloads.svelte";
  import { suppressTooltips, unsuppressTooltips } from "../util/tooltip.svelte";
  import type { Playlist, Track } from "../types";

  let { onRemoveFromPlaylist = undefined }: { onRemoveFromPlaylist?: (trackId: string) => void } = $props();

  let visible = $state(false);
  let x = $state(0);
  let y = $state(0);
  let track = $state<Track | null>(null);
  let playlists = $state<Playlist[]>([]);
  let showPlaylists = $state(false);
  let trackPlaylistIds = $state<Set<number>>(new Set());

  let inQueue = $derived(track !== null && player.queue.some((t) => t.id === track!.id));
  let inPlaylist = $derived(nav.activePlaylistId !== null);
  let isDownloaded = $derived(track !== null && downloads.isDownloaded(track.id));
  let isDownloading = $derived(track !== null && downloads.isActive(track.id));

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
    suppressTooltips("ctxmenu");
  }

  function close() {
    visible = false;
    showPlaylists = false;
    unsuppressTooltips("ctxmenu");
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
    } catch (e) {
      showToast(`Failed to add: ${e}`, "error");
    }
    close();
  }

  async function handleDownload() {
    if (!track || isDownloading) return;
    const t = track;
    try {
      if (downloads.isDownloaded(t.id)) {
        await deleteDownload(t.id);
      } else {
        downloads.register(t);
        await downloadTrack(t.id);
      }
    } catch {
      // download failures surface in the activity panel
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
    const removedId = track.id;
    try {
      await removeFromPlaylist(nav.activePlaylistId, removedId);
      onRemoveFromPlaylist?.(removedId);
      showToast("Removed from playlist");
    } catch (e) {
      showToast(`Failed to remove: ${e}`, "error");
    }
    close();
  }

  function showToast(msg: string, type: "info" | "error" = "info") {
    toastState.add(msg, type, 2000);
  }

  // --- keyboard navigation (WAI-ARIA menu pattern) ---------------------------
  // The menu container carries role="menu"; arrow keys move focus between the
  // item buttons, Home/End jump to first/last, Escape backs out of the
  // playlist submenu or closes the menu. Disabled items are skipped (they
  // cannot take focus).

  function focusableItems(): HTMLButtonElement[] {
    const menu = document.querySelector(".ctx-menu");
    if (!menu) return [];
    return [...menu.querySelectorAll<HTMLButtonElement>("button.ctx-item, button.ctx-back")].filter(
      (b) => !b.disabled,
    );
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp" && e.key !== "Home" && e.key !== "End") {
      return;
    }
    const items = focusableItems();
    if (items.length === 0) return;
    e.preventDefault();
    const active = document.activeElement;
    const current = active instanceof HTMLElement ? items.indexOf(active as HTMLButtonElement) : -1;
    let next: number;
    if (e.key === "Home") next = 0;
    else if (e.key === "End") next = items.length - 1;
    else if (e.key === "ArrowDown") next = current + 1 >= items.length ? 0 : current + 1;
    else next = current - 1 < 0 ? items.length - 1 : current - 1;
    items[next]?.focus();
  }

  // Escape is handled at window level (mirroring the window click-to-close):
  // it must work no matter where focus is, including after a mouse click that
  // some engines do not turn into button focus. Backs out of the playlist
  // submenu first, then closes.
  function handleWindowKeydown(e: KeyboardEvent) {
    if (!visible || e.key !== "Escape") return;
    if (showPlaylists) {
      showPlaylists = false;
      return;
    }
    close();
  }

  // Focus the first item whenever the menu renders or the view swaps, so the
  // menu pattern works without a prior pointer interaction.
  $effect(() => {
    if (!visible) return;
    // Read so a submenu swap re-runs this and re-anchors focus in the new view.
    const _view = showPlaylists;
    // One frame later: the {#if visible} block must be in the DOM first.
    requestAnimationFrame(() => {
      if (!visible) return;
      const menu = document.querySelector(".ctx-menu");
      const target =
        menu?.querySelector<HTMLElement>(".ctx-back") ??
        menu?.querySelector<HTMLElement>(".ctx-item");
      target?.focus();
    });
  });
</script>

<svelte:window onclick={close} onkeydown={handleWindowKeydown} oncontextmenu={() => { if (visible) close(); }} />

{#if visible}
  <div
    class="ctx-menu"
    style="left: {x}px; top: {y}px"
    role="menu"
    aria-orientation="vertical"
    tabindex="-1"
    onkeydown={handleKeydown}
    onclick={(e) => e.stopPropagation()}
  >
    {#if !showPlaylists}
      <button class="ctx-item" role="menuitem" onclick={handlePlayNext}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="5 3 19 12 5 21 5 3"/><line x1="22" y1="3" x2="22" y2="21"/></svg>
        Play next
      </button>
      <button class="ctx-item" role="menuitem" onclick={handleAddToQueue}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="8" y1="6" x2="21" y2="6"/><line x1="8" y1="12" x2="21" y2="12"/><line x1="8" y1="18" x2="21" y2="18"/><line x1="3" y1="6" x2="3.01" y2="6"/><line x1="3" y1="12" x2="3.01" y2="12"/><line x1="3" y1="18" x2="3.01" y2="18"/></svg>
        Add to queue
      </button>
      <div class="ctx-divider"></div>
      <button class="ctx-item" role="menuitem" aria-expanded={showPlaylists ? "true" : "false"} aria-haspopup="true" onclick={expandPlaylists}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
        Add to playlist
      </button>
      <button class="ctx-item" role="menuitem" class:ctx-muted={isDownloading} onclick={handleDownload} disabled={isDownloading}>
        {#if isDownloaded}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6v14a2 2 0 0 1-2 2H7a2 2 0 0 1-2-2V6m3 0V4a2 2 0 0 1 2-2h4a2 2 0 0 1 2 2v2"/></svg>
          Remove download
        {:else if isDownloading}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><polyline points="12 6 12 12 16 14"/></svg>
          Downloading...
        {:else}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>
          Download
        {/if}
      </button>
      {#if inQueue || inPlaylist}
        <div class="ctx-divider"></div>
        {#if inQueue}
          <button class="ctx-item ctx-danger" role="menuitem" onclick={handleRemoveFromQueue}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="8" y1="12" x2="16" y2="12"/></svg>
            Remove from queue
          </button>
        {/if}
        {#if inPlaylist}
          <button class="ctx-item ctx-danger" role="menuitem" onclick={handleRemoveFromPlaylist}>
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="8" y1="12" x2="16" y2="12"/></svg>
            Remove from playlist
          </button>
        {/if}
      {/if}
    {:else}
      <div class="ctx-header">
        <button class="ctx-back" role="menuitem" aria-label="Back to menu items" onclick={() => { showPlaylists = false; }}>
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="15 18 9 12 15 6"/></svg>
        </button>
        <span>Choose playlist</span>
      </div>
      {#if playlists.length === 0}
        <span class="ctx-empty">No playlists yet</span>
      {:else}
        {#each playlists as p (p.id)}
          {@const alreadyIn = trackPlaylistIds.has(p.id)}
          <button class="ctx-item" role="menuitem" class:ctx-muted={alreadyIn} onclick={() => { if (!alreadyIn) handleAdd(p.id); }} disabled={alreadyIn}>
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
</style>
