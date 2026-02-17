import type { ExploreSection } from "../types";

const CACHE_TTL = 5 * 60 * 1000; // 5 minutes

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
