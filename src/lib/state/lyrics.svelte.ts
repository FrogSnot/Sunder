export interface SyncedLine {
  time: number; // seconds
  text: string;
}

class LyricsState {
  trackId = $state("");
  loading = $state(false);
  error = $state("");
  content = $state("");
  source = $state("");
  synced = $state(false);
  syncedLines = $state<SyncedLine[]>([]);
  visible = $state(false);

  reset() {
    this.trackId = "";
    this.loading = false;
    this.error = "";
    this.content = "";
    this.source = "";
    this.synced = false;
    this.syncedLines = [];
  }
}

export const lyricsState = new LyricsState();

/** Parse LRC format "[mm:ss.xx] text" into SyncedLine[] */
export function parseLrc(lrc: string): SyncedLine[] {
  const lines: SyncedLine[] = [];
  for (const line of lrc.split("\n")) {
    const match = line.match(/^\[(\d{2}):(\d{2})\.(\d{2,3})\]\s*(.*)/);
    if (match) {
      const mins = parseInt(match[1], 10);
      const secs = parseInt(match[2], 10);
      const ms = parseInt(match[3].padEnd(3, "0"), 10);
      lines.push({
        time: mins * 60 + secs + ms / 1000,
        text: match[4],
      });
    }
  }
  return lines;
}
