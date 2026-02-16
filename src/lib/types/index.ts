export interface Track {
  id: string;
  title: string;
  artist: string;
  thumbnail: string;
  duration_secs: number;
  stream_url?: string;
}

export interface SearchResult {
  tracks: Track[];
  source: "local" | "remote";
}

export interface PlaybackProgress {
  position_ms: number;
  duration_ms: number;
  state: string;
}
