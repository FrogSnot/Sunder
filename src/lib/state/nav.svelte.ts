export type Tab = "search" | "explore" | "playlists" | "playlist-detail" | "queue" | "downloads";

class NavState {
  activeTab = $state<Tab>("search");
  activePlaylistId = $state<number | null>(null);
  activePlaylistName = $state("");
  focusMode = $state(false);

  toggleFocus(hasTrack: boolean) {
    if (!this.focusMode && !hasTrack) return;
    this.focusMode = !this.focusMode;
  }

  /** Jump straight to a playlist detail view (used by the queue chip). */
  jumpToPlaylist(id: number, name: string) {
    this.activeTab = "playlist-detail";
    this.activePlaylistId = id;
    this.activePlaylistName = name;
  }
}

export const nav = new NavState();
