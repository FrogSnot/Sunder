import { search as bridgeSearch } from "../ipc/bridge";
import { toastState } from "./toast.svelte";
import type { Track } from "../types";

class SearchState {
  results = $state<Track[]>([]);
  searching = $state(false);
  query = $state("");
  limit = $state(20);
  hasMore = $state(false);
  resultSource = $state<"local" | "remote" | null>(null);

  #gen = 0;
  bumpGen() { return ++this.#gen; }
  isLatest(g: number) { return this.#gen === g; }

  async remoteSearch(q: string) {
    if (!q) return;
    this.searching = true;
    const myGen = this.bumpGen();
    try {
      const res = await bridgeSearch(q, this.limit);
      if (!this.isLatest(myGen)) return;
      this.results = res.tracks;
      this.hasMore = res.tracks.length >= this.limit;
      this.resultSource = res.source;
    } catch (e) {
      if (!this.isLatest(myGen)) return;
      console.error("search failed:", e);
      toastState.add(`Search failed: ${e}`, "error", 8000);
    } finally {
      if (this.isLatest(myGen)) this.searching = false;
    }
  }

  async loadMore() {
    this.limit += 20;
    await this.remoteSearch(this.query);
  }
}

export const searchState = new SearchState();
