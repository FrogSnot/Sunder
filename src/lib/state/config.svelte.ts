import { invoke } from "@tauri-apps/api/core";

export interface AppConfig {
  volume: number;
  eq_enabled: boolean;
  eq_gains: number[];
  notifications_enabled: boolean;
}

const defaults: AppConfig = {
  volume: 0.8,
  eq_enabled: false,
  eq_gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  notifications_enabled: true,
};

class ConfigState {
  current = $state<AppConfig>({ ...defaults });
  loaded = $state(false);

  async load() {
    try {
      this.current = await invoke<AppConfig>("get_config");
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
