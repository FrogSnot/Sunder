import { search as bridgeSearch } from "../ipc/bridge";
import { toastState } from "./toast.svelte";
import type { Track } from "../types";

class SearchState {
  results = $state<Track[]>([]);
  searching = $state(false);
  query = $state("");
  limit = $state(20);
  hasMore = $state(false);

  async remoteSearch(q: string) {
    if (!q) return;
    this.searching = true;
    try {
      const res = await bridgeSearch(q, this.limit);
      this.results = res.tracks;
      this.hasMore = res.tracks.length >= this.limit;
    } catch (e) {
      console.error("search failed:", e);
      toastState.add(`Search failed: ${e}`, "error", 8000);
    } finally {
      this.searching = false;
    }
  }

  async loadMore() {
    this.limit += 20;
    await this.remoteSearch(this.query);
  }
}

export const searchState = new SearchState();
