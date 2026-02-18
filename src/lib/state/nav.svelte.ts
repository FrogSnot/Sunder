export type Tab = "search" | "explore" | "playlists" | "playlist-detail" | "queue";

class NavState {
  activeTab = $state<Tab>("search");
  activePlaylistId = $state<number | null>(null);
  activePlaylistName = $state("");
}

export const nav = new NavState();
