import { invoke } from "@tauri-apps/api/core";
import { player } from "./player.svelte";
import { setVolume, setEqEnabled, setEqGains } from "../ipc/bridge";

export interface AppConfig {
  volume: number;
  eq_enabled: boolean;
  eq_gains: number[];
}

const defaults: AppConfig = {
  volume: 0.8,
  eq_enabled: false,
  eq_gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
};

class ConfigState {
  current = $state<AppConfig>({ ...defaults });
  loaded = $state(false);

  async load() {
    try {
      this.current = await invoke<AppConfig>("get_config");
      
      // Sync into player state
      player.volume = this.current.volume;
      player.eqEnabled = this.current.eq_enabled;
      player.eqGains = [...this.current.eq_gains];
      
      // Explicitly tell backend to sync (in case engine started with defaults)
      await setVolume(player.volume);
      await setEqEnabled(player.eqEnabled);
      await setEqGains(player.eqGains);

      this.loaded = true;
    } catch {
      this.current = { ...defaults };
    }
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

// Global effect to watch player state and sync to config
$effect.root(() => {
  $effect(() => {
    if (config.loaded) {
      const volume = player.volume;
      const eq_enabled = player.eqEnabled;
      const eq_gains = $state.snapshot(player.eqGains);
      
      // Batch update to avoid multiple saves
      config.update({ volume, eq_enabled, eq_gains });
    }
  });
});
