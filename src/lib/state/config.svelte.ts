import { invoke } from "@tauri-apps/api/core";

export interface AppConfig {
  volume: number;
  eq_enabled: boolean;
  eq_gains: number[];
  eq_preset: string;
  lyrics_auto_fetch: boolean;
  search_limit: number;
  audio_quality: number;
  seek_step_secs: number;
  volume_step: number;
  subtitle_lang: string;
  prefetch_ahead: number;
}

const defaults: AppConfig = {
  volume: 0.8,
  eq_enabled: false,
  eq_gains: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  eq_preset: "Flat",
  lyrics_auto_fetch: false,
  search_limit: 10,
  audio_quality: 2,
  seek_step_secs: 5,
  volume_step: 0.05,
  subtitle_lang: "en",
  prefetch_ahead: 2,
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
      await invoke("set_config", { newConfig: this.current });
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
