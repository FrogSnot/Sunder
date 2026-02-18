import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Track, SearchResult, PlaybackProgress, Playlist, ExploreData } from "../types";
import { player } from "../state/player.svelte";

export async function search(query: string): Promise<SearchResult> {
  return invoke<SearchResult>("search", { query });
}

export async function searchLocal(query: string): Promise<Track[]> {
  return invoke<Track[]>("search_local", { query });
}

export async function playTrack(track: Track): Promise<void> {
  player.currentTrack = track;
  player.isBuffering = true;
  player.downloadPercent = 0;
  player.downloadStage = "preparing";
  const idx = player.queue.findIndex((t) => t.id === track.id);
  if (idx !== -1) {
    player.queueIndex = idx;
  }
  await invoke("play_track", { trackId: track.id });
}

async function playNextInQueue(): Promise<void> {
  const next = player.nextTrack();
  if (next) {
    await playTrack(next);
  }
}

export async function pause(): Promise<void> {
  await invoke("pause");
}

export async function resume(): Promise<void> {
  await invoke("resume");
}

export async function stop(): Promise<void> {
  await invoke("stop");
  player.currentTrack = null;
}

export async function setVolume(volume: number): Promise<void> {
  player.volume = volume;
  await invoke("set_volume", { volume });
}

export async function seek(positionSecs: number): Promise<void> {
  await invoke("seek", { positionSecs });
}

export async function prefetchTrack(trackId: string): Promise<void> {
  await invoke("prefetch_track", { trackId });
}

export async function createPlaylist(name: string): Promise<Playlist> {
  return invoke<Playlist>("create_playlist", { name });
}

export async function listPlaylists(): Promise<Playlist[]> {
  return invoke<Playlist[]>("list_playlists");
}

export async function deletePlaylist(playlistId: number): Promise<void> {
  await invoke("delete_playlist", { playlistId });
}

export async function renamePlaylist(playlistId: number, name: string): Promise<void> {
  await invoke("rename_playlist", { playlistId, name });
}

export async function addToPlaylist(playlistId: number, trackId: string): Promise<void> {
  await invoke("add_to_playlist", { playlistId, trackId });
}

export async function removeFromPlaylist(playlistId: number, trackId: string): Promise<void> {
  await invoke("remove_from_playlist", { playlistId, trackId });
}

export async function getPlaylistTracks(playlistId: number): Promise<Track[]> {
  return invoke<Track[]>("get_playlist_tracks", { playlistId });
}

export async function reorderPlaylistTracks(playlistId: number, trackIds: string[]): Promise<void> {
  await invoke("reorder_playlist_tracks", { playlistId, trackIds });
}

export async function getRecentlyPlayed(): Promise<Track[]> {
  return invoke<Track[]>("get_recently_played");
}

export async function getExplore(): Promise<ExploreData> {
  return invoke<ExploreData>("get_explore");
}

export function initProgressListener(): () => void {
  let unlistenProgress: (() => void) | undefined;
  let unlistenDownload: (() => void) | undefined;
  let unlistenFinished: (() => void) | undefined;

  listen<PlaybackProgress>("playback-progress", (event) => {
    player.updateFromProgress(event.payload);
  }).then((fn) => { unlistenProgress = fn; });

  listen<{ percent: number; stage: string }>("download-progress", (event) => {
    player.downloadPercent = event.payload.percent;
    player.downloadStage = event.payload.stage;
  }).then((fn) => { unlistenDownload = fn; });

  listen("track-finished", () => {
    playNextInQueue();
  }).then((fn) => { unlistenFinished = fn; });

  return () => {
    unlistenProgress?.();
    unlistenDownload?.();
    unlistenFinished?.();
  };
}
