import type { Track, PlaybackProgress } from "../types";
import { prefetchTrack, setRepeatMode, stop, getTracksByIds } from "../ipc/bridge";

const PREFETCH_AHEAD = 2;
/** Context tracks materialized into the queue ahead of the current one. */
const CONTEXT_AHEAD = 2;
/** localStorage key for the lazy playback context (issue #44). */
const CTX_STORAGE_KEY = "sunder.playback-context";

/**
 * Track resolution for lazy context materialization. Injectable so tests and
 * dev harnesses can run without the Tauri IPC bridge.
 */
let resolveTracks: (ids: string[]) => Promise<Track[]> = getTracksByIds;
export function setTrackResolver(fn: (ids: string[]) => Promise<Track[]>): void {
  resolveTracks = fn;
}

export interface PlaybackContext {
  playlistId: number;
  playlistName: string;
  /** Track ids in playlist order. Ids only: the playlist is resolved lazily. */
  order: string[];
  /** Index into order of the track currently playing from the context. */
  index: number;
  /**
   * True while foreign tracks sit between the player and the next context
   * track ("play next" inserts, single plays from other views, manual jumps).
   * Those must drain first; the chain resumes automatically after them.
   */
  suspended: boolean;
}

class PlayerState {
  currentTrack = $state<Track | null>(null);
  isPlaying = $state(false);
  isBuffering = $state(false);
  isSeeking = $state(false);
  currentTime = $state(0);
  duration = $state(0);
  volume = $state(0.8);
  speed = $state(1.0);
  queue = $state<Track[]>([]);
  queueIndex = $state(-1);
  shuffled = $state(false);
  playbackState = $state("idle");
  downloadPercent = $state(0);
  downloadStage = $state("");
  consecutiveErrors = $state(0);
  lastError = $state("");
  /** Remedy selector from the backend: "ytdlp_blocked" | "device" | "load" */
  errorKind = $state("");
  /** Version of the yt-dlp that produced the current blocked-stream error */
  ytdlpVersion = $state("");
  /** "override" | "managed" | "system": where the active yt-dlp comes from */
  ytdlpSource = $state("");
  /** One-click update offered? (Linux + no env override; backend decides) */
  ytdlpCanUpdate = $state(false);
  ytdlpUpdating = $state(false);
  failedTrack = $state<Track | null>(null);
  findingAlt = $state(false);
  sleepTimerRemaining = $state<number | null>(null);
  sleepTimerSetMinutes = $state<number | null>(null);
  private sleepTimerHandle: ReturnType<typeof setInterval> | null = null;

  eqEnabled = $state(false);
  eqGains = $state<number[]>([0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
  eqPreset = $state("Flat");
  showEq = $state(false);

  repeatMode = $state<"off" | "queue" | "track">("off");
  /** Lazy playlist context: what auto-advance continues through (issue #44). */
  playbackContext = $state<PlaybackContext | null>(null);

  progress = $derived(this.duration > 0 ? this.currentTime / this.duration : 0);
  formattedTime = $derived(formatTime(this.currentTime));
  formattedDuration = $derived(formatTime(this.duration));
  formattedSleepTimer = $derived(this.sleepTimerRemaining !== null ? formatTime(this.sleepTimerRemaining) : "");
  /** True while a foreign track (or a bypassed one-off) sits in front of the
   * chain. The queue chip dims on this instead of unmounting, so there is no
   * hide/show flicker around one-off plays (issue #44 review nit). */
  contextSuspended = $derived(!!this.playbackContext?.suspended);
  /** True when the context chain has a next track to continue through.
   * Gated on suspension honestly: while a foreign track is current AND the
   * queue ahead is empty, Next would do nothing, so it must not claim so. */
  contextHasNext = $derived.by(() => {
    const ctx = this.playbackContext;
    return !!ctx && !ctx.suspended && ctx.index + 1 < ctx.order.length;
  });
  hasNext = $derived(
    this.queueIndex < this.queue.length - 1 ||
      (this.repeatMode === "queue" && this.queue.length > 0) ||
      this.contextHasNext,
  );
  hasPrev = $derived.by(() => {
    const ctx = this.playbackContext;
    return (
      this.queueIndex > 0 ||
      (this.repeatMode === "queue" && this.queue.length > 0) ||
      (!!ctx && ctx.index > 0)
    );
  });

  updateFromProgress(p: PlaybackProgress) {
    if (!this.isSeeking) {
      this.currentTime = p.position_ms / 1000;
    }
    this.duration = p.duration_ms / 1000;
    this.playbackState = p.state;
    this.volume = p.volume;
    this.speed = p.speed;
    this.isPlaying = p.state === "playing";
    this.isBuffering = p.state === "buffering" || p.state === "loading";
    if (this.isPlaying) {
      this.downloadStage = "";
      this.downloadPercent = 0;
      this.consecutiveErrors = 0;
      this.lastError = "";
      this.errorKind = "";
      this.ytdlpVersion = "";
      this.ytdlpSource = "";
      this.failedTrack = null;
      this.findingAlt = false;
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
      this.queueEpoch++;
      this.queue = [...this.queue, track];
    }
  }

  setQueue(tracks: Track[]) {
    this.queueEpoch++;
    this.queue = [...tracks];
    this.queueIndex = -1;
    this.playbackContext = null;
  }

  /**
   * Start a playlist at `index` WITHOUT materializing it (issue #44). Only the
   * clicked track is resolved now; the rest of the playlist is an id order
   * that gets pulled into the queue on demand during advance/prefetch.
   */
  async playFromPlaylist(
    playlistId: number,
    playlistName: string,
    order: string[],
    index: number,
  ): Promise<Track | null> {
    if (index < 0 || index >= order.length) return null;
    this.queueEpoch++;
    this.playbackContext = {
      playlistId,
      playlistName,
      order: [...order],
      index,
      suspended: false,
    };
    this.queue = [];
    this.queueIndex = -1;
    const track = await this.materializeTrack(order[index]);
    if (!track) {
      this.playbackContext = null;
      return null;
    }
    this.queue = [track];
    this.queueIndex = 0;
    this.ensureNext();
    return track;
  }

  private async materializeTrack(id: string): Promise<Track | null> {
    try {
      const tracks = await resolveTracks([id]);
      return tracks[0] ?? null;
    } catch {
      return null;
    }
  }

  /**
   * Sync the context chain with `trackId`. Context track -> index moves there
   * (and a suspended chain resumes); foreign track -> suspend until the chain
   * resurfaces. This is what keeps "play next" insertions from hijacking the
   * chain: the insertion drains, then the playlist continues at N+1.
   */
  resyncContextForTrack(trackId: string | null): void {
    const ctx = this.playbackContext;
    if (!ctx || !trackId) return;
    if (this.contextBypass.has(trackId)) {
      // Deliberate one-off of an upcoming playlist track ("play next" on a
      // song that belongs to the playlist): play it now, but do not move the
      // chain. The entry STAYS until a chain track re-anchors, because the
      // advance path resyncs the same id twice (nextTrack + playTrack); a
      // second resync of the bypassed id must not flip the chain back on.
      ctx.suspended = true;
      return;
    }
    const ci = ctx.order.indexOf(trackId);
    if (ci === -1) {
      ctx.suspended = true;
      return;
    }
    // A real context track is current: the user advanced (or explicitly
    // clicked) into the chain, so the bypass window is over.
    this.contextBypass.clear();
    ctx.index = ci;
    ctx.suspended = false;
  }

  /**
   * Lazily materialize the next CONTEXT_AHEAD context tracks so the playlist
   * continues right after the current one. Insertion point is AFTER any
   * foreign run ("play next" inserts), so user insertions always drain first
   * and are never reordered or duplicated. Idempotent: pre-existing context
   * tracks in the aligned slots count toward the look-ahead.
   */
  ensureNext(): void {
    const ctx = this.playbackContext;
    if (!ctx || ctx.suspended) return;
    if (this.queueIndex < 0 || this.queueIndex >= this.queue.length) return;
    if (this.queue[this.queueIndex]?.id !== ctx.order[ctx.index]) return;
    // Skip any foreign tracks queued directly after the current one; the
    // chain continues behind them.
    let at = this.queueIndex + 1;
    while (at < this.queue.length && !ctx.order.includes(this.queue[at].id)) {
      at++;
    }
    // Count context tracks already materialized in the aligned slots.
    let aligned = 0;
    while (
      aligned < CONTEXT_AHEAD &&
      this.queue[at + aligned]?.id === ctx.order[ctx.index + 1 + aligned]
    ) {
      aligned++;
    }
    const nextCtx = ctx.index + 1 + aligned;
    if (aligned >= CONTEXT_AHEAD || nextCtx >= ctx.order.length) return;
    const need = Math.min(CONTEXT_AHEAD - aligned, ctx.order.length - nextCtx);
    const ids: string[] = [];
    for (let i = 0; i < need; i++) {
      const id = ctx.order[nextCtx + i];
      if (!this.queue.some((t) => t.id === id)) ids.push(id);
    }
    if (ids.length === 0) return;
    const insertAt = at + aligned;
    // Capture state so a stale insert can never land after the user moved on.
    const ctx0 = ctx;
    const qIndex0 = this.queueIndex;
    const qLen0 = this.queue.length;
    const epoch0 = this.queueEpoch;
    void resolveTracks(ids)
      .then((tracks) => {
        if (
          this.playbackContext !== ctx0 ||
          this.queueEpoch !== epoch0 ||
          this.queueIndex !== qIndex0 ||
          this.queue.length !== qLen0
        ) {
          return; // mutated meanwhile: shuffle, insert, jump, advance, clear
        }
        const fresh = tracks.filter((t) => t && !this.queue.some((q) => q.id === t.id));
        if (fresh.length === 0) return;
        const updated = [...this.queue];
        updated.splice(insertAt, 0, ...fresh);
        this.queue = updated;
      })
      // Prefetch-only failure: the chain still advances via the bridge
      // fallback, so a rejected bulk resolve here is safe to ignore.
      .catch(() => {});
  }

  /** Ids queued as one-off "play next" inserts while they belong to the
   * active playlist: playing them must not re-anchor the chain. */
  private contextBypass = new Set<string>();

  /**
   * Bumped on every queue/context mutation. Async materialization (ensureNext)
   * captures the epoch before resolving and aborts if it changed, so a
   * shuffle/insert/advance landing mid-resolve can never apply a stale insert
   * plan derived from pre-mutation state.
   */
  private queueEpoch = 0;

  playNext(track: Track) {
    this.queueEpoch++;
    if (this.playbackContext?.order.includes(track.id)) {
      this.contextBypass.add(track.id);
    }
    const filtered = this.queue.filter((t) => t.id !== track.id);
    const insertAt = this.queueIndex + 1;
    filtered.splice(insertAt, 0, track);
    this.queue = [...filtered];
    if (this.currentTrack) {
      this.queueIndex = filtered.findIndex((t) => t.id === this.currentTrack!.id);
    }
    this.ensureNext();
  }

  /** Explicit Play All keeps a materialized queue but rides the same context
   * chain (issue #44): chip + auto-advance semantics stay consistent. */
  attachContext(playlistId: number, playlistName: string, order: string[], index: number): void {
    if (index < 0 || index >= order.length) return;
    this.queueEpoch++;
    this.playbackContext = {
      playlistId,
      playlistName,
      order: [...order],
      index,
      suspended: false,
    };
  }

  /**
   * Async fallback for bridge.playNext once the materialized window is
   * exhausted: resolve the next context track on demand and slide it into
   * the queue. Returns null when there is no live chain.
   */
  async nextContextTrack(): Promise<Track | null> {
    const ctx = this.playbackContext;
    const curId = this.queue[this.queueIndex]?.id ?? null;
    // index === -1 is the repeat-queue restart signal: the playlist chain
    // played out and repeat starts it from the top. Do NOT resync here or
    // the current (last) track would re-anchor the index at the end.
    if (ctx && ctx.index !== -1) this.resyncContextForTrack(curId);
    if (!ctx || ctx.suspended) return null;
    const nextId = ctx.order[ctx.index + 1];
    if (!nextId) return null;
    const existing = this.queue.findIndex((t) => t.id === nextId);
    if (existing !== -1) {
      this.queueIndex = existing;
      this.ensureNext();
      return this.queue[existing] ?? null;
    }
    const track = await this.materializeTrack(nextId);
    if (!track) return null;
    this.queueEpoch++;
    const updated = [...this.queue];
    updated.splice(this.queueIndex + 1, 0, track);
    this.queue = updated;
    this.queueIndex = this.queueIndex + 1;
    this.ensureNext();
    return track;
  }

  /** Mirror of nextContextTrack for previous. */
  async prevContextTrack(): Promise<Track | null> {
    const ctx = this.playbackContext;
    const curId = this.queue[this.queueIndex]?.id ?? null;
    if (ctx) this.resyncContextForTrack(curId);
    if (!ctx || ctx.suspended) return null;
    const prevId = ctx.order[ctx.index - 1];
    if (!prevId) return null;
    const existing = this.queue.findIndex((t) => t.id === prevId);
    if (existing !== -1) {
      this.queueIndex = existing;
      this.ensureNext();
      return this.queue[existing] ?? null;
    }
    const track = await this.materializeTrack(prevId);
    if (!track) return null;
    this.queueEpoch++;
    const updated = [...this.queue];
    updated.splice(this.queueIndex, 0, track);
    this.queue = updated;
    this.ensureNext();
    return track;
  }

  playFromQueue(index: number) {
    if (index >= 0 && index < this.queue.length) {
      this.queueIndex = index;
      const track = this.queue[index];
      // An explicit click is a clear statement of intent: drop any stale
      // bypass marker so the chain re-anchors here.
      if (track) this.contextBypass.delete(track.id);
      // Jumping to a context track resumes the chain; a foreign track
      // suspends it until the chain resurfaces.
      this.resyncContextForTrack(track?.id ?? null);
      this.ensureNext();
      this.prefetchAhead(index);
      return track;
    }
    return null;
  }

  nextTrack(manual = false): Track | null {
    if (!manual && this.repeatMode === "track") {
      return this.queue[this.queueIndex] ?? null;
    }
    if (this.queueIndex < this.queue.length - 1) {
      this.queueIndex++;
      const track = this.queue[this.queueIndex];
      this.resyncContextForTrack(track?.id ?? null);
      this.ensureNext();
      this.prefetchAhead(this.queueIndex);
      return track ?? null;
    }
    // Context chain handling beyond the materialized window. The bridge
    // resolves lazily when we return null (issue #44).
    const ctx = this.playbackContext;
    if (ctx && !ctx.suspended && ctx.order.length > 0) {
      if (ctx.index + 1 < ctx.order.length) {
        const nextId = ctx.order[ctx.index + 1];
        const existing = this.queue.findIndex((t) => t.id === nextId);
        if (existing !== -1) {
          this.queueIndex = existing;
          this.ensureNext();
          this.prefetchAhead(existing);
          return this.queue[existing];
        }
        // Next chain track not materialized yet: defer to the async
        // resolver instead of wrapping the queue underneath it.
        return null;
      }
      if (this.repeatMode === "queue") {
        // The playlist chain has played out; repeat restarts the CONTEXT
        // from the top (not just the materialized window). nextContextTrack
        // sees index === -1 and lazily resolves playlist track 0.
        ctx.index = -1;
      }
      return null;
    }
    if (this.repeatMode === "queue" && this.queue.length > 0) {
      this.queueIndex = 0;
      const track = this.queue[0];
      this.resyncContextForTrack(track?.id ?? null);
      this.ensureNext();
      this.prefetchAhead(0);
      return track;
    }
    return null;
  }

  cycleRepeat() {
    if (this.repeatMode === "off") {
      this.repeatMode = "queue";
    } else if (this.repeatMode === "queue") {
      this.repeatMode = "track";
    } else {
      this.repeatMode = "off";
    }
    setRepeatMode(this.repeatMode).catch(() => {});
  }

  prevTrack(manual = false): Track | null {
    if (!manual && this.repeatMode === "track") {
      return this.queue[this.queueIndex] ?? null;
    }
    if (this.queueIndex > 0) {
      this.queueIndex--;
      const track = this.queue[this.queueIndex];
      this.resyncContextForTrack(track?.id ?? null);
      this.ensureNext();
      return track ?? null;
    }
    if (this.repeatMode === "queue" && this.queue.length > 0) {
      this.queueIndex = this.queue.length - 1;
      const track = this.queue[this.queueIndex];
      this.resyncContextForTrack(track?.id ?? null);
      this.ensureNext();
      return track ?? null;
    }
    // Context fallback: step back through the playlist past the window.
    const ctx = this.playbackContext;
    if (ctx && !ctx.suspended && ctx.index > 0) {
      const prevId = ctx.order[ctx.index - 1];
      const existing = this.queue.findIndex((t) => t.id === prevId);
      if (existing !== -1) {
        this.queueIndex = existing;
        this.ensureNext();
        return this.queue[existing];
      }
    }
    return null;
  }

  removeFromQueue(index: number) {
    this.queueEpoch++;
    const removedId = this.queue[index]?.id;
    const updated = [...this.queue];
    updated.splice(index, 1);
    this.queue = updated;
    // Removing the current track keeps the chain: the track plays out, and the
    // context resyncs on the next advance.
    if (index === this.queueIndex) {
      if (this.queueIndex >= this.queue.length) {
        this.queueIndex = this.queue.length - 1;
      }
      return;
    }
    if (index < this.queueIndex) {
      this.queueIndex--;
    }
    // Removing an upcoming context track just shortens the chain there; the
    // rest of the playlist order continues after it.
    const ctx = this.playbackContext;
    if (ctx && removedId) {
      const ci = ctx.order.indexOf(removedId);
      if (ci > ctx.index) {
        ctx.order = ctx.order.filter((id) => id !== removedId);
      }
    }
  }

  shuffle() {
    this.queueEpoch++;
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
    // Shuffle rewrites the remaining context to match what the user now sees,
    // so the chain keeps playing the shuffled order (issue #44). The queue
    // rows are ALREADY shuffled above - tailIds must be derived from them
    // as-is; re-shuffling here would desync queue and context ~50% of the
    // time. Only ids beyond the materialized window (unshuffled by the queue
    // shuffle, since they are not queue rows) get a shuffle of their own.
    const ctx = this.playbackContext;
    if (ctx && current && ctx.order[ctx.index] === current.id) {
      const tailIds = this.queue.slice(this.queueIndex + 1).map((t) => t.id);
      const inWindow = new Set(tailIds);
      const beyond = ctx.order.slice(ctx.index + 1).filter((id) => !inWindow.has(id));
      shuffleIds(beyond);
      ctx.order = [...ctx.order.slice(0, ctx.index + 1), ...tailIds, ...beyond];
      ctx.suspended = false;
      // Refill the materialized window in the NEW order. Any ensureNext
      // promise that was in flight during the shuffle just aborted via the
      // epoch guard, so this one re-derives against the shuffled state.
      this.ensureNext();
    }
  }

  moveInQueue(from: number, to: number) {
    if (from === to) return;
    this.queueEpoch++;
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
    this.queueEpoch++;
    this.queue = [];
    this.queueIndex = -1;
    this.shuffled = false;
    this.playbackContext = null;
    this.contextBypass.clear();
  }

  setSleepTimer(minutes: number) {
    this.clearSleepTimer();
    if (minutes <= 0) return;

    this.sleepTimerSetMinutes = minutes;
    this.sleepTimerRemaining = minutes * 60;
    this.sleepTimerHandle = setInterval(() => {
      if (this.sleepTimerRemaining !== null && this.sleepTimerRemaining > 0) {
        this.sleepTimerRemaining--;
        if (this.sleepTimerRemaining === 0) {
          stop().catch(() => {});
          this.isPlaying = false;
          this.isBuffering = false;
          this.currentTime = 0;
          this.duration = 0;
          this.playbackState = "stopped";
          this.clearSleepTimer();
        }
      } else {
        this.clearSleepTimer();
      }
    }, 1000);
  }

  clearSleepTimer() {
    if (this.sleepTimerHandle) {
      clearInterval(this.sleepTimerHandle);
      this.sleepTimerHandle = null;
    }
    this.sleepTimerRemaining = null;
    this.sleepTimerSetMinutes = null;
  }

  /** Persist the lazy context (localStorage: keeps the src-tauri config
   * schema untouched). Ids only, so this stays cheap even for huge lists. */
  persistContext(): void {
    try {
      const ctx = this.playbackContext;
      if (!ctx) {
        localStorage.removeItem(CTX_STORAGE_KEY);
        return;
      }
      localStorage.setItem(
        CTX_STORAGE_KEY,
        JSON.stringify({
          playlistId: ctx.playlistId,
          playlistName: ctx.playlistName,
          order: ctx.order,
          index: ctx.index,
        }),
      );
    } catch {
      /* storage unavailable (private mode etc.) */
    }
  }

  /** Restore a persisted context on launch, re-anchored to the restored
   * queue's current track. Dropped when the queue no longer matches. */
  restorePersistedContext(): void {
    try {
      const raw = localStorage.getItem(CTX_STORAGE_KEY);
      if (!raw) return;
      const saved = JSON.parse(raw) as {
        playlistId: number;
        playlistName: string;
        order: string[];
        index: number;
      };
      if (!saved?.order?.length) return;
      const curId = this.queue[this.queueIndex]?.id ?? null;
      const ci = curId ? saved.order.indexOf(curId) : -1;
      if (ci === -1) {
        localStorage.removeItem(CTX_STORAGE_KEY);
        return;
      }
      this.playbackContext = {
        playlistId: saved.playlistId,
        playlistName: saved.playlistName,
        order: saved.order,
        index: ci,
        suspended: false,
      };
      this.ensureNext();
    } catch {
      /* corrupt payload - ignore */
    }
  }
}

function shuffleIds(ids: string[]): void {
  for (let i = ids.length - 1; i > 0; i--) {
    const j = Math.floor(Math.random() * (i + 1));
    [ids[i], ids[j]] = [ids[j], ids[i]];
  }
}

function formatTime(secs: number): string {
  if (!secs || secs < 0) return "0:00";
  const m = Math.floor(secs / 60);
  const s = Math.floor(secs % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}

export const player = new PlayerState();

// Persist the context whenever any part of it changes (index moves on every
// advance, order changes on shuffle/removals). Same $effect.root pattern as
// config.svelte.ts.
$effect.root(() => {
  $effect(() => {
    const ctx = $state.snapshot(player.playbackContext);
    void ctx;
    player.persistContext();
  });
});
