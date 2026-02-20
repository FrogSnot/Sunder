<script lang="ts">
  import { onMount } from "svelte";
  import {
    listPlaylists,
    createPlaylist,
    deletePlaylist,
    getPlaylistTracks,
    removeFromPlaylist,
    reorderPlaylistTracks,
    renamePlaylist,
    playTrack,
  } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { nav } from "../state/nav.svelte";
  import WormText from "./WormText.svelte";
  import type { Playlist, Track } from "../types";

  let playlists = $state<Playlist[]>([]);
  let detailTracks = $state<Track[]>([]);
  let newName = $state("");
  let creating = $state(false);

  let viewing = $derived(nav.activeTab === "playlist-detail" && nav.activePlaylistId !== null);

  onMount(() => { refreshPlaylists(); });

  $effect(() => {
    if (nav.activeTab === "playlists") refreshPlaylists();
  });

  async function refreshPlaylists() {
    try {
      playlists = await listPlaylists();
    } catch (e) {
      console.error("list playlists:", e);
    }
  }

  async function handleCreate() {
    const name = newName.trim();
    if (!name) return;
    creating = true;
    try {
      await createPlaylist(name);
      newName = "";
      await refreshPlaylists();
    } catch (e) {
      console.error("create playlist:", e);
    } finally {
      creating = false;
    }
  }

  async function handleDelete(id: number) {
    try {
      await deletePlaylist(id);
      if (nav.activePlaylistId === id) {
        nav.activeTab = "playlists";
        nav.activePlaylistId = null;
      }
      await refreshPlaylists();
    } catch (e) {
      console.error("delete playlist:", e);
    }
  }

  async function openPlaylist(p: Playlist) {
    nav.activeTab = "playlist-detail";
    nav.activePlaylistId = p.id;
    nav.activePlaylistName = p.name;
    try {
      detailTracks = await getPlaylistTracks(p.id);
    } catch (e) {
      console.error("get tracks:", e);
    }
  }

  function goBack() {
    nav.activeTab = "playlists";
    nav.activePlaylistId = null;
  }

  async function handleRemove(trackId: string) {
    if (nav.activePlaylistId === null) return;
    try {
      await removeFromPlaylist(nav.activePlaylistId, trackId);
      detailTracks = detailTracks.filter((t) => t.id !== trackId);
    } catch (e) {
      console.error("remove track:", e);
    }
  }

  async function handlePlay(track: Track) {
    try {
      await playTrack(track);
    } catch (e) {
      console.error("play:", e);
    }
  }

  function formatDuration(secs: number): string {
    if (!secs) return "--:--";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  function isActive(track: Track): boolean {
    return player.currentTrack?.id === track.id;
  }

  async function handlePlayAll() {
    if (detailTracks.length === 0) return;
    player.clearQueue();
    for (const t of detailTracks) player.addToQueue(t);
    const first = player.playFromQueue(0);
    if (first) await playTrack(first);
  }

  async function handleQuickPlay(p: Playlist) {
    try {
      const tracks = await getPlaylistTracks(p.id);
      if (tracks.length === 0) return;
      player.clearQueue();
      for (const t of tracks) player.addToQueue(t);
      const first = player.playFromQueue(0);
      if (first) await playTrack(first);
    } catch (e) {
      console.error("quick play:", e);
    }
  }

  let renamingId = $state<number | null>(null);
  let renameValue = $state("");

  function startRename(p: Playlist) {
    renamingId = p.id;
    renameValue = p.name;
  }

  async function commitRename(id: number) {
    const name = renameValue.trim();
    if (name) {
      try {
        await renamePlaylist(id, name);
        await refreshPlaylists();
        if (nav.activePlaylistId === id) nav.activePlaylistName = name;
      } catch (e) {
        console.error("rename:", e);
      }
    }
    renamingId = null;
  }

  function handleRenameKeydown(e: KeyboardEvent, id: number) {
    e.stopPropagation();
    if (e.key === "Enter") { e.preventDefault(); commitRename(id); }
    if (e.key === "Escape") renamingId = null;
  }

  function handleRenameKeyup(e: KeyboardEvent) {
    e.stopPropagation();
    e.preventDefault();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") handleCreate();
  }

  let dragFrom = $state(-1);
  let dragOverIdx = $state(-1);
  let dragging = $state(false);

  function onPointerDown(e: PointerEvent, i: number) {
    e.preventDefault();
    dragFrom = i;
    dragOverIdx = i;
    dragging = true;
    const handle = e.currentTarget as HTMLElement;
    handle.setPointerCapture(e.pointerId);
  }

  function onPointerMove(e: PointerEvent) {
    if (!dragging) return;
    const el = document.elementFromPoint(e.clientX, e.clientY);
    if (!el) return;
    const row = el.closest('.track-row') as HTMLElement | null;
    if (!row || !row.dataset.idx) return;
    const idx = parseInt(row.dataset.idx, 10);
    if (!isNaN(idx)) dragOverIdx = idx;
  }

  async function onPointerUp() {
    if (!dragging) return;
    const from = dragFrom;
    const to = dragOverIdx;
    dragFrom = -1;
    dragOverIdx = -1;
    dragging = false;
    if (from >= 0 && to >= 0 && from !== to) {
      const moved = detailTracks.splice(from, 1)[0];
      detailTracks.splice(to, 0, moved);
      detailTracks = detailTracks;
      if (nav.activePlaylistId !== null) {
        try {
          await reorderPlaylistTracks(nav.activePlaylistId, detailTracks.map(t => t.id));
        } catch (err) {
          console.error("reorder:", err);
        }
      }
    }
  }
</script>

{#if viewing}
  <div class="detail">
    <button class="back-btn" onclick={goBack}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="15 18 9 12 15 6" />
      </svg>
      Back
    </button>
    <div class="detail-header">
      <h2 class="detail-title">{nav.activePlaylistName}</h2>
      {#if detailTracks.length > 0}
        <button class="play-all-btn" onclick={handlePlayAll} aria-label="Play all">
          <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
          Play All
        </button>
      {/if}
    </div>

    {#if detailTracks.length === 0}
      <p class="empty-sub">No tracks yet. Add tracks from search results.</p>
    {:else}
      <div class="track-list">
        {#each detailTracks as track, i (track.id)}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="track-row"
            class:active={isActive(track)}
            class:dragging={dragging && dragFrom === i}
            class:drag-over={dragging && dragOverIdx === i && dragFrom !== i}
            data-idx={i}
          >
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <span
              class="drag-handle"
              onpointerdown={(e) => onPointerDown(e, i)}
              onpointermove={onPointerMove}
              onpointerup={onPointerUp}
            >
              <svg viewBox="0 0 16 16" fill="currentColor"><circle cx="5" cy="3" r="1.5"/><circle cx="11" cy="3" r="1.5"/><circle cx="5" cy="8" r="1.5"/><circle cx="11" cy="8" r="1.5"/><circle cx="5" cy="13" r="1.5"/><circle cx="11" cy="13" r="1.5"/></svg>
            </span>
            <span class="track-num">{i + 1}</span>
            <button class="track-play" onclick={() => handlePlay(track)}>
              <img class="thumb" src={track.thumbnail || ""} alt="" loading="lazy" />
              <div class="track-info">
                <span class="track-title">{track.title}</span>
                <span class="track-artist">{track.artist}</span>
              </div>
              <span class="track-duration">{formatDuration(track.duration_secs)}</span>
            </button>
            <button class="remove-btn" onclick={() => handleRemove(track.id)} aria-label="Remove">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="18" y1="6" x2="6" y2="18" />
                <line x1="6" y1="6" x2="18" y2="18" />
              </svg>
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{:else}
  <div class="playlists">
    <div class="create-row">
      <input
        type="text"
        placeholder="New playlist name..."
        bind:value={newName}
        onkeydown={handleKeydown}
      />
      <button class="create-btn" onclick={handleCreate} disabled={creating || !newName.trim()}>
        {creating ? "..." : "+ Create"}
      </button>
    </div>

    {#if playlists.length === 0}
      <div class="empty-state">
        <p class="empty-title"><WormText text="No playlists yet" /></p>
        <p class="empty-sub">Create one above to get started</p>
      </div>
    {:else}
      <div class="list">
        {#each playlists as p (p.id)}
          <div class="playlist-row">
            <button class="playlist-btn" onclick={(e) => { if (renamingId === p.id) e.preventDefault(); else openPlaylist(p); }}>
              <div class="playlist-icon">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M9 18V5l12-2v13" />
                  <circle cx="6" cy="18" r="3" />
                  <circle cx="18" cy="16" r="3" />
                </svg>
              </div>
              <div class="playlist-info">
                {#if renamingId === p.id}
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    class="rename-input"
                    type="text"
                    bind:value={renameValue}
                    autofocus
                    onkeydown={(e) => handleRenameKeydown(e, p.id)}
                    onkeyup={handleRenameKeyup}
                    onblur={() => commitRename(p.id)}
                    onclick={(e) => e.stopPropagation()}
                  />
                {:else}
                  <span class="playlist-name">{p.name}</span>
                {/if}
                <span class="playlist-count">{p.track_count} track{p.track_count === 1 ? "" : "s"}</span>
              </div>
            </button>
            <button class="row-action-btn play" onclick={() => handleQuickPlay(p)} aria-label="Play all tracks">
              <svg viewBox="0 0 24 24" fill="currentColor"><polygon points="5 3 19 12 5 21 5 3"/></svg>
            </button>
            <button class="row-action-btn rename" onclick={() => startRename(p)} aria-label="Rename playlist">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <path d="M11 4H4a2 2 0 00-2 2v14a2 2 0 002 2h14a2 2 0 002-2v-7" />
                <path d="M18.5 2.5a2.121 2.121 0 013 3L12 15l-4 1 1-4 9.5-9.5z" />
              </svg>
            </button>
            <button class="row-action-btn delete" onclick={() => handleDelete(p.id)} aria-label="Delete playlist">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <polyline points="3 6 5 6 21 6" />
                <path d="M19 6v14a2 2 0 01-2 2H7a2 2 0 01-2-2V6m3 0V4a2 2 0 012-2h4a2 2 0 012 2v2" />
              </svg>
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  .create-row {
    display: flex;
    gap: 8px;
    margin-bottom: 20px;
  }

  .create-row input {
    flex: 1;
    background: var(--bg-elevated);
    border: 1px solid var(--bg-overlay);
    border-radius: var(--radius);
    padding: 8px 14px;
    font-size: 0.9rem;
    outline: none;
    transition: border-color var(--transition);
  }

  .create-row input:focus {
    border-color: var(--accent-dim);
  }

  .create-btn {
    padding: 8px 16px;
    background: var(--accent);
    color: #121212;
    border-radius: var(--radius);
    font-weight: 600;
    font-size: 0.85rem;
    transition: background 200ms ease, transform 150ms var(--ease-spring);
  }

  .create-btn:hover:not(:disabled) {
    background: var(--accent-light);
    transform: scale(1.03);
  }

  .create-btn:active:not(:disabled) {
    transform: scale(0.97);
  }

  .create-btn:disabled { opacity: 0.5; cursor: default; }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 40vh;
    color: var(--text-muted);
    animation: viewEnter 500ms var(--ease-out-expo);
  }

  .empty-title {
    font-size: 1.1rem;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .empty-sub { font-size: 0.85rem; color: var(--text-muted); }

  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: viewEnter 350ms var(--ease-out-expo);
  }

  .playlist-row {
    display: flex;
    align-items: center;
  }

  .playlist-btn {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border-radius: var(--radius);
    transition: background 200ms ease, transform 200ms ease;
    text-align: left;
  }

  .playlist-btn:hover {
    background: var(--bg-elevated);
    transform: translateX(4px);
  }

  .playlist-btn:active {
    transform: scale(0.99);
  }

  .playlist-icon {
    width: 40px;
    height: 40px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--bg-overlay);
    border-radius: var(--radius-sm);
    color: var(--accent);
    flex-shrink: 0;
    transition: background 200ms ease, transform 200ms ease;
  }

  .playlist-btn:hover .playlist-icon {
    background: var(--accent-dim);
    transform: scale(1.05);
  }

  .playlist-icon svg { width: 20px; height: 20px; }

  .playlist-info {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .playlist-name {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text-primary);
  }

  .playlist-count {
    font-size: 0.8rem;
    color: var(--text-secondary);
  }

  .row-action-btn {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    transition: color 200ms ease, background 200ms ease, transform 150ms ease;
    flex-shrink: 0;
  }

  .row-action-btn:hover { background: var(--bg-elevated); transform: scale(1.1); }
  .row-action-btn.play:hover { color: var(--accent); }
  .row-action-btn.rename:hover { color: var(--accent); }
  .row-action-btn.delete:hover { color: var(--error); }
  .row-action-btn svg { width: 16px; height: 16px; }

  .rename-input {
    background: var(--bg-overlay);
    border: 1px solid var(--accent-dim);
    border-radius: var(--radius-sm);
    padding: 2px 6px;
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text-primary);
    outline: none;
    width: 100%;
  }

  .detail-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 16px;
  }

  .detail-title {
    font-size: 1.3rem;
    font-weight: 700;
    color: var(--text-primary);
  }

  .play-all-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    background: var(--accent);
    color: #121212;
    border-radius: var(--radius);
    font-weight: 600;
    font-size: 0.85rem;
    transition: background 200ms ease, transform 150ms var(--ease-spring);
  }

  .play-all-btn:hover {
    background: var(--accent-light);
    transform: scale(1.05);
  }

  .play-all-btn:active {
    transform: scale(0.95);
  }

  .play-all-btn svg {
    width: 14px;
    height: 14px;
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-bottom: 12px;
    padding: 4px 0;
    transition: color 200ms ease, transform 200ms ease;
  }

  .back-btn:hover {
    color: var(--text-primary);
    transform: translateX(-4px);
  }
  .back-btn svg { width: 16px; height: 16px; }

  .track-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    animation: viewEnter 350ms var(--ease-out-expo);
  }

  .track-row {
    display: flex;
    align-items: center;
    gap: 4px;
    opacity: 1;
    background: var(--bg-base);
    border-radius: var(--radius);
    transition: background 200ms ease, opacity 150ms ease;
  }

  .track-row.dragging { opacity: 0.3; }
  .track-row.drag-over { border-top: 2px solid var(--accent); margin-top: -2px; }

  .drag-handle {
    width: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    cursor: grab;
    flex-shrink: 0;
    opacity: 0.4;
    transition: opacity var(--transition), color var(--transition);
    touch-action: none;
  }

  .drag-handle:active { cursor: grabbing; }
  .track-row:hover .drag-handle { opacity: 1; }
  .drag-handle svg { width: 12px; height: 12px; }

  .track-row.active {
    background: var(--bg-elevated);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius);
    position: relative;
  }

  .track-row.active::before {
    content: '';
    position: absolute;
    inset: 0;
    border-radius: var(--radius);
    pointer-events: none;
  }

  .track-num {
    width: 28px;
    text-align: center;
    font-size: 0.8rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .track-play {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 10px;
    border-radius: var(--radius);
    transition: background var(--transition);
    text-align: left;
  }

  .track-play:hover { background: var(--bg-elevated); }

  .thumb {
    width: 40px;
    height: 40px;
    border-radius: var(--radius-sm);
    object-fit: cover;
    background: var(--bg-overlay);
    flex-shrink: 0;
  }

  .track-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .track-title {
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track-artist {
    font-size: 0.8rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .track-duration {
    font-size: 0.8rem;
    color: var(--text-muted);
    flex-shrink: 0;
    font-variant-numeric: tabular-nums;
  }

  .remove-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    transition: color 200ms ease, transform 150ms ease;
    flex-shrink: 0;
  }

  .remove-btn:hover {
    color: var(--error);
    transform: scale(1.15);
  }
  .remove-btn svg { width: 14px; height: 14px; }
</style>
