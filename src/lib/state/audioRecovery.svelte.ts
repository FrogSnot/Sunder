import { retryAudioDevice } from "../ipc/bridge";
import { toastState } from "./toast.svelte";
import { listen } from "@tauri-apps/api/event";

class AudioRecoveryState {
  private lostToastId: number | null = null;
  private listenerAttached = false;

  async init() {
    if (this.listenerAttached) return;

    let unlistenLost: (() => void) | null = null;
    let unlistenRestored: (() => void) | null = null;

    try {
      unlistenLost = await listen<{ device_name: string | null; error?: string }>(
        "audio-device-lost",
        () => {
          if (this.lostToastId !== null) {
            toastState.remove(this.lostToastId);
            this.lostToastId = null;
          }
          this.lostToastId = toastState.add(
            "Audio device unavailable",
            "error",
            {
              persistent: true,
              action: {
                label: "Retry",
                onClick: () => {
                  retryAudioDevice().catch((e) =>
                    console.error("retryAudioDevice failed:", e),
                  );
                },
              },
            },
          );
        },
      );
      unlistenRestored = await listen("audio-device-restored", () => {
        if (this.lostToastId !== null) {
          toastState.remove(this.lostToastId);
          this.lostToastId = null;
        }
        toastState.add("Audio device restored", "info", { durationMs: 3000 });
      });
      this.listenerAttached = true;
    } catch (e) {
      unlistenRestored?.();
      unlistenLost?.();
      console.error("audioRecovery.init failed:", e);
    }
  }
}

export const audioRecovery = new AudioRecoveryState();
