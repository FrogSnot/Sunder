import type { Track, PlaybackProgress } from "../types";
import { prefetchTrack } from "../ipc/bridge";

const PREFETCH_AHEAD = 2;

class PlayerState {
  currentTrack = $state<Track | null>(null);
  isPlaying = $state(false);
  isBuffering = $state(false);
  isSeeking = $state(false);
  currentTime = $state(0);
  duration = $state(0);
  volume = $state(0.8);
  queue = $state<Track[]>([]);
  queueIndex = $state(-1);
  shuffled = $state(false);
  playbackState = $state("idle");
  downloadPercent = $state(0);
  downloadStage = $state("");

  progress = $derived(this.duration > 0 ? this.currentTime / this.duration : 0);
  formattedTime = $derived(formatTime(this.currentTime));
  formattedDuration = $derived(formatTime(this.duration));
  hasNext = $derived(this.queueIndex < this.queue.length - 1);
  hasPrev = $derived(this.queueIndex > 0);

  updateFromProgress(p: PlaybackProgress) {
    if (!this.isSeeking) {
      this.currentTime = p.position_ms / 1000;
    }
    this.duration = p.duration_ms / 1000;
    this.playbackState = p.state;
    this.isPlaying = p.state === "playing";
    this.isBuffering = p.state === "buffering" || p.state === "loading";
    if (this.isPlaying) {
      this.downloadStage = "";
      this.downloadPercent = 0;
    }
  }

  prefetchAhead(fromIndex: number) {
    for (let i = 1; i <= PREFETCH_AHEAD; i++) {
      const track = this.queue[fromIndex + i];
      if (track) prefetchTrack(track.id).catch(() => {});
    }
  }

  addToQueue(track: Track) {
    if (!this.queue.some((t) => t.id === track.id)) {
      this.queue = [...this.queue, track];
    }
  }

  playNext(track: Track) {
    const filtered = this.queue.filter((t) => t.id !== track.id);
    const insertAt = this.queueIndex + 1;
    filtered.splice(insertAt, 0, track);
    this.queue = [...filtered];
    if (this.currentTrack) {
      this.queueIndex = filtered.findIndex((t) => t.id === this.currentTrack!.id);
    }
  }

  playFromQueue(index: number) {
    if (index >= 0 && index < this.queue.length) {
      this.queueIndex = index;
      this.prefetchAhead(index);
      return this.queue[index];
    }
    return null;
  }

  nextTrack(): Track | null {
    if (this.queueIndex < this.queue.length - 1) {
      this.queueIndex++;
      this.prefetchAhead(this.queueIndex);
      return this.queue[this.queueIndex];
    }
    return null;
  }

  prevTrack(): Track | null {
    if (this.queueIndex > 0) {
      this.queueIndex--;
      return this.queue[this.queueIndex];
    }
    return null;
  }

  removeFromQueue(index: number) {
    const updated = [...this.queue];
    updated.splice(index, 1);
    this.queue = updated;
    if (index < this.queueIndex) {
      this.queueIndex--;
    } else if (index === this.queueIndex) {
      if (this.queueIndex >= this.queue.length) {
        this.queueIndex = this.queue.length - 1;
      }
    }
  }

  shuffle() {
    if (this.queue.length <= 1) return;
    const current = this.queueIndex >= 0 ? this.queue[this.queueIndex] : null;
    const rest = this.queue.filter((_, i) => i !== this.queueIndex);
    for (let i = rest.length - 1; i > 0; i--) {
      const j = Math.floor(Math.random() * (i + 1));
      [rest[i], rest[j]] = [rest[j], rest[i]];
    }
    if (current) {
      this.queue = [current, ...rest];
      this.queueIndex = 0;
    } else {
      this.queue = [...rest];
      this.queueIndex = -1;
    }
    this.shuffled = true;
  }

  moveInQueue(from: number, to: number) {
    if (from === to) return;
    if (from < 0 || to < 0 || from >= this.queue.length || to >= this.queue.length) return;
    const updated = [...this.queue];
    const [item] = updated.splice(from, 1);
    updated.splice(to, 0, item);
    this.queue = updated;
    if (this.queueIndex === from) {
      this.queueIndex = to;
    } else if (from < this.queueIndex && to >= this.queueIndex) {
      this.queueIndex--;
    } else if (from > this.queueIndex && to <= this.queueIndex) {
      this.queueIndex++;
    }
  }

  clearQueue() {
    this.queue = [];
    this.queueIndex = -1;
    this.shuffled = false;
  }
}

function formatTime(secs: number): string {
  if (!secs || secs < 0) return "0:00";
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export const player = new PlayerState();
