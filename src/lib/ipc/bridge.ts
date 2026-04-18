import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getVersion } from "@tauri-apps/api/app";
import { save, open } from "@tauri-apps/plugin-dialog";
import type { Track, SearchResult, PlaybackProgress, Playlist, ExploreData, EqSettings } from "../types";
import { player } from "../state/player.svelte";
import { config } from "../state/config.svelte";
import { lyricsState, parseLrc } from "../state/lyrics.svelte";

export async function search(query: string): Promise<SearchResult> {
  return invoke<SearchResult>("search", { query });
}

export async function searchLocal(query: string): Promise<Track[]> {
  return invoke<Track[]>("search_local", { query });
}

export async function importYtPlaylist(url: string, playlistName: string): Promise<Playlist> {
  return invoke<Playlist>("import_yt_playlist", { url, playlistName });
}

export async function refreshYtPlaylist(playlistId: number): Promise<number> {
  return invoke<number>("refresh_yt_playlist", { playlistId });
}

export async function playTrack(track: Track): Promise<void> {
  player.currentTrack = track;
  player.isBuffering = true;
  player.downloadPercent = 0;
  player.downloadStage = "preparing";
  const idx = player.queue.findIndex((t) => t.id === track.id);
  if (idx !== -1) {
    player.queueIndex = idx;
  } else {
    const insertAt = player.queueIndex + 1;
    const updated = [...player.queue];
    updated.splice(insertAt, 0, track);
    player.queue = updated;
    player.queueIndex = insertAt;
  }
  player.prefetchAhead(player.queueIndex);
  await invoke("play_track", { trackId: track.id });
  // Lazy lyrics: only fetch if the lyrics panel is already open
  if (lyricsState.visible) {
    fetchLyrics(track.id, track.artist, track.title, track.duration_secs);
  }
}

let advancing = false;

export async function playNext(manual = false): Promise<void> {
  if (advancing) return;
  advancing = true;
  try {
    const next = player.nextTrack(manual);
    if (next) {
      await playTrack(next);
    }
  } finally {
    advancing = false;
  }
}

export async function playPrev(manual = false): Promise<void> {
  if (player.currentTime > 5.0) {
    await seek(0);
    player.currentTime = 0;
    return;
  }
  const prev = player.prevTrack(manual);
  if (prev) {
    await playTrack(prev);
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

export async function setSpeed(speed: number): Promise<void> {
  player.speed = speed;
  await invoke("set_speed", { speed });
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

export async function playlistsContainingTrack(trackId: string): Promise<number[]> {
  return invoke<number[]>("playlists_containing_track", { trackId });
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

export async function setEqGains(gains: number[]): Promise<void> {
  await invoke("set_eq_gains", { gains });
}

export async function setEqEnabled(enabled: boolean): Promise<void> {
  await invoke("set_eq_enabled", { enabled });
}

export async function getEqSettings(): Promise<EqSettings> {
  return invoke<EqSettings>("get_eq_settings");
}

export async function setRepeatMode(mode: "off" | "queue" | "track"): Promise<void> {
  await invoke("set_repeat_mode", { mode });
}

export async function exportPlaylist(playlistId: number, playlistName: string): Promise<boolean> {
  const path = await save({
    defaultPath: `${playlistName.replace(/[^a-zA-Z0-9_-]/g, "_")}.json`,
    filters: [{ name: "JSON", extensions: ["json"] }],
  });
  if (!path) return false;
  await invoke("export_playlist_json", { playlistId, path });
  return true;
}

export async function importPlaylistJson(): Promise<Playlist | null> {
  const path = await open({
    filters: [{ name: "JSON", extensions: ["json"] }],
    multiple: false,
    directory: false,
  });
  if (!path) return null;
  return invoke<Playlist>("import_playlist_json", { path });
}

export async function setDiscordRpc(enabled: boolean, track?: Track): Promise<void> {
  await invoke("set_discord_rpc", {
    enabled,
    title: track?.title ?? null,
    artist: track?.artist ?? null,
    thumbnail: track?.thumbnail ?? null,
  });
}

export async function getTracksByIds(trackIds: string[]): Promise<Track[]> {
  return invoke<Track[]>("get_tracks_by_ids", { trackIds });
}

export async function restoreQueue(): Promise<void> {
  const { saved_queue, saved_queue_index } = config.current;
  if (!saved_queue?.length) return;
  const tracks = await getTracksByIds(saved_queue);
  if (tracks.length === 0) return;
  player.queue = tracks;
  player.queueIndex = Math.min(Math.max(saved_queue_index ?? -1, -1), tracks.length - 1);
  if (player.queueIndex >= 0) {
    player.currentTrack = tracks[player.queueIndex];
  }
}

export interface UpdateInfo {
  available: boolean;
  version?: string;
  url?: string;
}

function isNewer(latest: string, current: string): boolean {
  const lp = latest.split(".").map((n) => parseInt(n, 10) || 0);
  const cp = current.split(".").map((n) => parseInt(n, 10) || 0);
  const len = Math.max(lp.length, cp.length);
  for (let i = 0; i < len; i++) {
    const a = lp[i] ?? 0;
    const b = cp[i] ?? 0;
    if (a !== b) return a > b;
  }
  return false;
}

export async function checkForUpdates(): Promise<UpdateInfo> {
  try {
    const current = await getVersion();
    const ctrl = new AbortController();
    const timer = setTimeout(() => ctrl.abort(), 8000);
    const res = await fetch(
      "https://api.github.com/repos/FrogSnot/Sunder/releases/latest",
      { headers: { Accept: "application/vnd.github+json" }, signal: ctrl.signal },
    );
    clearTimeout(timer);
    if (!res.ok) {
      console.warn("[updater] GitHub API", res.status);
      return { available: false };
    }
    const data = await res.json();
    const latest = String(data.tag_name ?? "").replace(/^v/, "");
    if (!latest) return { available: false };
    if (!isNewer(latest, current)) return { available: false };
    return {
      available: true,
      version: `v${latest}`,
      url: data.html_url ?? "https://github.com/FrogSnot/Sunder/releases/latest",
    };
  } catch (e) {
    console.warn("[updater] check failed", e);
    return { available: false };
  }
}

export async function openUrl(url: string): Promise<void> {
  await invoke("open_url", { url });
}

export function initProgressListener(): () => void {
  let unlistenProgress: (() => void) | undefined;
  let unlistenDownload: (() => void) | undefined;
  let unlistenFinished: (() => void) | undefined;
  let unlistenError: (() => void) | undefined;
  let unlistenNext: (() => void) | undefined;
  let unlistenPrev: (() => void) | undefined;
  let unlistenToggle: (() => void) | undefined;

  listen<PlaybackProgress>("playback-progress", (event) => {
    player.updateFromProgress(event.payload);
  }).then((fn) => { unlistenProgress = fn; });

  listen<{ percent: number; stage: string }>("download-progress", (event) => {
    player.downloadPercent = event.payload.percent;
    player.downloadStage = event.payload.stage;
  }).then((fn) => { unlistenDownload = fn; });

  listen("track-finished", () => {
    playNext().catch((e) => console.error("Failed to play next track after finish:", e));
  }).then((fn) => { unlistenFinished = fn; });

  listen<{ video_id: string; error: string }>("playback-error", (event) => {
    const failedId = event.payload.video_id;
    player.lastError = event.payload.error;
    player.consecutiveErrors++;
    player.isBuffering = false;
    player.failedTrack = player.currentTrack;
    player.downloadStage = "error";

    if (player.consecutiveErrors < 3 && player.hasNext) {
      setTimeout(() => {
        if (player.currentTrack?.id === failedId && !player.findingAlt) {
          playNext().catch((e) => console.error("Failed to play next track after error:", e));
        }
      }, 4000);
    }
  }).then((fn) => { unlistenError = fn; });

  listen("media-next", () => {
    playNext(true).catch((e) => console.error("Media key next failed:", e));
  }).then((fn) => { unlistenNext = fn; });

  listen("media-previous", () => {
    playPrev(true).catch((e) => console.error("Media key previous failed:", e));
  }).then((fn) => { unlistenPrev = fn; });

  listen("media-toggle", () => {
    if (player.isPlaying) {
      pause().catch((e) => console.error("Media key pause failed:", e));
    } else {
      resume().catch((e) => console.error("Media key resume failed:", e));
    }
  }).then((fn) => { unlistenToggle = fn; });

  return () => {
    unlistenProgress?.();
    unlistenDownload?.();
    unlistenFinished?.();
    unlistenError?.();
    unlistenNext?.();
    unlistenPrev?.();
    unlistenToggle?.();
  };
}

export async function fetchLyrics(trackId: string, artist: string, title: string, durationSecs?: number) {
  if (lyricsState.trackId === trackId && !lyricsState.error) return;
  lyricsState.reset();
  lyricsState.trackId = trackId;
  lyricsState.loading = true;

  try {
    const { cleanArtist, cleanTitle } = cleanForSearch(artist, title);

    if (await tryLrclib(cleanArtist, cleanTitle, durationSecs)) return;
    if (cleanTitle !== title && await tryLrclib(artist, title, durationSecs)) return;
    if (durationSecs && await tryLrclib(cleanArtist, cleanTitle)) return;
    if (await tryLrclibQuery(`${cleanArtist} ${cleanTitle}`)) return;
    if (await tryLrclibQuery(cleanTitle)) return;
    if (await tryLyricsOvh(cleanArtist, cleanTitle)) return;
    if (cleanTitle !== title && await tryLyricsOvh(artist, title)) return;

    try {
      const subs = await invoke<string>("get_subtitles", { videoId: trackId, lang: "en" });
      if (subs && subs.trim().length > 20) {
        lyricsState.content = subs;
        lyricsState.source = "YouTube";
        return;
      }
    } catch { /* no subtitles */ }

    lyricsState.error = "Lyrics not found.";
  } catch {
    lyricsState.error = "Failed to fetch lyrics.";
  } finally {
    lyricsState.loading = false;
  }
}

async function tryLrclib(artist: string, title: string, duration?: number): Promise<boolean> {
  try {
    let url = `https://lrclib.net/api/get?artist=${encodeURIComponent(artist)}&track_name=${encodeURIComponent(title)}`;
    if (duration) url += `&duration=${Math.round(duration)}`;
    const res = await fetch(url);
    if (!res.ok) return false;
    const data = await res.json();
    if (data.syncedLyrics) {
      lyricsState.syncedLines = parseLrc(data.syncedLyrics);
      lyricsState.synced = true;
      lyricsState.content = data.plainLyrics || "";
      lyricsState.source = "LRCLIB";
      return true;
    } else if (data.plainLyrics) {
      lyricsState.content = data.plainLyrics;
      lyricsState.synced = false;
      lyricsState.source = "LRCLIB";
      return true;
    }
  } catch { /* ignore */ }
  return false;
}

async function tryLrclibQuery(query: string): Promise<boolean> {
  try {
    const res = await fetch(`https://lrclib.net/api/search?q=${encodeURIComponent(query)}`);
    if (!res.ok) return false;
    const data = await res.json();
    if (data.length > 0) {
      const first = data[0];
      if (first.syncedLyrics) {
        lyricsState.syncedLines = parseLrc(first.syncedLyrics);
        lyricsState.synced = true;
        lyricsState.content = first.plainLyrics || "";
        lyricsState.source = "LRCLIB (Search)";
        return true;
      } else if (first.plainLyrics) {
        lyricsState.content = first.plainLyrics;
        lyricsState.source = "LRCLIB (Search)";
        return true;
      }
    }
  } catch { /* ignore */ }
  return false;
}

async function tryLyricsOvh(artist: string, title: string): Promise<boolean> {
  try {
    const res = await fetch(`https://api.lyrics.ovh/v1/${encodeURIComponent(artist)}/${encodeURIComponent(title)}`);
    if (!res.ok) return false;
    const data = await res.json();
    if (data.lyrics) {
      lyricsState.content = data.lyrics;
      lyricsState.source = "Lyrics.ovh";
      return true;
    }
  } catch { /* ignore */ }
  return false;
}

function cleanForSearch(artist: string, title: string) {
  let cleanTitle = title.replace(/\(.*\)|\[.*\]/g, "").trim();
  cleanTitle = cleanTitle.replace(/Official Video|Official Audio|Music Video|LYRICS/gi, "").trim();
  const cleanArtist = artist.replace(/ - Topic$/i, "").trim();
  return { cleanArtist, cleanTitle };
}
