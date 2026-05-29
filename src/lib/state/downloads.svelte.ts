import type { DownloadEvent, DownloadStatus, Track } from "../types";

export interface DownloadProgress {
  status: DownloadStatus;
  percent: number;
}

class DownloadsState {
  ids = $state(new Set<string>());
  progress = $state(new Map<string, DownloadProgress>());
  meta = $state(new Map<string, Track>());

  // Aggregates for the current download batch. A batch starts when the first
  // track begins after everything has settled, and persists (so the panel can
  // show a final tally) until the next batch begins.
  sessionTotal = $state(0);
  sessionDone = $state(0);
  sessionFailed = $state(0);
  failedIds = $state(new Set<string>());

  #seen = new Set<string>();
  #settled = true;

  isDownloaded(id: string): boolean {
    return this.ids.has(id);
  }

  getProgress(id: string): DownloadProgress | undefined {
    return this.progress.get(id);
  }

  getMeta(id: string): Track | undefined {
    return this.meta.get(id);
  }

  isActive(id: string): boolean {
    const p = this.progress.get(id);
    return !!p && p.status !== "done" && p.status !== "error";
  }

  /** Smooth 0-100 progress across the whole batch, robust to large queues. */
  get sessionPercent(): number {
    if (this.sessionTotal === 0) return 0;
    let frac = 0;
    for (const p of this.progress.values()) {
      if (p.status === "downloading") frac += Math.min(p.percent, 100) / 100;
      else if (p.status === "converting") frac += 0.9;
    }
    const done = this.sessionDone + this.sessionFailed + frac;
    return Math.min(100, Math.round((done / this.sessionTotal) * 100));
  }

  register(tracks: Track | Track[]) {
    const list = Array.isArray(tracks) ? tracks : [tracks];
    if (list.length === 0) return;
    const next = new Map(this.meta);
    for (const t of list) next.set(t.id, t);
    this.meta = next;
  }

  setDownloaded(ids: string[]) {
    this.ids = new Set(ids);
  }

  updateFromEvent(ev: DownloadEvent) {
    // Begin a fresh batch if the previous one had fully settled.
    if (this.#settled && !this.#seen.has(ev.track_id)) {
      this.sessionTotal = 0;
      this.sessionDone = 0;
      this.sessionFailed = 0;
      this.failedIds = new Set();
      this.#settled = false;
    }
    if (!this.#seen.has(ev.track_id)) {
      this.#seen.add(ev.track_id);
      this.sessionTotal += 1;
    }

    const next = new Map(this.progress);
    if (ev.status === "done") {
      next.delete(ev.track_id);
      this.progress = next;
      const ids = new Set(this.ids);
      ids.add(ev.track_id);
      this.ids = ids;
      this.sessionDone += 1;
      if (this.failedIds.has(ev.track_id)) {
        const f = new Set(this.failedIds);
        f.delete(ev.track_id);
        this.failedIds = f;
        this.sessionFailed = Math.max(0, this.sessionFailed - 1);
      }
    } else if (ev.status === "error") {
      next.delete(ev.track_id);
      this.progress = next;
      if (!this.failedIds.has(ev.track_id)) {
        const f = new Set(this.failedIds);
        f.add(ev.track_id);
        this.failedIds = f;
        this.sessionFailed += 1;
      }
    } else {
      next.set(ev.track_id, { status: ev.status, percent: ev.percent });
      this.progress = next;
    }

    // Once nothing is in flight the batch is settled; the next new track will
    // open a fresh batch.
    if (this.progress.size === 0) {
      this.#seen.clear();
      this.#settled = true;
    }
  }

  clearFailed(id: string) {
    if (!this.failedIds.has(id)) return;
    const f = new Set(this.failedIds);
    f.delete(id);
    this.failedIds = f;
    this.sessionFailed = Math.max(0, this.sessionFailed - 1);
  }

  markRemoved(id: string) {
    const ids = new Set(this.ids);
    ids.delete(id);
    this.ids = ids;
    const p = new Map(this.progress);
    p.delete(id);
    this.progress = p;
  }
}

export const downloads = new DownloadsState();
