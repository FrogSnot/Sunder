import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Track, SearchResult, PlaybackProgress } from "../types";
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
  await invoke("play_track", { trackId: track.id });
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

export function initProgressListener(): () => void {
  let unlisten: (() => void) | undefined;

  listen<PlaybackProgress>("playback-progress", (event) => {
    player.updateFromProgress(event.payload);
  }).then((fn) => {
    unlisten = fn;
  });

  return () => unlisten?.();
}
