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
}

export const nav = new NavState();
