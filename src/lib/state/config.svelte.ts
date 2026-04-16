import { invoke } from "@tauri-apps/api/core";
import { player } from "./player.svelte";
import { setVolume, setEqEnabled, setEqGains } from "../ipc/bridge";

export interface AppConfig {
  volume: number;
  eq_enabled: boolean;
  eq_gains: number[];
  notifications_enabled: boolean;
  discord_rpc_enabled: boolean;
  saved_queue: string[];
  saved_queue_index: number;
}

const defaults: AppConfig = {
  volume: 0.8,
  eq_enabled: false,
  eq_gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  notifications_enabled: true,
  discord_rpc_enabled: false,
  saved_queue: [],
  saved_queue_index: -1,
};

class ConfigState {
  current = $state<AppConfig>({ ...defaults });
  loaded = $state(false);

  async load() {
    try {
      this.current = await invoke<AppConfig>("get_config");
    } catch {
      this.current = { ...defaults };
    }

    // Sync into player state
    player.volume = this.current.volume;
    player.eqEnabled = this.current.eq_enabled;
    player.eqGains = [...this.current.eq_gains];

    // Best-effort backend sync (engine may have started with defaults)
    try {
      await setVolume(player.volume);
      await setEqEnabled(player.eqEnabled);
      await setEqGains(player.eqGains);
    } catch (e) {
      console.error("Failed to sync config to backend:", e);
    }

    this.loaded = true;
  }

  async save() {
    try {
      await invoke("set_config", { config: $state.snapshot(this.current) });
    } catch (e) {
      console.error("Failed to save config:", e);
    }
  }

  async update(partial: Partial<AppConfig>) {
    Object.assign(this.current, partial);
    await this.save();
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

    clearTimeout(saveTimer);
    saveTimer = setTimeout(() => {
      config.update({ volume, eq_enabled, eq_gains, saved_queue, saved_queue_index });
    }, 300);
  });
});
