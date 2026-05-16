import type { ExploreSection } from "../types";

const CACHE_TTL = 15 * 60 * 1000; // 15 minutes

class ExploreCache {
  sections = $state<ExploreSection[]>([]);
  loaded = $state(false);
  loading = $state(false);
  fetchedAt = $state(0);

  get stale(): boolean {
    return !this.loaded || Date.now() - this.fetchedAt > CACHE_TTL;
  }
}

export const exploreCache = new ExploreCache();
