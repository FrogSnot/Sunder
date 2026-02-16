import type { Track } from "../types";

class SearchState {
  results = $state<Track[]>([]);
  searching = $state(false);
  query = $state("");
}

export const searchState = new SearchState();
