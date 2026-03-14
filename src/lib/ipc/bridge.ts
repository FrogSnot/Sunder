import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { Track, SearchResult, PlaybackProgress, Playlist, ExploreData, EqSettings } from "../types";
import { player } from "../state/player.svelte.ts";
import { lyricsState } from "../state/lyrics.svelte.ts";

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
  } else {
    const insertAt = player.queueIndex + 1;
    const updated = [...player.queue];
    updated.splice(insertAt, 0, track);
    player.queue = updated;
    player.queueIndex = insertAt;
  }
  player.prefetchAhead(player.queueIndex);
  await invoke("play_track", { trackId: track.id });
}

let advancing = false;

export async function playNext(): Promise<void> {
  if (advancing) return;
  advancing = true;
  try {
    const next = player.nextTrack();
    if (next) {
      await playTrack(next);
    }
  } finally {
    advancing = false;
  }
}

export async function playPrev(): Promise<void> {
  if (advancing) return;
  advancing = true;
  try {
    const prev = player.prevTrack();
    if (prev) {
      await playTrack(prev);
    }
  } finally {
    advancing = false;
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

let seekTimer: ReturnType<typeof setTimeout> | null = null;

export async function seek(positionSecs: number): Promise<void> {
  player.currentTime = positionSecs;
  player.isSeeking = true;
  if (seekTimer) clearTimeout(seekTimer);
  seekTimer = setTimeout(() => { player.isSeeking = false; }, 400);
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

export async function importYtPlaylist(url: string, playlistName: string = ""): Promise<Playlist> {
  return invoke<Playlist>("import_yt_playlist", { url, playlistName });
}

export async function getSubtitles(videoId: string): Promise<string> {
  return invoke<string>("get_subtitles", { videoId });
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

// --- Multi-source lyrics fetching ---

function cleanForSearch(artist: string, title: string) {
  const cleanArtist = artist
    .replace(/ - Topic$/, "")
    .replace(/VEVO$/i, "")
    .trim();
  const cleanTitle = title
    .replace(/\s*\(Official\s*(Music\s*)?Video\)/i, "")
    .replace(/\s*\[Official\s*(Music\s*)?Video\]/i, "")
    .replace(/\s*\(Lyrics?\)/i, "")
    .replace(/\s*\[Lyrics?\]/i, "")
    .replace(/\s*\(Audio\)/i, "")
    .replace(/\s*\(Visuali[sz]er\)/i, "")
    .replace(/\s*\(ft\..*?\)/i, "")
    .replace(/\s*\[ft\..*?\]/i, "")
    .replace(/\s*\(feat\..*?\)/i, "")
    .replace(/\s*\[feat\..*?\]/i, "")
    .trim();
  return { cleanArtist, cleanTitle };
}

async function tryLrclib(artist: string, title: string, durationSecs?: number): Promise<boolean> {
  try {
    const params = new URLSearchParams({
      artist_name: artist,
      track_name: title,
    });
    if (durationSecs && durationSecs > 0) {
      params.set("duration", Math.round(durationSecs).toString());
    }

    const res = await fetch(`https://lrclib.net/api/search?${params.toString()}`, {
      headers: { "User-Agent": "Sunder v0.1.0 (https://github.com/FrogSnot/Sunder)" },
    });

    if (!res.ok) return false;

    const results = await res.json();
    if (!Array.isArray(results) || results.length === 0) return false;

    // Prefer results with synced lyrics
    const withSynced = results.find((r: any) => r.syncedLyrics);
    const best = withSynced || results[0];

    if (best.syncedLyrics) {
      const { parseLrc } = await import("../state/lyrics.svelte.ts");
      const lines = parseLrc(best.syncedLyrics);
      if (lines.length > 0) {
        lyricsState.syncedLines = lines;
        lyricsState.synced = true;
      }
    }

    if (best.plainLyrics) {
      lyricsState.content = best.plainLyrics;
      lyricsState.source = "LRCLIB";
    }

    return lyricsState.synced || !!lyricsState.content;
  } catch {
    return false;
  }
}

async function tryLrclibQuery(query: string): Promise<boolean> {
  try {
    const params = new URLSearchParams({ q: query });
    const res = await fetch(`https://lrclib.net/api/search?${params.toString()}`, {
      headers: { "User-Agent": "Sunder v0.1.0 (https://github.com/FrogSnot/Sunder)" },
    });
    if (!res.ok) return false;

    const results = await res.json();
    if (!Array.isArray(results) || results.length === 0) return false;

    const withSynced = results.find((r: any) => r.syncedLyrics);
    const best = withSynced || results[0];

    if (best.syncedLyrics) {
      const { parseLrc } = await import("../state/lyrics.svelte.ts");
      const lines = parseLrc(best.syncedLyrics);
      if (lines.length > 0) {
        lyricsState.syncedLines = lines;
        lyricsState.synced = true;
      }
    }

    if (best.plainLyrics) {
      lyricsState.content = best.plainLyrics;
      lyricsState.source = "LRCLIB";
    }

    return lyricsState.synced || !!lyricsState.content;
  } catch {
    return false;
  }
}

async function tryLyricsOvh(artist: string, title: string): Promise<boolean> {
  try {
    const res = await fetch(
      `https://api.lyrics.ovh/v1/${encodeURIComponent(artist)}/${encodeURIComponent(title)}`
    );
    if (!res.ok) return false;

    const data = await res.json();
    if (data.lyrics) {
      lyricsState.content = data.lyrics;
      lyricsState.source = "Lyrics.ovh";
      return true;
    }
    return false;
  } catch {
    return false;
  }
}

export async function fetchLyrics(artist: string, title: string, trackId: string, durationSecs?: number): Promise<void> {
  lyricsState.reset();
  lyricsState.loading = true;
  lyricsState.trackId = trackId;

  try {
    const { cleanArtist, cleanTitle } = cleanForSearch(artist, title);

    // 1. LRCLIB structured search (cleaned + duration)
    if (await tryLrclib(cleanArtist, cleanTitle, durationSecs)) return;

    // 2. LRCLIB structured (original title + duration)
    if (cleanTitle !== title && await tryLrclib(artist, title, durationSecs)) return;

    // 3. LRCLIB structured (cleaned, no duration)
    if (durationSecs && await tryLrclib(cleanArtist, cleanTitle)) return;

    // 4. LRCLIB free-text query ("artist title")
    if (await tryLrclibQuery(`${cleanArtist} ${cleanTitle}`)) return;

    // 5. LRCLIB free-text query (title only)
    if (await tryLrclibQuery(cleanTitle)) return;

    // 6. lyrics.ovh with cleaned title
    if (await tryLyricsOvh(cleanArtist, cleanTitle)) return;

    // 7. lyrics.ovh with original title
    if (cleanTitle !== title && await tryLyricsOvh(artist, title)) return;

    // 8. YouTube subtitles/captions (last resort, via backend)
    try {
      const subs = await invoke<string>("get_subtitles", { videoId: trackId });
      if (subs && subs.trim().length > 20) {
        lyricsState.content = subs;
        lyricsState.source = "YouTube";
        return;
      }
    } catch { /* no subtitles available */ }

    lyricsState.error = "Lyrics not found.";
  } catch {
    lyricsState.error = "Failed to fetch lyrics.";
  } finally {
    lyricsState.loading = false;
  }
}


export function initProgressListener(): () => void {
  let unlistenProgress: (() => void) | undefined;
  let unlistenDownload: (() => void) | undefined;
  let unlistenFinished: (() => void) | undefined;
  let unlistenError: (() => void) | undefined;

  listen<PlaybackProgress>("playback-progress", (event) => {
    player.updateFromProgress(event.payload);
  }).then((fn) => { unlistenProgress = fn; });

  listen<{ percent: number; stage: string }>("download-progress", (event) => {
    player.downloadPercent = event.payload.percent;
    player.downloadStage = event.payload.stage;
  }).then((fn) => { unlistenDownload = fn; });

  listen("track-finished", () => {
    playNext();
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
          playNext();
        }
      }, 4000);
    }
  }).then((fn) => { unlistenError = fn; });

  return () => {
    unlistenProgress?.();
    unlistenDownload?.();
    unlistenFinished?.();
    unlistenError?.();
  };
}
