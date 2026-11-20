import { invoke } from "@tauri-apps/api/core";
import { player } from "./player.svelte";
import { restoreQueue, setVolume, setEqEnabled, setEqGains, setSpeed, setRepeatMode } from "../ipc/bridge";
import { toastState } from "./toast.svelte";

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
  private queueRestoreFailed = false;

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

    // Start autosaving after the saved queue and its index have been restored.
    try {
      await restoreQueue();
    } catch (e) {
      this.queueRestoreFailed = true;
      console.error("Failed to restore queue:", e);
      toastState.add("Could not restore the saved queue", "error");
    }
    this.loaded = true;
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
    // Keep the on-disk queue if restoration failed and no new queue was started.
    if (this.queueRestoreFailed) {
      if (partial.saved_queue?.length) {
        this.queueRestoreFailed = false;
      } else {
        delete partial.saved_queue;
        delete partial.saved_queue_index;
      }
    }
    this.enqueueSave(() => partial);
  }
}

export const config = new ConfigState();

// Debounced watcher: persist player state changes with 300ms delay
let saveTimer: ReturnType<typeof setTimeout> | undefined;

$effect.root(() => {
  $effect(() => {
    if (!config.loaded) return;
    const volume = player.volume;
    const eq_enabled = player.eqEnabled;
    const eq_gains = $state.snapshot(player.eqGains);
    const saved_queue = player.queue.map(t => t.id);
    const saved_queue_index = player.queueIndex;
    const repeat_mode = player.repeatMode;
    const playback_speed = player.speed;

    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      config.update({ volume, eq_enabled, eq_gains, saved_queue, saved_queue_index, repeat_mode, playback_speed });
    }, 300);
  });
});
