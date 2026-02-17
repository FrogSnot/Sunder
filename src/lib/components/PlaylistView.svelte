<script lang="ts">
  import { onMount } from "svelte";
  import {
    listPlaylists,
    createPlaylist,
    deletePlaylist,
    getPlaylistTracks,
    removeFromPlaylist,
    playTrack,
  } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { nav } from "../state/nav.svelte";
  import type { Playlist, Track } from "../types";

  let playlists = $state<Playlist[]>([]);
  let detailTracks = $state<Track[]>([]);
  let newName = $state("");
  let creating = $state(false);

  let viewing = $derived(nav.activeTab === "playlist-detail" && nav.activePlaylistId !== null);

  onMount(() => { refreshPlaylists(); });

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

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Enter") handleCreate();
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
    <h2 class="detail-title">{nav.activePlaylistName}</h2>

    {#if detailTracks.length === 0}
      <p class="empty-sub">No tracks yet. Add tracks from search results.</p>
    {:else}
      <div class="track-list">
        {#each detailTracks as track, i (track.id)}
          <div class="track-row" class:active={isActive(track)}>
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
        <p class="empty-title">No playlists yet</p>
        <p class="empty-sub">Create one above to get started</p>
      </div>
    {:else}
      <div class="list">
        {#each playlists as p (p.id)}
          <div class="playlist-row">
            <button class="playlist-btn" onclick={() => openPlaylist(p)}>
              <div class="playlist-icon">
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M9 18V5l12-2v13" />
                  <circle cx="6" cy="18" r="3" />
                  <circle cx="18" cy="16" r="3" />
                </svg>
              </div>
              <div class="playlist-info">
                <span class="playlist-name">{p.name}</span>
                <span class="playlist-count">{p.track_count} track{p.track_count === 1 ? "" : "s"}</span>
              </div>
            </button>
            <button class="delete-btn" onclick={() => handleDelete(p.id)} aria-label="Delete playlist">
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
    transition: background var(--transition);
  }

  .create-btn:hover:not(:disabled) { background: var(--accent-light); }
  .create-btn:disabled { opacity: 0.5; cursor: default; }

  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 40vh;
    color: var(--text-muted);
  }

  .empty-title {
    font-size: 1.1rem;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .empty-sub { font-size: 0.85rem; color: var(--text-muted); }

  .list { display: flex; flex-direction: column; gap: 2px; }

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
    transition: background var(--transition);
    text-align: left;
  }

  .playlist-btn:hover { background: var(--bg-elevated); }

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

  .delete-btn {
    width: 32px;
    height: 32px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    transition: color var(--transition), background var(--transition);
    flex-shrink: 0;
  }

  .delete-btn:hover {
    color: var(--error);
    background: var(--bg-elevated);
  }

  .delete-btn svg { width: 16px; height: 16px; }

  /* Detail view */
  .detail-title {
    font-size: 1.3rem;
    font-weight: 700;
    color: var(--text-primary);
    margin-bottom: 16px;
  }

  .back-btn {
    display: flex;
    align-items: center;
    gap: 4px;
    font-size: 0.85rem;
    color: var(--text-secondary);
    margin-bottom: 12px;
    padding: 4px 0;
    transition: color var(--transition);
  }

  .back-btn:hover { color: var(--text-primary); }
  .back-btn svg { width: 16px; height: 16px; }

  .track-list { display: flex; flex-direction: column; gap: 2px; }

  .track-row {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .track-row.active {
    background: var(--bg-elevated);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius);
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
    transition: color var(--transition);
    flex-shrink: 0;
  }

  .remove-btn:hover { color: var(--error); }
  .remove-btn svg { width: 14px; height: 14px; }
</style>
