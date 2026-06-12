export interface DragReorderItem {
  id: string;
  title: string;
  artist: string;
  thumbnail?: string;
}

export interface DragReorderOptions {
  getList: () => DragReorderItem[];
  onReorder: (from: number, to: number) => void;
  getScrollContainer?: () => HTMLElement | null;
  rowSelector?: string;
  longPressMs?: number;
  dragThreshold?: number;
  edgeSize?: number;
  maxScrollSpeed?: number;
  disabled?: () => boolean;
}

type ResolvedOptions = {
  getList: () => DragReorderItem[];
  onReorder: (from: number, to: number) => void;
  getScrollContainer: () => HTMLElement | null;
  rowSelector: string;
  longPressMs: number;
  dragThreshold: number;
  edgeSize: number;
  maxScrollSpeed: number;
  disabled: () => boolean;
};

const DEFAULTS: Omit<ResolvedOptions, "getList" | "onReorder"> = {
  getScrollContainer: () => null,
  rowSelector: ".drag-row",
  longPressMs: 220,
  dragThreshold: 5,
  edgeSize: 64,
  maxScrollSpeed: 12,
  disabled: () => false,
};

export class DragReorder {
  dragging = $state(false);
  dragFrom = $state(-1);
  dragOver = $state(-1);
  dropPosition = $state<"before" | "after">("before");

  private _opts: ResolvedOptions;
  private _pressTimer: ReturnType<typeof setTimeout> | null = null;
  private _pressIndex = -1;
  private _pressStartX = 0;
  private _pressStartY = 0;
  private _pressActive = false;
  private _rafId: number | null = null;
  private _x = 0;
  private _y = 0;
  private _scrollContainer: HTMLElement | null = null;
  private _ghostEl: HTMLElement | null = null;
  private _onMove: (e: PointerEvent) => void;
  private _onUp: (e: PointerEvent) => void;
  private _onKey: (e: KeyboardEvent) => void;
  private _onWindowBlur: () => void;
  private _onResize: () => void;

  constructor(options: DragReorderOptions) {
    this._opts = { ...DEFAULTS, ...options };
    this._onMove = this._handleMove.bind(this);
    this._onUp = this._handleUp.bind(this);
    this._onKey = this._handleKey.bind(this);
    this._onWindowBlur = () => this._endDrag();
    this._onResize = () => this._updateGhost();
  }

  attachGhost(el: HTMLElement | null) {
    if (this._ghostEl && this._ghostEl !== el) {
      this._ghostEl.style.transform = "";
    }
    this._ghostEl = el;
    if (el) this._updateGhost();
  }

  onPointerDown(e: PointerEvent, index: number) {
    if (this._opts.disabled()) return;
    if (e.button !== 0 && e.pointerType === "mouse") return;
    const target = e.target as HTMLElement;
    if (!target) return;
    if (target.closest(".drag-skip")) return;
    if (target.closest("button") && !target.closest(".drag-handle")) return;

    this._pressIndex = index;
    this._pressStartX = e.clientX;
    this._pressStartY = e.clientY;
    this._pressActive = true;

    if (this._pressTimer) clearTimeout(this._pressTimer);
    this._pressTimer = setTimeout(() => {
      if (this._pressActive && this._pressIndex === index) {
        this._startDrag(e, index);
      }
    }, this._opts.longPressMs);
  }

  onPointerCancel() {
    if (!this.dragging && this._pressActive) {
      this._pressActive = false;
      if (this._pressTimer) {
        clearTimeout(this._pressTimer);
        this._pressTimer = null;
      }
    }
  }

  getGhostItem(): DragReorderItem | null {
    if (!this.dragging || this.dragFrom < 0) return null;
    return this._opts.getList()[this.dragFrom] ?? null;
  }

  dropClassFor(idx: number): string {
    if (!this.dragging || this.dragFrom === idx || this.dragOver !== idx) return "";
    return this.dropPosition === "before" ? "drop-before" : "drop-after";
  }

  isDragging(idx: number): boolean {
    return this.dragging && this.dragFrom === idx;
  }

  justDragged(): boolean {
    return this._justDraggedUntil > performance.now();
  }

  private _justDraggedUntil = 0;

  destroy() {
    this._endDrag();
    this._pressActive = false;
    if (this._pressTimer) clearTimeout(this._pressTimer);
    this._pressTimer = null;
  }

  private _startDrag(e: PointerEvent, index: number) {
    if (this.dragging) return;
    this.dragging = true;
    this.dragFrom = index;
    this.dragOver = index;
    this.dropPosition = "before";
    this._x = e.clientX;
    this._y = e.clientY;
    this._scrollContainer = this._opts.getScrollContainer() ?? null;

    document.body.style.touchAction = "none";
    document.body.style.userSelect = "none";
    document.body.style.cursor = "grabbing";
    document.body.classList.add("is-dragging");

    document.addEventListener("pointermove", this._onMove, { passive: true });
    document.addEventListener("pointerup", this._onUp);
    document.addEventListener("pointercancel", this._onUp);
    document.addEventListener("keydown", this._onKey);
    window.addEventListener("blur", this._onWindowBlur);
    window.addEventListener("resize", this._onResize);

    this._updateGhost();
    this._scheduleFrame();
  }

  private _handleMove(e: PointerEvent) {
    if (this._pressActive && !this.dragging) {
      const dx = e.clientX - this._pressStartX;
      const dy = e.clientY - this._pressStartY;
      if (Math.hypot(dx, dy) > this._opts.dragThreshold) {
        if (this._pressTimer) {
          clearTimeout(this._pressTimer);
          this._pressTimer = null;
        }
        this._startDrag(e, this._pressIndex);
      }
      return;
    }
    if (!this.dragging) return;

    this._x = e.clientX;
    this._y = e.clientY;
    this._scheduleFrame();
  }

  private _scheduleFrame() {
    if (this._rafId !== null) return;
    this._rafId = requestAnimationFrame(() => {
      this._rafId = null;
      if (!this.dragging) return;
      this._updateGhost();
      this._updateHover();
      this._autoScroll();
    });
  }

  private _updateGhost() {
    if (!this._ghostEl) return;
    this._ghostEl.style.transform = `translate3d(${this._x + 14}px, ${this._y + 14}px, 0)`;
  }

  private _updateHover() {
    const selector = this._opts.rowSelector;
    const target = document.elementFromPoint(this._x, this._y);
    if (!target) return;
    const row = target.closest(selector) as HTMLElement | null;
    if (!row || row.dataset.idx === undefined) {
      this._autoScrollContinue();
      return;
    }
    const idx = parseInt(row.dataset.idx, 10);
    if (Number.isNaN(idx)) return;
    this.dragOver = idx;
    const rect = row.getBoundingClientRect();
    const midY = rect.top + rect.height / 2;
    this.dropPosition = this._y < midY ? "before" : "after";
    this._autoScrollContinue();
  }

  private _autoScroll() {
    const container = this._scrollContainer;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const y = this._y;
    const edge = this._opts.edgeSize;
    if (y < rect.top + edge) {
      const intensity = 1 - Math.min(1, Math.max(0, (y - rect.top) / edge));
      container.scrollTop -= this._opts.maxScrollSpeed * intensity;
    } else if (y > rect.bottom - edge) {
      const intensity = 1 - Math.min(1, Math.max(0, (rect.bottom - y) / edge));
      container.scrollTop += this._opts.maxScrollSpeed * intensity;
    }
  }

  private _autoScrollContinue() {
    const container = this._scrollContainer;
    if (!container) return;
    const rect = container.getBoundingClientRect();
    const y = this._y;
    const edge = this._opts.edgeSize;
    if (y >= rect.top + edge && y <= rect.bottom - edge) return;
    this._scheduleFrame();
  }

  private _handleUp(_e: PointerEvent) {
    const wasDragging = this.dragging;
    if (wasDragging) {
      const list = this._opts.getList();
      const from = this.dragFrom;
      const to = this.dragOver;
      if (from >= 0 && to >= 0 && from < list.length && to < list.length) {
        let target = this.dropPosition === "after" ? to + 1 : to;
        if (from < target) target -= 1;
        if (target !== from && target >= 0 && target < list.length) {
          this._opts.onReorder(from, target);
          this._justDraggedUntil = performance.now() + 80;
        }
      }
    }
    this._endDrag();
  }

  private _handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      this._endDrag();
    }
  }

  private _endDrag() {
    this.dragging = false;
    this.dragFrom = -1;
    this.dragOver = -1;

    if (this._pressTimer) {
      clearTimeout(this._pressTimer);
      this._pressTimer = null;
    }
    if (this._rafId !== null) {
      cancelAnimationFrame(this._rafId);
      this._rafId = null;
    }
    this._pressActive = false;
    this._pressIndex = -1;

    document.removeEventListener("pointermove", this._onMove);
    document.removeEventListener("pointerup", this._onUp);
    document.removeEventListener("pointercancel", this._onUp);
    document.removeEventListener("keydown", this._onKey);
    window.removeEventListener("blur", this._onWindowBlur);
    window.removeEventListener("resize", this._onResize);

    document.body.style.touchAction = "";
    document.body.style.userSelect = "";
    document.body.style.cursor = "";
    document.body.classList.remove("is-dragging");
  }
}
