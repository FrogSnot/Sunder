import type { Track, PlaybackProgress } from "../types";

class PlayerState {
  currentTrack = $state<Track | null>(null);
  isPlaying = $state(false);
  isBuffering = $state(false);
  isSeeking = $state(false);
  currentTime = $state(0);
  duration = $state(0);
  volume = $state(0.8);
  queue = $state<Track[]>([]);
  playbackState = $state("idle");
  downloadPercent = $state(0);
  downloadStage = $state("");

  progress = $derived(this.duration > 0 ? this.currentTime / this.duration : 0);
  formattedTime = $derived(formatTime(this.currentTime));
  formattedDuration = $derived(formatTime(this.duration));

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
}

function formatTime(secs: number): string {
  if (!secs || secs < 0) return "0:00";
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export const player = new PlayerState();
