
export interface LrcLine {
  time: number;   // seconds
  text: string;
}

class LyricsState {
  visible = $state(false);
  content = $state("");
  syncedLines = $state<LrcLine[]>([]);
  loading = $state(false);
  error = $state("");
  trackId = $state<string | null>(null);
  synced = $state(false);
  source = $state(""); // "LRCLIB", "Lyrics.ovh", etc.

  get hasSynced(): boolean {
    return this.syncedLines.length > 0;
  }

  reset() {
    this.content = "";
    this.syncedLines = [];
    this.error = "";
    this.loading = false;
    this.synced = false;
    this.source = "";
  }
}

/** Parse LRC format timestamps like [01:23.45] into lines */
export function parseLrc(lrc: string): LrcLine[] {
  const lines: LrcLine[] = [];
  // More robust regex to handle various LRC formats
  const regex = /^\[(\d{1,2}):(\d{1,2}(?:\.\d+)?)]\s*(.*)$/;

  for (let raw of lrc.split("\n")) {
    raw = raw.trim();
    if (!raw) continue;

    const m = raw.match(regex);
    if (m) {
      const mins = parseInt(m[1], 10);
      const secsPart = m[2];
      const secs = parseFloat(secsPart);
      const time = mins * 60 + secs;
      const text = m[3].trim();
      
      // Only add non-empty lines, or keep empty ones for pacing?
      // Usually better to keep them if they are timestamps.
      lines.push({ time, text: text || "..." });
    }
  }

  return lines.sort((a, b) => a.time - b.time);
}

export const lyricsState = new LyricsState();
