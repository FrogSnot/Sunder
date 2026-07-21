export interface ToastAction {
  label: string;
  onClick: () => void;
}

export interface Toast {
  id: number;
  message: string;
  type: "error" | "info";
  persistent?: boolean;
  action?: ToastAction;
}

class ToastState {
  toasts = $state<Toast[]>([]);
  private nextId = 0;

  add(
    message: string,
    type: "error" | "info" = "info",
    durationOrOptions: number | { durationMs?: number; persistent?: boolean; action?: ToastAction } = 5000,
  ): number {
    const id = this.nextId++;
    // Backwards compatibility: existing call sites pass a number (durationMs).
    // New persistent+action call sites pass an options object.
    let durationMs: number;
    let persistent: boolean;
    let action: ToastAction | undefined;
    if (typeof durationOrOptions === "number") {
      durationMs = durationOrOptions;
      persistent = false;
      action = undefined;
    } else {
      durationMs = durationOrOptions.durationMs ?? 5000;
      persistent = durationOrOptions.persistent ?? false;
      action = durationOrOptions.action;
    }
    this.toasts = [...this.toasts, { id, message, type, persistent, action }];
    // Auto-dismiss non-persistent toasts after durationMs. Persistent toasts
    // stay until explicitly removed (e.g. on audio-device-restored event).
    if (!persistent) {
      setTimeout(() => {
        this.remove(id);
      }, durationMs);
    }
    return id;
  }

  remove(id: number) {
    this.toasts = this.toasts.filter(t => t.id !== id);
  }
}

export const toastState = new ToastState();
