// Cookie-source state for the bot-wall fix (issue: "ytdlp_botwall").
//
// PRIVACY RULE (maintainer directive): the cookie choice is NEVER persisted -
// no localStorage, no config file, nothing on disk. In-memory svelte state
// only, so it resets on every app restart. Browser cookies are
// session-scoped credentials tied to the user's YouTube identity; remembering
// which browser to borrow them from (or worse, any cookie material itself)
// would turn a deliberate per-session gesture into an unnoticed standing
// grant. The user re-affirms the choice each session; nothing remembered.
//
// Structure mirrors config.svelte.ts (a small class of $state fields with an
// apply/sync surface) minus every persistence path on purpose.

export type CookieSource = "none" | "firefox" | "chrome" | "chromium" | "edge";

/** File-picked cookies.txt path (mutually exclusive with the browser presets). */
export type CookieSourceValue = CookieSource | (string & {});

export const COOKIE_SOURCES: { value: CookieSource; label: string }[] = [
  { value: "none", label: "None" },
  { value: "firefox", label: "Firefox" },
  { value: "chrome", label: "Chrome" },
  { value: "chromium", label: "Chromium" },
  { value: "edge", label: "Edge" },
];

export function isCookieSource(v: string): v is CookieSource {
  return v === "none" || v === "firefox" || v === "chrome" || v === "chromium" || v === "edge";
}

class CookieState {
  /** Last applied source (never persisted; informational only). */
  applied = $state<CookieSourceValue | null>(null);
  /** True while a set_cookie_source round-trip is in flight. */
  applying = $state(false);
  /** Inline error from a rejected apply (e.g. bad cookies.txt path). */
  applyError = $state("");

  /** UI selection before Apply; separate from `applied` so a rejected apply
   * leaves the last-known-good state intact. */
  selected = $state<CookieSourceValue>("none");

  beginApply(): boolean {
    if (this.applying) return false;
    this.applying = true;
    this.applyError = "";
    return true;
  }

  finishApply(source: CookieSourceValue): void {
    this.applying = false;
    this.applied = source;
    this.selected = source;
  }

  failApply(message: string): void {
    this.applying = false;
    this.applyError = message;
  }

  /** Reset session state (used when the banner is dismissed without saving). */
  clearError(): void {
    this.applyError = "";
  }
}

export const cookies = new CookieState();
