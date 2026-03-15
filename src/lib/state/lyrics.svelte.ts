
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
  autoScroll = $state(true);

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
    this.autoScroll = true;
  }
}

/** Parse LRC format timestamps like [01:23.45] or [01:23:456] into lines */
export function parseLrc(lrc: string): LrcLine[] {
  const lines: LrcLine[] = [];
  // Regex to handle [mm:ss.xx] or [mm:ss:xx]
  const regex = /^\[(\d{1,2}):(\d{1,2}(?:[.:]\d+)?)]\s*(.*)$/;

  for (let raw of lrc.split("\n")) {
    raw = raw.trim();
    if (!raw || raw.startsWith("[offset:")) continue;

    const m = raw.match(regex);
    if (m) {
      const mins = parseInt(m[1], 10);
      const secsPart = m[2].replace(":", "."); // Normalize [mm:ss:xxx] to [mm:ss.xxx]
      const secs = parseFloat(secsPart);
      const time = mins * 60 + secs;
      const text = m[3].trim();
      
      if (text || lines.length > 0) {
        lines.push({ time, text: text || "..." });
      }
    }
  }

  return lines.sort((a, b) => a.time - b.time);
}

export const lyricsState = new LyricsState();
