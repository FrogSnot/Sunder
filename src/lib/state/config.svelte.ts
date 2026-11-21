import { invoke } from "@tauri-apps/api/core";
import { player } from "./player.svelte";
import { restoreQueue, setVolume, setEqEnabled, setEqGains, setSpeed, setRepeatMode } from "../ipc/bridge";
import { toastState } from "./toast.svelte";

import {
  decideQueuePatch,
  idlePreserveState,
  isPartialRestore,
  preserveStateFor,
  type QueuePreserveState,
} from "../util/queuePreserve";

/**
 * Upper bound on how long load() waits for the queue restore before it lets
 * autosave start anyway.
 *
 * Restoring the queue is one indexed DB read and normally answers in tens of
 * milliseconds. If it does not, the backend is wedged or the DB mutex is
 * blocked, and nothing will ever complete that call. Autosave is gated on
 * `loaded`, so without this bound a hung restore would silently disable every
 * setting - volume, EQ, queue - for the rest of the session, and nothing would
 * throw. Three seconds is far above any legitimate restore yet short enough
 * that saving is not meaningfully delayed.
 */
const QUEUE_RESTORE_TIMEOUT_MS = 3000;

export interface AppConfig {
  volume: number;
  eq_enabled: boolean;
  eq_gains: number[];
  notifications_enabled: boolean;
  discord_rpc_enabled: boolean;
  saved_queue: string[];
  saved_queue_index: number;
  repeat_mode: "off" | "queue" | "track";
  playback_speed: number;
}

const defaults: AppConfig = {
  volume: 0.8,
  eq_enabled: false,
  eq_gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  notifications_enabled: true,
  discord_rpc_enabled: false,
  saved_queue: [],
  saved_queue_index: -1,
  repeat_mode: "off",
  playback_speed: 1.0,
};

class ConfigState {
  current = $state<AppConfig>({ ...defaults });
  loaded = $state(false);
  private pendingSave = Promise.resolve();
  /** While active, the saved queue on disk is kept instead of overwritten. */
  private queuePreserve: QueuePreserveState = idlePreserveState;

  async load() {
    this.pendingSave = this.pendingSave.then(async () => {
      try {
        this.current = await invoke<AppConfig>("get_config");
      } catch {
        this.current = { ...defaults };
      }
    });
    await this.pendingSave;

    // Capture saved values before any async work so progress events
    // can't overwrite them via updateFromProgress().
    const savedVolume = this.current.volume;
    const savedEqEnabled = this.current.eq_enabled;
    const savedEqGains = [...this.current.eq_gains];
    const savedRepeatMode = this.current.repeat_mode;
    const savedSpeed = this.current.playback_speed;

    // Sync into player state
    player.volume = savedVolume;
    player.eqEnabled = savedEqEnabled;
    player.eqGains = savedEqGains;
    player.repeatMode = savedRepeatMode;
    player.speed = savedSpeed;

    // Best-effort backend sync (engine may have started with defaults).
    // Set speed and repeat first so subsequent state emissions from
    // setVolume/setEq carry the correct speed value.
    try {
      await setSpeed(savedSpeed);
      await setRepeatMode(savedRepeatMode);
      await setVolume(savedVolume);
      await setEqEnabled(savedEqEnabled);
      await setEqGains(savedEqGains);
    } catch (e) {
      console.error("Failed to sync config to backend:", e);
    }

    // Start autosaving after the saved queue and its index have been restored,
    // but never wait forever: see QUEUE_RESTORE_TIMEOUT_MS.
    // Held in a variable because the timeout below only stops WAITING for it;
    // the restore itself keeps running and may still settle later.
    const restoreAttempt = restoreQueue();
    this.queuePreserve = idlePreserveState;
    try {
      const outcome = await Promise.race([
        restoreAttempt,
        new Promise<"timeout">((resolve) => {
          setTimeout(() => resolve("timeout"), QUEUE_RESTORE_TIMEOUT_MS);
        }),
      ]);
      if (outcome === "timeout") {
        // Treat a wedged restore exactly like a failed one so the saved queue
        // on disk is preserved instead of being overwritten by an empty one.
        this.queuePreserve = preserveStateFor([]);
        console.error(
          `Queue restore did not finish in ${QUEUE_RESTORE_TIMEOUT_MS}ms; keeping the saved queue`,
        );
        toastState.add("Could not restore the saved queue", "error");
        // The race gave up, but restoreQueue() is still in flight. If it later
        // lands as a PARTIAL restore it installs the surviving subset as the
        // queue, and the debounced save would then see a queue that no longer
        // equals [] - ending preservation and overwriting the full saved list
        // with the shortened one. Re-arm preservation when it settles: this
        // continuation runs in the microtask right after the restore resolves,
        // ahead of the 300ms debounce it would otherwise race.
        void restoreAttempt
          .then((late) => {
            if (isPartialRestore(late.requested, late.restored)) {
              this.queuePreserve = preserveStateFor(player.queue.map((t) => t.id));
              console.warn(
                `Late restore brought back ${late.restored} of ${late.requested} saved tracks; keeping the saved queue`,
              );
            }
          })
          .catch(() => {
            // Already reported above; a late failure must not go unhandled.
          });
      } else if (isPartialRestore(outcome.requested, outcome.restored)) {
        // Some saved tracks are no longer in the local library. Autosave would
        // otherwise persist the shorter list and lose them permanently.
        this.queuePreserve = preserveStateFor(player.queue.map((t) => t.id));
        // Logged rather than surfaced as a toast: this can happen on every
        // startup and would be noise, and the saved queue is kept intact.
        console.warn(
          `Restored ${outcome.restored} of ${outcome.requested} saved tracks; keeping the saved queue until the queue changes`,
        );
      }
    } catch (e) {
      this.queuePreserve = preserveStateFor([]);
      console.error("Failed to restore queue:", e);
      toastState.add("Could not restore the saved queue", "error");
    } finally {
      // Must run even when the restore above hangs or throws: autosave is
      // gated on this flag, so leaving it false would disable every setting.
      this.loaded = true;
    }
  }

  private enqueueSave(settings: () => Partial<AppConfig>) {
    // Compare against the last successful save after earlier requests settle.
    this.pendingSave = this.pendingSave.then(async () => {
      const patch = Object.fromEntries(
        Object.entries(settings()).filter(([key, value]) =>
          JSON.stringify(value) !== JSON.stringify(this.current[key as keyof AppConfig]),
        ),
      );
      if (Object.keys(patch).length === 0) return;

      const track = patch.discord_rpc_enabled && player.isPlaying ? player.currentTrack : null;
      const warning = await invoke<string | null>("set_config", { patch, track: $state.snapshot(track) });
      Object.assign(this.current, patch);
      if (warning) {
        console.warn("Settings directory sync failed:", warning);
        toastState.add("Settings applied, but may be lost after a system crash", "error");
      }
    }).catch((e) => {
      console.error("Failed to save config:", e);
      toastState.add("Could not save settings", "error");
    });
  }

  toggle(key: "notifications_enabled" | "discord_rpc_enabled") {
    this.enqueueSave(() => ({ [key]: !this.current[key] }));
  }

  update(partial: Partial<AppConfig>) {
    const next = { ...partial };
    if (next.saved_queue !== undefined) {
      // Preservation ends as soon as the queue differs from the restored
      // subset, which covers starting a new queue, reordering, and clearing.
      const decision = decideQueuePatch(this.queuePreserve, next.saved_queue);
      this.queuePreserve = decision.state;
      if (decision.suppress) {
        delete next.saved_queue;
        delete next.saved_queue_index;
      }
    }
    this.enqueueSave(() => next);
  }

  /**
   * Persist the current player state immediately instead of waiting out the
   * 300ms debounce. Returns the promise for the whole queued save chain so a
   * caller (the window close handler) can wait for it to settle.
   */
  flush(): Promise<void> {
    clearTimeout(saveTimer);
    if (!this.loaded) return this.pendingSave;
    this.update(playerPatch());
    return this.pendingSave;
  }
}

export const config = new ConfigState();

// Debounced watcher: persist player state changes with 300ms delay
let saveTimer: ReturnType<typeof setTimeout> | undefined;

/**
 * The player-owned slice of AppConfig. Read under the current effect / call so
 * both the debounced watcher and flush() persist exactly the same fields.
 */
function playerPatch(): Partial<AppConfig> {
  return {
    volume: player.volume,
    eq_enabled: player.eqEnabled,
    eq_gains: $state.snapshot(player.eqGains),
    saved_queue: player.queue.map((t) => t.id),
    saved_queue_index: player.queueIndex,
    repeat_mode: player.repeatMode,
    playback_speed: player.speed,
  };
}

$effect.root(() => {
  $effect(() => {
    if (!config.loaded) return;
    const patch = playerPatch();

    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      config.update(patch);
    }, 300);
  });
});
