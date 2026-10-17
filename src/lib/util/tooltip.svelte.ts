// Sunder custom tooltip engine - replaces native browser `title=` tooltips.
//
// Two entry points share one implementation:
//   1. Action (primary, ergonomic for app-wide wiring):  use:tooltip={text}
//      or use:tooltip={{ text, placement, delay }}
//   2. Component wrapper: <Tooltip text="..." placement="top" delay={450}>
//      (see Tooltip.svelte - it binds this same action to a display:contents
//      host and resolves the rect from the slotted trigger element)
//
// This module is intentionally rune-free so src/dev/tooltipTest.mjs can
// import the pure positioning math directly under node --experimental-strip-types.

// ---- types -----------------------------------------------------------------

export type Placement = "top" | "bottom" | "left" | "right";

export interface Rect {
  left: number;
  top: number;
  width: number;
  height: number;
}

export interface Size {
  width: number;
  height: number;
}

export interface Viewport {
  width: number;
  height: number;
}

export interface PlacementInput {
  /** Trigger rect in viewport coordinates. */
  trigger: Rect;
  /** Measured tooltip size. */
  tooltip: Size;
  /** Viewport box the tooltip must stay inside. */
  viewport: Viewport;
  /** Preferred placement; flipped on viewport collision. */
  placement: Placement;
  /** Minimum distance to the viewport edge. Default 8. */
  gutter?: number;
  /** Gap between trigger and tooltip on the placement axis. Default 6. */
  offset?: number;
}

export interface PlacementResult {
  placement: Placement;
  left: number;
  top: number;
  /** True when the preferred placement collided and the opposite side won. */
  flipped: boolean;
  /** True when an axis was clamped to respect the gutter. */
  clamped: boolean;
}

// ---- pure positioning ------------------------------------------------------

const FLIP: Record<Placement, Placement> = {
  top: "bottom",
  bottom: "top",
  left: "right",
  right: "left",
};

/**
 * Compute the tooltip's fixed-position coordinates.
 *
 * Rules:
 * - tooltip is centered on the trigger on the cross axis
 * - sits `offset` px from the trigger on the placement axis
 * - if the preferred side would push the tooltip within `gutter` px of the
 *   viewport edge, it flips to the opposite side
 * - if the flipped side also collides, the side with more room wins
 * - overhang on the cross axis is clamped so the tooltip never enters the
 *   gutter on any edge
 * - degenerate (zero) sizes/rects never produce NaN; results stay finite and
 *   clamped inside the viewport
 */
export function computePosition(input: PlacementInput): PlacementResult {
  const gutter = input.gutter ?? 8;
  const offset = input.offset ?? 6;
  const tw = finite(input.tooltip.width);
  const th = finite(input.tooltip.height);
  const vw = Math.max(finite(input.viewport.width), 0);
  const vh = Math.max(finite(input.viewport.height), 0);
  const t = normalizeRect(input.trigger);

  let placement = input.placement;
  const fits = (p: Placement): boolean => {
    switch (p) {
      case "top":
        return t.top - offset - th >= gutter;
      case "bottom":
        return t.top + t.height + offset + th <= vh - gutter;
      case "left":
        return t.left - offset - tw >= gutter;
      case "right":
        return t.left + t.width + offset + tw <= vw - gutter;
    }
  };

  let flipped = false;
  if (!fits(placement)) {
    const other = FLIP[placement];
    // Prefer the flip; if neither side fits, keep whichever has more room.
    if (fits(other)) {
      placement = other;
      flipped = true;
    } else if (!fits(placement) && roomFor(other, t, tw, th, vw, vh, gutter, offset) > roomFor(placement, t, tw, th, vw, vh, gutter, offset)) {
      placement = other;
      flipped = true;
    }
  }

  // Anchor point on the placement axis + centered cross axis.
  let left: number;
  let top: number;
  switch (placement) {
    case "top":
      left = t.left + t.width / 2 - tw / 2;
      top = t.top - offset - th;
      break;
    case "bottom":
      left = t.left + t.width / 2 - tw / 2;
      top = t.top + t.height + offset;
      break;
    case "left":
      left = t.left - offset - tw;
      top = t.top + t.height / 2 - th / 2;
      break;
    case "right":
      left = t.left + t.width + offset;
      top = t.top + t.height / 2 - th / 2;
      break;
  }

  // Clamp cross-axis (and the placement axis when both sides collided) so the
  // tooltip never enters the gutter on any viewport edge.
  let clamped = false;
  const clampMin = gutter;
  const clampLeft = vw - gutter - tw;
  const clampTop = vh - gutter - th;
  const cx = Math.min(Math.max(left, clampMin), Math.max(clampMin, clampLeft));
  if (cx !== left) clamped = true;
  const cy = Math.min(Math.max(top, clampMin), Math.max(clampMin, clampTop));
  if (cy !== top) clamped = true;

  return { placement, left: finite(cx), top: finite(cy), flipped, clamped };
}

function roomFor(
  p: Placement,
  t: Rect,
  tw: number,
  th: number,
  vw: number,
  vh: number,
  gutter: number,
  offset: number,
): number {
  switch (p) {
    case "top":
      return t.top - gutter - offset;
    case "bottom":
      return vh - gutter - offset - (t.top + t.height);
    case "left":
      return t.left - gutter - offset;
    case "right":
      return vw - gutter - offset - (t.left + t.width);
  }
}

function finite(n: number): number {
  return typeof n === "number" && Number.isFinite(n) ? n : 0;
}

function normalizeRect(r: Rect): Rect {
  return {
    left: finite(r.left),
    top: finite(r.top),
    width: Math.max(finite(r.width), 0),
    height: Math.max(finite(r.height), 0),
  };
}

/**
 * Resolve the element whose rect anchors the tooltip. Tooltip.svelte hosts the
 * action on a display:contents wrapper (no box of its own), so fall back to
 * the first element child when the host has a zero rect.
 */
export function resolveTriggerRect(el: Element): Rect {
  const r = el.getBoundingClientRect();
  if ((r.width === 0 && r.height === 0) && el.firstElementChild) {
    return el.firstElementChild.getBoundingClientRect();
  }
  return r;
}

// ---- suppression guard -----------------------------------------------------
//
// Tooltips must never appear during drag interactions or while a context menu
// is open. The wiring flips the guard around those lifetimes; the engine also
// auto-suppresses while the drag engine's body.is-dragging class is present.

const SUPPRESSED_REASONS = new Set<string>();

export function suppressTooltips(reason = "default"): void {
  SUPPRESSED_REASONS.add(reason);
}

export function unsuppressTooltips(reason = "default"): void {
  SUPPRESSED_REASONS.delete(reason);
}

export function isSuppressed(): boolean {
  if (SUPPRESSED_REASONS.size > 0) return true;
  if (typeof document !== "undefined" && document.body?.classList.contains("is-dragging")) {
    return true;
  }
  return false;
}

// ---- action (primary API) --------------------------------------------------

export interface TooltipParams {
  /** Tooltip content. Empty string disables the tooltip. */
  text: string;
  placement?: Placement;
  /** Hover-intent delay in ms. Default 450 - never an instant flash. */
  delay?: number;
}

export const TOOLTIP_GUTTER = 8;
export const TOOLTIP_OFFSET = 6;
export const TOOLTIP_DELAY = 450;

/**
 * Svelte 5 action: <button use:tooltip="Open playlist">...</button>
 * or           <button use:tooltip={{ text, placement: "bottom" }}>
 *
 * Shows on hover-intent (450ms) and on keyboard focus-visible (a11y parity
 * with the native title attribute). Hides on pointerleave, blur, Escape and
 * pointerdown. The tooltip node is aria-hidden and pointer-events: none -
 * the trigger's own aria-label stays the real accessibility surface; this is
 * pure visual polish.
 */
export function tooltip(el: HTMLElement, params: TooltipParams | string) {
  let cfg: TooltipParams =
    typeof params === "string" ? { text: params } : params;
  let node: HTMLElement | null = null;
  let timer: ReturnType<typeof setTimeout> | null = null;
  let raf = 0;
  let visible = false;

  const delay = () => cfg.delay ?? TOOLTIP_DELAY;

  function removeNode() {
    if (raf) cancelAnimationFrame(raf);
    raf = 0;
    node?.remove();
    node = null;
    visible = false;
  }

  function hide() {
    if (timer) clearTimeout(timer);
    timer = null;
    if (node) {
      // Reverse the entrance, then drop the node.
      node.style.opacity = "0";
      node.style.transform = "scale(0.97)";
      const dying = node;
      node = null;
      visible = false;
      setTimeout(() => dying.remove(), 160);
    }
  }

  function show() {
    const text = cfg.text;
    if (!text || visible || isSuppressed()) return;
    const rect = resolveTriggerRect(el);
    // Hidden triggers (display:none ancestors) produce a zero rect - skip.
    if (rect.width === 0 && rect.height === 0) return;

    node = document.createElement("div");
    node.className = "sunder-tooltip";
    node.textContent = text;
    node.setAttribute("aria-hidden", "true");
    applyTooltipStyles(node);
    document.body.appendChild(node);

    // Measure at its final width, then place with collision handling.
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    const pos = computePosition({
      trigger: rect,
      tooltip: { width: node.offsetWidth, height: node.offsetHeight },
      viewport: { width: vw, height: vh },
      placement: cfg.placement ?? "top",
      gutter: TOOLTIP_GUTTER,
      offset: TOOLTIP_OFFSET,
    });
    node.dataset.placement = pos.placement;
    node.style.left = `${Math.round(pos.left)}px`;
    node.style.top = `${Math.round(pos.top)}px`;

    // Entrance: 140ms fade + scale(0.97 -> 1), transform/opacity only.
    const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    if (!reduced) node.style.transform = "scale(0.97)";
    node.style.opacity = "0";
    visible = true;
    raf = requestAnimationFrame(() => {
      raf = 0;
      if (!node) return;
      node.style.opacity = "1";
      if (!reduced) node.style.transform = "scale(1)";
    });
  }

  function arm() {
    if (timer) clearTimeout(timer);
    if (isSuppressed()) return;
    timer = setTimeout(() => {
      timer = null;
      show();
    }, delay());
  }

  const onPointerEnter = () => arm();
  const onPointerLeave = () => hide();
  const onPointerDown = () => hide();
  const onFocusIn = () => {
    // Keyboard parity with the title attribute: only focus-visible shows.
    if (el.matches?.(":focus-visible")) arm();
  };
  const onFocusOut = () => hide();
  const onWindowKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") hide();
  };
  const onWindowChange = () => hide();

  el.addEventListener("pointerenter", onPointerEnter);
  el.addEventListener("pointerleave", onPointerLeave);
  el.addEventListener("pointerdown", onPointerDown);
  el.addEventListener("focusin", onFocusIn);
  el.addEventListener("focusout", onFocusOut);
  window.addEventListener("keydown", onWindowKey);
  window.addEventListener("scroll", onWindowChange, true);
  window.addEventListener("resize", onWindowChange);

  return {
    update(next: TooltipParams | string) {
      cfg = typeof next === "string" ? { text: next } : next;
      if (!cfg.text) hide();
    },
    destroy() {
      if (timer) clearTimeout(timer);
      if (raf) cancelAnimationFrame(raf);
      hide();
      removeNode();
      el.removeEventListener("pointerenter", onPointerEnter);
      el.removeEventListener("pointerleave", onPointerLeave);
      el.removeEventListener("pointerdown", onPointerDown);
      el.removeEventListener("focusin", onFocusIn);
      el.removeEventListener("focusout", onFocusOut);
      window.removeEventListener("keydown", onWindowKey);
      window.removeEventListener("scroll", onWindowChange, true);
      window.removeEventListener("resize", onWindowChange);
    },
  };
}

// ---- shared visual treatment ----------------------------------------------

/** DragGhost-matching glassy shell, applied inline so the action-created node
 * needs no stylesheet dependency. Colors come from the Sunder CSS variables. */
export function applyTooltipStyles(node: HTMLElement): void {
  node.style.position = "fixed";
  node.style.pointerEvents = "none";
  node.style.zIndex = "10000";
  node.style.maxWidth = "280px";
  node.style.padding = "6px 10px";
  node.style.background = "var(--bg-overlay)";
  node.style.border = "1px solid var(--accent-dim)";
  node.style.borderRadius = "var(--radius, 8px)";
  node.style.boxShadow = "0 8px 24px rgba(0, 0, 0, 0.45), 0 0 0 1px var(--accent-dim)";
  node.style.backdropFilter = "blur(12px)";
  node.style.setProperty("-webkit-backdrop-filter", "blur(12px)");
  node.style.color = "var(--text-primary)";
  node.style.fontSize = "0.75rem";
  node.style.lineHeight = "1.45";
  node.style.whiteSpace = "normal";
  node.style.overflowWrap = "break-word";
  node.style.transition =
    "opacity 140ms ease, transform 140ms ease";
  // prefers-reduced-motion kills the transform channel.
  const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
  if (reduced) node.style.transition = "opacity 140ms ease";
}
