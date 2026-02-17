<script lang="ts">
  import { playTrack, listPlaylists, addToPlaylist } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { searchState } from "../state/search.svelte";
  import type { Track, Playlist } from "../types";

  let tracks = $derived(searchState.results);
  let menuTrack = $state<string | null>(null);
  let playlists = $state<Playlist[]>([]);

  function formatDuration(secs: number): string {
    if (!secs) return "--:--";
    const m = Math.floor(secs / 60);
    const s = Math.floor(secs % 60);
    return `${m}:${s.toString().padStart(2, "0")}`;
  }

  async function handlePlay(track: Track) {
    try {
      await playTrack(track);
    } catch (e) {
      console.error("play failed:", e);
    }
  }

  function isActive(track: Track): boolean {
    return player.currentTrack?.id === track.id;
  }

  async function openMenu(e: MouseEvent, trackId: string) {
    e.stopPropagation();
    if (menuTrack === trackId) { menuTrack = null; return; }
    try { playlists = await listPlaylists(); } catch (_) {}
    menuTrack = trackId;
  }

  async function addTrack(playlistId: number) {
    if (!menuTrack) return;
    try { await addToPlaylist(playlistId, menuTrack); } catch (e) { console.error("add:", e); }
    menuTrack = null;
  }

  function closeMenu() { menuTrack = null; }
</script>

<svelte:window onclick={closeMenu} />

{#if tracks.length === 0}
  <div class="empty-state">
    <p class="empty-title">Search for something</p>
    <p class="empty-sub">Results will appear here</p>
  </div>
{:else}
  <div class="track-list">
    {#each tracks as track (track.id)}
      <div class="track-row" class:active={isActive(track)}>
        <button class="track-main" onclick={() => handlePlay(track)}>
          <img
            class="thumb"
            src={track.thumbnail || ""}
            alt=""
            loading="lazy"
          />
          <div class="track-info">
            <span class="track-title">{track.title}</span>
            <span class="track-artist">{track.artist}</span>
          </div>
          <span class="track-duration">{formatDuration(track.duration_secs)}</span>
        </button>
        <div class="add-wrap">
          <button class="add-btn" onclick={(e) => openMenu(e, track.id)} aria-label="Add to playlist">
            <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/></svg>
          </button>
          {#if menuTrack === track.id}
            <!-- svelte-ignore a11y_no_static_element_interactions a11y_click_events_have_key_events -->
            <div class="playlist-menu" onclick={(e) => e.stopPropagation()}>
              {#if playlists.length === 0}
                <span class="menu-empty">No playlists yet</span>
              {:else}
                {#each playlists as p (p.id)}
                  <button class="menu-item" onclick={() => addTrack(p.id)}>{p.name}</button>
                {/each}
              {/if}
            </div>
          {/if}
        </div>
      </div>
    {/each}
  </div>
{/if}

<style>
  .empty-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 60vh;
    color: var(--text-muted);
  }

  .empty-title {
    font-size: 1.2rem;
    color: var(--text-secondary);
    margin-bottom: 4px;
  }

  .empty-sub {
    font-size: 0.85rem;
  }

  .track-list {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .track-row {
    display: flex;
    align-items: center;
    gap: 4px;
    border-radius: var(--radius);
    transition: background var(--transition);
  }

  .track-row:hover {
    background: var(--bg-elevated);
  }

  .track-row.active {
    background: var(--bg-elevated);
    border-left: 3px solid var(--accent);
  }

  .track-main {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    text-align: left;
    min-width: 0;
  }

  .add-wrap {
    position: relative;
    flex-shrink: 0;
    padding-right: 8px;
  }

  .add-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    transition: color var(--transition), background var(--transition);
    opacity: 0;
  }

  .track-row:hover .add-btn { opacity: 1; }
  .add-btn:hover { color: var(--accent); background: var(--bg-overlay); }
  .add-btn svg { width: 16px; height: 16px; }

  .playlist-menu {
    position: absolute;
    right: 0;
    top: 100%;
    width: 180px;
    background: var(--bg-elevated);
    border: 1px solid var(--bg-overlay);
    border-radius: var(--radius);
    padding: 4px;
    z-index: 20;
    box-shadow: 0 8px 24px rgba(0,0,0,0.4);
  }

  .menu-item {
    display: block;
    width: 100%;
    text-align: left;
    padding: 8px 10px;
    font-size: 0.85rem;
    color: var(--text-primary);
    border-radius: var(--radius-sm);
    transition: background var(--transition);
  }

  .menu-item:hover { background: var(--bg-overlay); }

  .menu-empty {
    display: block;
    padding: 8px 10px;
    font-size: 0.8rem;
    color: var(--text-muted);
  }

  .thumb {
    width: 48px;
    height: 48px;
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
</style>
