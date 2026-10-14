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

const DEFAULTS = {
  getScrollContainer: () => null as HTMLElement | null,
  rowSelector: ".drag-row",
  longPressMs: 220,
  dragThreshold: 6,
  edgeSize: 64,
  maxScrollSpeed: 14,
  disabled: () => false,
};

const SETTLE_MS = 220;
const FALLBACK_STEP = 58;

/**
 * Drag-to-reorder engine.
 *
 * Design notes:
 * - Rows are never re-ordered in the DOM during a drag. Every row gets an
 *   inline translateY from a slot model, so the list "makes a gap" that
 *   follows the pointer live. The reorder commits once, on drop.
 * - Geometry is measured once at drag start (restY per row, in scroll-content
 *   coordinates) so scrolling never invalidates it. Rows that mount later
 *   (virtualization) are absorbed on scroll and shifted in the same tick.
 * - Ranks are DOM (visual) order, NOT list-index order. The queue view renders
 *   played rows (low indices) BELOW up-next rows (high indices), so index
 *   arithmetic must never stand in for spatial ranks. A rank map translates
 *   row indices to ranks everywhere.
 * - Hit testing uses REST geometry only: K is a pure function of pointer Y, so
 *   there is no hysteresis band and the pointer can never get stuck between
 *   two slots. K is recomputed synchronously on drop because the last
 *   pointermove's rAF may not have run.
 * - pointercancel / blur / Escape revert instead of committing.
 * - Keyboard: focus a drag handle, Enter/Space lifts, arrows move the gap,
 *   Enter/Space drops, Escape cancels. Ctrl/Cmd+Arrow reorders directly.
 */
export class DragReorder {
  dragging = $state(false);
  dragFrom = $state(-1);
  dragOver = $state(-1);
  dropPosition = $state<"before" | "after">("after");
  /**
   * 1-based position the dragged item WILL land at if dropped right now -
   * the same commit math that feeds onReorder's `to`. -1 while not dragging.
   * Updates every frame the target slot changes (pointer or keyboard).
   */
  targetPosition = $state(-1);

  private _opts: DragReorderOptions & typeof DEFAULTS;
  private _mode: "pointer" | "key" = "pointer";
  private _container: HTMLElement | null = null;
  private _fullLen = 0;
  /** Row indices in visual (DOM) order. */
  private _list: number[] = [];
  /** index -> rank into _list. */
  private _rank = new Map<number, number>();
  private _restY = new Map<number, number>();
  private _heights = new Map<number, number>();
  private _dragRank = -1;
  /**
   * First rank of the second DOM run (e.g. the played section in the queue
   * view, where DOM order is a rotation of array order). Equal to
   * _list.length when DOM order is strictly increasing (search/downloads/
   * playlists). Drives the cross-section commit rule.
   */
  private _breakRank = Number.MAX_SAFE_INTEGER;
  private _K = 0;
  private _stepX = FALLBACK_STEP;
  private _x = 0;
  private _y = 0;
  private _pointerId = -1;
  private _rafId: number | null = null;
  private _ghostEl: HTMLElement | null = null;
  private _justDraggedUntil = 0;
  private _settling = false;
  private _settleTimer: ReturnType<typeof setTimeout> | null = null;
  private _capturedEl: Element | null = null;

  private _pressTimer: ReturnType<typeof setTimeout> | null = null;
  private _pressIndex = -1;
  private _pressEl: Element | null = null;
  private _pressStartX = 0;
  private _pressStartY = 0;
  private _pressActive = false;
  private _pressTouch = false;

  private _onMove = (e: PointerEvent) => this._handleMove(e);
  private _onUp = (e: PointerEvent) => this._handleUp(e);
  private _onKey = (e: KeyboardEvent) => this._handleKey(e);
  private _onScroll = () => this._handleScroll();
  private _onTouchMove = (e: TouchEvent) => {
    if (this.dragging && this._mode === "pointer") e.preventDefault();
  };
  private _onBlur = () => {
    if (this.dragging) this._revert();
  };
  private _onContextMenu = (e: Event) => {
    if (this.busy()) e.preventDefault();
  };
  private _onResize = () => {
    if (!this.dragging) return;
    this._updateGhost();
    this._scheduleFrame();
  };
  private _onLostCapture = () => {
    if (this.dragging && this._mode === "pointer") this._revert();
  };

  constructor(options: DragReorderOptions) {
    this._opts = { ...DEFAULTS, ...options };
  }

  // ---------------------------------------------------------------- public API

  attachGhost(el: HTMLElement | null) {
    if (this._ghostEl && this._ghostEl !== el) {
      this._ghostEl.style.transform = "";
    }
    this._ghostEl = el;
    if (el && this.dragging && this._mode === "pointer") this._updateGhost();
  }

  getGhostItem(): DragReorderItem | null {
    if (!this.dragging || this._mode !== "pointer" || this.dragFrom < 0) return null;
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

  /** True while a drag is up or its settle animation is in flight. */
  busy(): boolean {
    return this.dragging || this._settling;
  }

  destroy() {
    this._teardownListeners();
    this._pressActive = false;
    this._clearPressTimer();
    if (this._rafId !== null) {
      cancelAnimationFrame(this._rafId);
      this._rafId = null;
    }
    if (this._settleTimer !== null) {
      clearTimeout(this._settleTimer);
      this._settleTimer = null;
    }
    this._clearTransforms();
    this._restoreBody();
    this.dragging = false;
    this.dragFrom = -1;
    this.dragOver = -1;
    this.targetPosition = -1;
  }

  onPointerDown(e: PointerEvent, index: number) {
    if (this._opts.disabled() || this.dragging || this._settling) return;
    if (e.button !== 0 && e.pointerType === "mouse") return;
    const target = e.target as HTMLElement | null;
    if (!target) return;
    if (target.closest(".drag-skip")) return;
    if (target.closest("button") && !target.closest(".drag-handle")) return;

    this._pressIndex = index;
    this._pressEl = e.currentTarget as Element | null;
    this._pressStartX = e.clientX;
    this._pressStartY = e.clientY;
    this._pressActive = true;
    this._pressTouch = e.pointerType === "touch";

    this._clearPressTimer();
    this._pressTimer = setTimeout(() => {
      this._pressTimer = null;
      if (this._pressActive && this._pressIndex === index && !this.dragging) {
        this._startPointerDrag(e, index);
      }
    }, this._opts.longPressMs);
  }

  onPointerCancel() {
    // pointercancel means the OS/browser took the gesture (native scroll, app
    // switch). Never commit in that case; just reset the press.
    this._pressActive = false;
    this._clearPressTimer();
    if (this.dragging && this._mode === "pointer") this._revert();
  }

  /**
   * Keydown handler for a drag handle. Returns a short status message for
   * screen readers, or null when the key was not handled.
   */
  onHandleKeydown(e: KeyboardEvent, index: number): string | null {
    if (this._opts.disabled() || this._settling) return null;

    // Active keyboard drag: focus never leaves the lifted handle, so IT
    // keeps routing keys (arrows move, Enter/Space/Tab drop, Escape
    // cancels). Without this the row lifts and is then stuck - the
    // document-level key listener only exists for pointer drags.
    if (this.dragging) {
      if (this._mode !== "key") return null;
      return this._keyHandle(e);
    }

    const key = e.key;

    if ((e.ctrlKey || e.metaKey) && !e.altKey && (key === "ArrowUp" || key === "ArrowDown")) {
      const list = this._opts.getList();
      const to = key === "ArrowUp" ? index - 1 : index + 1;
      e.preventDefault();
      if (to < 0 || to >= list.length || to === index) return null;
      const title = this._titleAt(index);
      this._opts.onReorder(index, to);
      this._justDraggedUntil = performance.now() + 250;
      // Svelte re-keys the row, so the focused handle node is replaced.
      // Restore focus to the moved track's handle (it now sits at `to`) so
      // repeat presses keep walking the same track.
      this._refocusHandle(to);
      return `Moved ${title} to position ${to + 1} of ${list.length}`;
    }

    if (key === "Enter" || key === " ") {
      e.preventDefault();
      this.dragFrom = index;
      this._mode = "key";
      if (!this._beginDrag()) {
        this.dragFrom = -1;
        return null;
      }
      const n = this._list.length;
      return `Lifted ${this._titleAt(index)}. Position ${this._K + 1} of ${n}. Use arrow keys to move, Enter to drop, Escape to cancel.`;
    }
    return null;
  }

  // ---------------------------------------------------------------- drag start

  private _startPointerDrag(e: PointerEvent, index: number) {
    this._mode = "pointer";
    this.dragFrom = index;
    this._x = e.clientX;
    this._y = e.clientY;
    this._pointerId = e.pointerId;
    if (!this._beginDrag()) {
      this.dragFrom = -1;
      return;
    }
    this._pressActive = false;

    const row = this._pressEl;
    if (row && typeof (row as HTMLElement).setPointerCapture === "function") {
      try {
        (row as HTMLElement).setPointerCapture(e.pointerId);
        this._capturedEl = row;
        row.addEventListener("lostpointercapture", this._onLostCapture);
      } catch {
        this._capturedEl = null;
      }
    }

    document.addEventListener("pointermove", this._onMove, { passive: true });
    document.addEventListener("pointerup", this._onUp);
    document.addEventListener("pointercancel", this._onUp);
    document.addEventListener("touchmove", this._onTouchMove, { passive: false });
    document.addEventListener("keydown", this._onKey);

    this._updateGhost();
    this._scheduleFrame();
  }

  /**
   * Shared begin: measure the rows, prime the slot model, lock the body.
   * Returns false when there is nothing to drag.
   */
  private _beginDrag(): boolean {
    const container = this._opts.getScrollContainer();
    this._container = container;
    this._fullLen = this._opts.getList().length;
    if (!this._measure()) {
      this.dragging = false;
      return false;
    }

    this.dragging = true;
    this._K = this._dragRank;
    this.dragOver = -1;
    this.dropPosition = "after";

    document.body.style.touchAction = "none";
    document.body.style.userSelect = "none";
    document.body.style.cursor = this._mode === "pointer" ? "grabbing" : "";
    document.body.classList.add("is-dragging");

    document.addEventListener("contextmenu", this._onContextMenu);
    window.addEventListener("blur", this._onBlur);
    window.addEventListener("resize", this._onResize);

    if (container) container.addEventListener("scroll", this._onScroll, { passive: true });

    this._applyTransforms();
    this._syncIndicator();
    return true;
  }

  /** Measure rest positions of every mounted drag row, in DOM order. */
  private _measure(): boolean {
    const container = this._container;
    const root: ParentNode = container ?? document;
    const els = Array.from(root.querySelectorAll<HTMLElement>(this._opts.rowSelector));
    if (els.length < 2) return false;

    const cRect = container ? container.getBoundingClientRect() : { top: 0 };
    const scrollTop = container ? container.scrollTop : 0;

    this._restY.clear();
    this._heights.clear();
    this._rank.clear();
    const list: number[] = [];
    for (const el of els) {
      const idx = parseInt(el.dataset.idx ?? "", 10);
      if (Number.isNaN(idx) || this._restY.has(idx)) continue;
      const rect = el.getBoundingClientRect();
      if (rect.height <= 0) continue;
      this._restY.set(idx, rect.top - cRect.top + scrollTop);
      this._heights.set(idx, rect.height);
      list.push(idx);
    }
    if (list.length < 2) return false;
    // DOM order IS rank order. Never sort by index value: the queue view
    // renders played rows (lower indices) below up-next rows.
    this._list = list;
    list.forEach((idx, r) => this._rank.set(idx, r));
    this._breakRank = this._computeBreakRank();

    this._dragRank = this._rank.get(this.dragFrom) ?? -1;
    if (this._dragRank < 0) return false;

    const step = this._slotStep();
    if (step > 0) this._stepX = step;
    return true;
  }

  /** Uniform step (height + gap) of the dragged row's slot, from measurements. */
  private _slotStep(): number {
    const r = this._dragRank;
    const stepAt = (a: number, b: number) => {
      const ya = this._restY.get(this._list[a]);
      const yb = this._restY.get(this._list[b]);
      return ya !== undefined && yb !== undefined && yb - ya > 0 ? yb - ya : 0;
    };
    if (r + 1 < this._list.length) {
      const s = stepAt(r, r + 1);
      if (s > 0) return s;
    }
    if (r > 0) {
      const s = stepAt(r - 1, r);
      if (s > 0) return s;
    }
    const deltas: number[] = [];
    for (let i = 1; i < this._list.length; i++) {
      const s = stepAt(i - 1, i);
      if (s > 0) deltas.push(s);
    }
    if (deltas.length === 0) return FALLBACK_STEP;
    deltas.sort((x, y) => x - y);
    return deltas[Math.floor(deltas.length / 2)];
  }

  // ------------------------------------------------------------ slot model

  /** Post-removal rank of a row that sat at rank q before the lift. */
  private _minus(q: number): number {
    return q > this._dragRank ? q - 1 : q;
  }

  private _restX(): number {
    return this._restYOf(this.dragFrom);
  }

  /** Current translateY for a non-dragged row, by RANK q. */
  private _shiftForRank(q: number): number {
    const s = this._stepX;
    let t = 0;
    if (q > this._dragRank) t -= s;
    if (this._minus(q) >= this._K) t += s;
    return t;
  }

  /** Current translateY for a row index. Unknown rows get no shift. */
  private _transformFor(idx: number): number {
    const r = this._rank.get(idx);
    if (r === undefined) return 0;
    if (r === this._dragRank) return this._draggedTransform();
    return this._shiftForRank(r);
  }

  /** translateY that places the dragged row in the gap of slot K. */
  private _draggedTransform(): number {
    const n = this._list.length;
    const K = this._K;
    if (K === this._dragRank) return 0;
    const restX = this._restX();
    if (K >= n - 1) {
      const lastRank = this._dragRank === n - 1 ? n - 2 : n - 1;
      const q = this._list[lastRank];
      const yLast = this._restYOf(q) + this._shiftForRank(lastRank);
      return yLast + this._stepX - restX;
    }
    const rK = K < this._dragRank ? K : K + 1;
    const q = this._list[rK];
    const yAt = this._restYOf(q) + this._shiftForRank(rK);
    return yAt - this._stepX - restX;
  }

  /** Rest Y of a row, estimated from the nearest measured neighbor when the
   * row is not currently mounted (deep virtualized scroll). */
  private _restYOf(idx: number): number {
    const known = this._restY.get(idx);
    if (known !== undefined) return known;
    const qRank = this._rank.get(idx);
    if (qRank === undefined) return 0;
    let bestRank = -1;
    let bestDist = Infinity;
    for (const r of this._rank.values()) {
      if (this._restY.has(this._list[r])) {
        const d = Math.abs(r - qRank);
        if (d < bestDist) {
          bestDist = d;
          bestRank = r;
        }
      }
    }
    if (bestRank < 0) return 0;
    return (
      (this._restY.get(this._list[bestRank]) ?? 0) +
      (qRank - bestRank) * this._stepX
    );
  }

  private _applyTransforms() {
    const root: ParentNode = this._container ?? document;
    const els = root.querySelectorAll<HTMLElement>(this._opts.rowSelector);
    for (const el of els) {
      const idx = parseInt(el.dataset.idx ?? "", 10);
      if (Number.isNaN(idx)) continue;
      const t = this._transformFor(idx);
      el.style.transform = t === 0 ? "" : `translate3d(0, ${t}px, 0)`;
    }
  }

  private _clearTransforms() {
    const root: ParentNode = this._container ?? document;
    const els = root.querySelectorAll<HTMLElement>(this._opts.rowSelector);
    for (const el of els) {
      if (el.style.transform) el.style.transform = "";
    }
  }

  /** Keep dragOver/dropPosition (indicator classes) and targetPosition (the
   * ghost badge) in sync with the current slot K. */
  private _syncIndicator() {
    const n = this._list.length;
    if (n === 0) return;
    if (this._K >= n - 1) {
      const lastRank = this._dragRank === n - 1 ? n - 2 : n - 1;
      this.dragOver = this._list[lastRank];
      this.dropPosition = "after";
    } else {
      const rK = this._K < this._dragRank ? this._K : this._K + 1;
      this.dragOver = this._list[rK];
      this.dropPosition = "before";
    }
    this.targetPosition = this._commitIndex() + 1;
  }

  // ---------------------------------------------------------------- frames

  private _scheduleFrame() {
    if (this._rafId !== null) return;
    this._rafId = requestAnimationFrame(() => {
      this._rafId = null;
      if (!this.dragging) return;
      this._frame();
    });
  }

  private _frame() {
    if (!this.dragging) return;
    if (this._mode !== "pointer") return;
    this._updateGhost();
    this._autoScroll();
    const K = this._hitTest();
    if (K !== this._K) {
      this._K = K;
      this._applyTransforms();
      this._syncIndicator();
    }
    // keep scrolling while the pointer rests in an edge zone
    if (this._inEdgeZone()) this._scheduleFrame();
  }

  private _pointerContentY(): number {
    const container = this._container;
    if (!container) return this._y;
    const rect = container.getBoundingClientRect();
    return this._y - rect.top + container.scrollTop;
  }

  /** Insertion slot K derived from row REST midpoints under the pointer.
   * Deliberately ignores current shifts: K is a pure, monotone function of
   * pointer Y, so it can never get stuck in a hysteresis band when the
   * pointer stops moving. */
  private _hitTest(): number {
    const py = this._pointerContentY();
    let K = 0;
    for (let r = 0; r < this._list.length; r++) {
      if (r === this._dragRank) continue;
      const q = this._list[r];
      const y = this._restYOf(q);
      const h = this._heights.get(q) ?? this._stepX;
      if (py >= y + h / 2) K++;
    }
    return Math.max(0, Math.min(this._list.length - 1, K));
  }

  private _inEdgeZone(): boolean {
    const container = this._container;
    if (!container) return false;
    const rect = container.getBoundingClientRect();
    const edge = this._opts.edgeSize;
    return this._y < rect.top + edge || this._y > rect.bottom - edge;
  }

  private _autoScroll() {
    const container = this._container;
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

  private _handleScroll() {
    if (!this.dragging) return;
    this._syncRows();
    this._scheduleFrame();
  }

  /**
   * Absorb rows that mounted after drag start (virtualization). Measured in
   * the scroll handler and shifted synchronously so they never paint
   * unshifted. New rows are ranked by measured rest Y (visual order).
   */
  private _syncRows() {
    const container = this._container;
    if (!container) return;
    const els = Array.from(container.querySelectorAll<HTMLElement>(this._opts.rowSelector));
    const fresh: { idx: number; restY: number; h: number; el: HTMLElement }[] = [];
    const cRect = container.getBoundingClientRect();
    const scrollTop = container.scrollTop;
    for (const el of els) {
      const idx = parseInt(el.dataset.idx ?? "", 10);
      if (Number.isNaN(idx) || this._restY.has(idx)) continue;
      const rect = el.getBoundingClientRect();
      if (rect.height <= 0) continue;
      // Fresh rows carry no inline transform yet: the rect IS the rest pose.
      fresh.push({ idx, restY: rect.top - cRect.top + scrollTop, h: rect.height, el });
    }
    if (fresh.length === 0) return;
    for (const f of fresh) {
      this._restY.set(f.idx, f.restY);
      this._heights.set(f.idx, f.h);
      // Insert into the visual order by rest Y.
      let at = this._list.length;
      for (let r = 0; r < this._list.length; r++) {
        if (this._restYOf(this._list[r]) > f.restY) {
          at = r;
          break;
        }
      }
      this._list.splice(at, 0, f.idx);
      if (at <= this._dragRank) this._dragRank++;
    }
    this._rebuildRanks();
    this._applyTransforms();
  }

  private _rebuildRanks() {
    this._rank.clear();
    this._list.forEach((idx, r) => this._rank.set(idx, r));
    this._breakRank = this._computeBreakRank();
  }

  /** First rank where DOM order stops ascending (start of the played run in
   * the queue view); list.length when the order is strictly ascending. */
  private _computeBreakRank(): number {
    for (let r = 1; r < this._list.length; r++) {
      if (this._list[r] < this._list[r - 1]) return r;
    }
    return this._list.length;
  }

  // ---------------------------------------------------------------- pointer flow

  private _handleMove(e: PointerEvent) {
    if (this._pressActive && !this.dragging) {
      const dx = e.clientX - this._pressStartX;
      const dy = e.clientY - this._pressStartY;
      const moved = Math.hypot(dx, dy) > this._opts.dragThreshold;
      if (this._pressTouch) {
        // Touch: movement before the long-press lift belongs to scrolling.
        if (moved) {
          this._pressActive = false;
          this._clearPressTimer();
        }
        return;
      }
      if (moved) {
        this._clearPressTimer();
        this._startPointerDrag(e, this._pressIndex);
      }
      return;
    }
    if (!this.dragging || this._mode !== "pointer") return;
    if (this._pointerId !== -1 && e.pointerId !== this._pointerId) return;
    this._x = e.clientX;
    this._y = e.clientY;
    this._scheduleFrame();
  }

  private _handleUp(e: PointerEvent) {
    if (this._pressActive && !this.dragging) {
      this._pressActive = false;
      this._clearPressTimer();
      return;
    }
    if (!this.dragging || this._mode !== "pointer") return;
    if (this._pointerId !== -1 && e.pointerId !== this._pointerId) return;
    // pointercancel bubbles from the captured row to the document; it must
    // revert, never commit.
    if (e.type === "pointercancel") {
      this._revert();
      return;
    }
    this._commit();
  }

  // ---------------------------------------------------------------- keyboard flow

  /** Document-level keys: pointer-mode Escape only (keyboard drags are
   * routed through the focused handle via onHandleKeydown). */
  private _handleKey(e: KeyboardEvent) {
    if (!this.dragging) return;
    if (this._mode === "pointer" && e.key === "Escape") {
      e.preventDefault();
      this._revert();
    }
  }

  /** Keys during an active keyboard drag. Returns a live-region announcement
   * or null when the key is unhandled. */
  private _keyHandle(e: KeyboardEvent): string | null {
    const n = this._list.length;
    switch (e.key) {
      case "ArrowUp":
        e.preventDefault();
        this._keyTo(this._K - 1);
        break;
      case "ArrowDown":
        e.preventDefault();
        this._keyTo(this._K + 1);
        break;
      case "Home":
        e.preventDefault();
        this._keyTo(0);
        break;
      case "End":
        e.preventDefault();
        this._keyTo(n - 1);
        break;
      case "Enter":
      case " ":
      case "Tab":
        // Tab must never strand focus mid-drag; treat it as a drop.
        e.preventDefault();
        return this._keyDrop();
      case "Escape":
        e.preventDefault();
        {
          const title = this._titleAt(this.dragFrom);
          const pos = (this._rank.get(this.dragFrom) ?? 0) + 1;
          this._revert();
          return `Cancelled. ${title} returned to position ${pos}.`;
        }
      default:
        return null;
    }
    return `Position ${this._K + 1} of ${n}. Arrow keys to move, Enter to drop, Escape to cancel.`;
  }

  private _keyDrop(): string {
    const from = this.dragFrom;
    const t = this._commitIndex();
    const title = this._titleAt(from);
    this._commit();
    return t === from
      ? `Dropped ${title}. Position unchanged.`
      : `Dropped ${title} at position ${t + 1} of ${this._fullLen}.`;
  }

  private _keyTo(K: number) {
    const clamped = Math.max(0, Math.min(this._list.length - 1, K));
    if (clamped === this._K) return;
    this._K = clamped;
    this._applyTransforms();
    this._syncIndicator();
    this._scrollGapIntoView();
  }

  private _scrollGapIntoView() {
    const container = this._container;
    if (!container) return;
    const y = this._restX() + this._draggedTransform();
    const edge = this._opts.edgeSize / 2;
    if (y < container.scrollTop + edge) {
      container.scrollTop = Math.max(0, y - edge);
    } else if (y + this._stepX > container.scrollTop + container.clientHeight - edge) {
      container.scrollTop = y + this._stepX - container.clientHeight + edge;
    }
  }

  // ---------------------------------------------------------------- commit / revert

  /** Underlying-list index the dragged row should land at, given slot K. */
  private _commitIndex(): number {
    const from = this.dragFrom;
    const n = this._list.length;
    if (this._K >= n - 1) {
      // "After the visually last row". Its list index is NOT necessarily
      // length-1: in the queue view the last rendered rows are the played
      // ones (low indices). Commit directly after that row's list index.
      const lastIdx = this._list[n - 1];
      return from > lastIdx ? lastIdx + 1 : lastIdx;
    }
    const rK = this._K < this._dragRank ? this._K : this._K + 1;
    const qK = this._list[rK];
    // Cross-section rule for views whose DOM order is a rotation of the
    // underlying array (the queue renders played rows below up-next even
    // though they precede them in the array). Dropping an up-next track at
    // the top edge of the played section must keep it the LAST UP-NEXT
    // track - the same visual gap, but section membership decides what
    // auto-advance plays next. Deeper played drops join the played section.
    if (rK >= this._breakRank && this._dragRank < this._breakRank && rK === this._breakRank) {
      return this._fullLen - 1;
    }
    // The row we land before sits at index qK; with `from` removed, rows
    // after it shift down by one.
    return qK > from ? qK - 1 : qK;
  }

  private _commit() {
    if (!this.dragging) return;
    const from = this.dragFrom;
    // Re-run the hit test synchronously: the final pointermove's rAF frame
    // may never have fired before pointerup, so this._K could be stale.
    if (this._mode === "pointer") {
      const K = this._hitTest();
      if (K !== this._K) {
        this._K = K;
        this._syncIndicator();
      }
    }
    const t = this._commitIndex();
    // Clear slot transforms synchronously, BEFORE the DOM reorder, so Svelte's
    // animate:flip measures clean rest positions and animates old -> new
    // layout itself. All of this happens in this task before the next paint,
    // so nothing snaps visibly.
    this._applyRestTransforms();
    if (t !== from && t >= 0 && t < this._fullLen) {
      this._opts.onReorder(from, t);
      this._justDraggedUntil = performance.now() + 250;
    }
    this.dragging = false;
    this.dragFrom = -1;
    this.dragOver = -1;
    this.targetPosition = -1;
    this._teardownListeners();
    this._restoreBody();
  }

  private _applyRestTransforms() {
    this._clearTransforms();
  }

  private _revert() {
    if (!this.dragging) return;
    this.dragging = false;
    this.dragFrom = -1;
    this.dragOver = -1;
    this.targetPosition = -1;
    this._teardownListeners();
    // Rows slide back to their rest positions via the drag transition...
    this._clearTransforms();
    // ...then unlock new drags once the slide is done, so a fresh measure
    // never reads mid-animation geometry.
    this._settling = true;
    this._settleTimer = setTimeout(() => {
      this._settleTimer = null;
      this._settling = false;
      this._restoreBody();
    }, SETTLE_MS);
  }

  private _restoreBody() {
    document.body.style.touchAction = "";
    document.body.style.userSelect = "";
    document.body.style.cursor = "";
    document.body.classList.remove("is-dragging");
  }

  // ---------------------------------------------------------------- plumbing

  private _teardownListeners() {
    document.removeEventListener("pointermove", this._onMove);
    document.removeEventListener("pointerup", this._onUp);
    document.removeEventListener("pointercancel", this._onUp);
    document.removeEventListener("touchmove", this._onTouchMove);
    document.removeEventListener("keydown", this._onKey);
    document.removeEventListener("contextmenu", this._onContextMenu);
    window.removeEventListener("blur", this._onBlur);
    window.removeEventListener("resize", this._onResize);
    if (this._container) {
      this._container.removeEventListener("scroll", this._onScroll);
    }
    if (this._capturedEl) {
      this._capturedEl.removeEventListener("lostpointercapture", this._onLostCapture);
      if (this._pointerId !== -1) {
        try {
          this._capturedEl.releasePointerCapture(this._pointerId);
        } catch {
          /* already released */
        }
      }
      this._capturedEl = null;
    }
    this._pointerId = -1;
    if (this._rafId !== null) {
      cancelAnimationFrame(this._rafId);
      this._rafId = null;
    }
  }

  private _clearPressTimer() {
    if (this._pressTimer !== null) {
      clearTimeout(this._pressTimer);
      this._pressTimer = null;
    }
  }

  private _updateGhost() {
    if (!this._ghostEl || this._mode !== "pointer") return;
    this._ghostEl.style.transform = `translate3d(${this._x + 14}px, ${this._y + 14}px, 0)`;
  }

  private _titleAt(index: number): string {
    return this._opts.getList()[index]?.title ?? "track";
  }

  /** After a keyboard reorder the row DOM is re-keyed by Svelte; put focus
   * back on the moved track's handle. Runs after Svelte's DOM flush. */
  private _refocusHandle(index: number) {
    const id = this._opts.getList()[index]?.id;
    if (id === undefined) return;
    queueMicrotask(() => {
      requestAnimationFrame(() => {
        const root: ParentNode = this._container ?? document;
        for (const el of root.querySelectorAll<HTMLElement>(".drag-row")) {
          if (el.dataset.trackId === id) {
            el.querySelector<HTMLElement>(".handle-btn")?.focus();
            return;
          }
        }
      });
    });
  }
}
