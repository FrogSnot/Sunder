<script lang="ts">
  import { fade, fly } from "svelte/transition";
  import { quintOut } from "svelte/easing";
  import { flip } from "svelte/animate";
  import { tooltip } from "../util/tooltip.svelte";
  import { search, playTrack, addToPlaylist } from "../ipc/bridge";
  import { player } from "../state/player.svelte";
  import { toastState } from "../state/toast.svelte";
  import { parseSuggestions, type Suggestion } from "../util/aiSuggest";
  import TrackArt from "./TrackArt.svelte";

  let {
    playlistId,
    playlistName,
    trackList,
    onClose,
  }: {
    playlistId: number;
    playlistName: string;
    trackList: { title: string; artist: string }[];
    onClose: () => void;
  } = $props();

  // ---- prompt ---------------------------------------------------------------

  const PROMPT_TEMPLATE = (name: string, tracks: string) =>
    `You are a music discovery assistant. I have a playlist called "${name}" with these tracks:\n\n${tracks}\n\nSuggest exactly 10 songs that fit the same vibe and would fit well in this playlist. They must NOT already be in the track list.\n\nSTRICT output format - follow it exactly:\n- One suggestion per line.\n- Each line: Song Title | Artist\n- No numbering, no bullets, no commentary, no extra text, nothing else.`;

  const prompt = $derived(
    PROMPT_TEMPLATE(
      playlistName,
      trackList.map((t) => `${t.title} | ${t.artist}`).join("\n"),
    ),
  );

  async function copyPrompt(): Promise<boolean> {
    try {
      if (navigator.clipboard?.writeText) {
        await navigator.clipboard.writeText(prompt);
        return true;
      }
    } catch {
      // fall through to the legacy path
    }
    try {
      const ta = document.createElement("textarea");
      ta.value = prompt;
      ta.style.position = "fixed";
      ta.style.opacity = "0";
      document.body.appendChild(ta);
      ta.select();
      const ok = document.execCommand("copy");
      ta.remove();
      return ok;
    } catch {
      return false;
    }
  }

  // ---- dialog lifecycle -------------------------------------------------------

  let textareaEl = $state<HTMLTextAreaElement | null>(null);
  let input = $state("");
  let resolving = $state(false);
  let resolveDone = $state(0);
  let resolveTotal = $state(0);
  let results = $state<Row[]>([]);

  interface Row {
    sug: Suggestion;
    state: "pending" | "resolved" | "missing";
    track: import("../types").Track | null;
    quality: "title-artist" | "title-only" | null;
    checked: boolean;
  }

  $effect(() => {
    // Focus the paste box once mounted. rAF: the intro transition (fly) is
    // still settling on the first frame, and a focus fired during it can be
    // lost; one frame later it sticks.
    if (textareaEl) {
      const raf = requestAnimationFrame(() => textareaEl?.focus());
      return () => cancelAnimationFrame(raf);
    }
  });

  $effect(() => {
    // Copy once on mount. The flag guards against effect re-runs (dev HMR).
    if (textareaEl && !copyDone) {
      copyDone = true;
      void doCopy();
    }
  });

  let copyDone = $state(false);

  async function doCopy() {
    const ok = await copyPrompt();
    toastState.add(
      ok
        ? "Prompt copied - paste it into any AI, then paste the answer back"
        : "Could not copy automatically - select and copy the prompt below",
      ok ? "info" : "error",
      5000,
    );
  }

  function handleClose() {
    results = [];
    onClose();
  }

  $effect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") handleClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  // ---- resolution --------------------------------------------------------------

  async function handleFind() {
    if (resolving) return;
    const parsed = parseSuggestions(input);
    if (parsed.length === 0) {
      toastState.add("No song lines found - paste the AI's answer first", "error", 4000);
      return;
    }
    resolving = true;
    resolveDone = 0;
    resolveTotal = parsed.length;
    results = parsed.map((sug) => ({ sug, state: "pending", track: null, quality: null, checked: true }));

    // Sequential so the UI stays smooth and the backend search stays cheap.
    for (let i = 0; i < parsed.length; i++) {
      const sug = parsed[i];
      const row = results[i];
      try {
        const query = sug.artist ? `${sug.title} ${sug.artist}` : sug.title;
        const res = await search(query, 5);
        const hit = pickBest(res.tracks ?? [], sug);
        if (hit) {
          row.track = hit.track;
          row.quality = hit.quality;
          row.state = "resolved";
        } else {
          row.state = "missing";
          row.checked = false;
        }
      } catch {
        row.state = "missing";
        row.checked = false;
      }
      resolveDone = i + 1;
      results = results; // trigger reactivity for the progress bar
    }
    resolving = false;
  }

  /** Exact-enough matching: artist must overlap when provided; otherwise the
   * top hit is treated as title-only quality. */
  function pickBest(
    tracks: import("../types").Track[],
    sug: Suggestion,
  ): { track: import("../types").Track; quality: "title-artist" | "title-only" } | null {
    if (tracks.length === 0) return null;
    // Fold diacritics so "Yoga" matches "Y\u00F3ga" and "Bjork" matches
    // "Bj\u00F6rk" - AI answers routinely drop accents.
    const norm = (s: string) =>
      s
        .toLowerCase()
        .normalize("NFD")
        .replace(/[\u0300-\u036f]/g, "")
        .replace(/[^a-z0-9]+/g, " ")
        .trim();
    const t = norm(sug.title);
    const a = norm(sug.artist);
    for (const tr of tracks) {
      if (t && norm(tr.title).includes(t)) {
        if (!a || norm(tr.artist).includes(a) || a.includes(norm(tr.artist))) {
          return { track: tr, quality: "title-artist" };
        }
      }
    }
    for (const tr of tracks) {
      if (t && norm(tr.title).includes(t)) {
        return { track: tr, quality: "title-only" };
      }
    }
    return null;
  }

  // ---- actions -------------------------------------------------------------------

  let addingAll = $state(false);
  let addingQueue = $state(false);

  function toggleRow(row: Row) {
    if (row.state !== "resolved") return;
    row.checked = !row.checked;
  }

  async function handleAddToPlaylist(row: Row) {
    if (!row.track) return;
    try {
      await addToPlaylist(playlistId, row.track.id);
      toastState.add(`Added "${row.track.title}" to ${playlistName}`, "info", 3000);
    } catch (e) {
      toastState.add(`Failed to add: ${e}`, "error", 5000);
    }
  }

  function handlePlayNext(row: Row) {
    if (!row.track) return;
    player.playNext(row.track);
    toastState.add(`Playing "${row.track.title}" next`, "info", 3000);
  }

  function handleAddToQueue(row: Row) {
    if (!row.track) return;
    player.addToQueue(row.track);
    toastState.add(`Added "${row.track.title}" to queue`, "info", 3000);
  }

  async function handlePlay(row: Row) {
    if (!row.track) return;
    try {
      await playTrack(row.track);
    } catch (e) {
      toastState.add(`Failed to play: ${e}`, "error", 5000);
    }
  }

  async function handleAddAll() {
    const rows = results.filter((r) => r.state === "resolved" && r.checked && r.track);
    if (rows.length === 0 || addingAll) return;
    addingAll = true;
    let added = 0;
    try {
      for (const row of rows) {
        await addToPlaylist(playlistId, row.track!.id);
        added++;
      }
      toastState.add(`Added ${added} song${added === 1 ? "" : "s"} to ${playlistName}`, "info", 4000);
    } catch (e) {
      toastState.add(`Added ${added}, then failed: ${e}`, "error", 5000);
    } finally {
      addingAll = false;
    }
  }

  /** Add all checked rows to the queue (same path the queue view uses).
   * Duplicates inside the queue itself are skipped - player.addToQueue
   * already dedupes against existing entries, so this only guards the batch
   * against proposing the same song twice. */
  function handleAddAllToQueue() {
    const rows = results.filter((r) => r.state === "resolved" && r.checked && r.track);
    if (rows.length === 0 || addingQueue) return;
    addingQueue = true;
    try {
      const seen = new Set(player.queue.map((t) => t.id));
      let added = 0;
      for (const row of rows) {
        const track = row.track!;
        if (seen.has(track.id)) continue;
        seen.add(track.id);
        player.addToQueue(track);
        added++;
      }
      if (added === 0) {
        toastState.add("All selected songs are already in the queue", "info", 4000);
      } else {
        toastState.add(`Added ${added} song${added === 1 ? "" : "s"} to queue`, "info", 4000);
      }
    } finally {
      addingQueue = false;
    }
  }

  const resolvedCount = $derived(results.filter((r) => r.state === "resolved").length);
  const checkedCount = $derived(results.filter((r) => r.state === "resolved" && r.checked).length);
</script>

<div class="ai-backdrop" transition:fade={{ duration: 150 }} role="presentation">
  <div
    class="ai-modal"
    transition:fly={{ y: 16, duration: 220, easing: quintOut }}
    role="dialog"
    aria-modal="true"
    aria-label="AI song suggestions for {playlistName}"
  >
    <div class="ai-head">
      <span class="ai-head-icon" aria-hidden="true">
        <svg viewBox="0 0 24 24" fill="currentColor">
          <path d="M12 2l1.9 5.7L19.5 9l-5.6 1.9L12 16l-1.9-5.1L4.5 9l5.6-1.3L12 2z" />
          <path d="M19 14l.9 2.6 2.6.9-2.6.9L19 21l-.9-2.6-2.6-.9 2.6-.9L19 14z" />
        </svg>
      </span>
      <div class="ai-head-text">
        <h3 class="ai-title">Ask AI for suggestions</h3>
        <p class="ai-sub">Playing matchmaker for "{playlistName}"</p>
      </div>
      <button class="ai-close" onclick={handleClose} aria-label="Close">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="18" y1="6" x2="6" y2="18" /><line x1="6" y1="6" x2="18" y2="18" />
        </svg>
      </button>
    </div>

    <div class="ai-body">
      <label class="ai-label" for="ai-prompt">1. Copy this prompt into any AI chat</label>
      <textarea id="ai-prompt" class="ai-prompt" readonly rows={6}>{prompt}</textarea>

      <label class="ai-label" for="ai-answer">2. Paste the AI's answer here</label>
      <textarea
        id="ai-answer"
        class="ai-answer"
        bind:value={input}
        bind:this={textareaEl}
        rows={6}
        placeholder={"e.g.\nMidnight City | M83\nNightcall - Kavinsky"}
        onkeydown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) handleFind();
        }}
      ></textarea>
    </div>

    <div class="ai-foot">
      {#if resolving}
        <div class="ai-progress" aria-live="polite">
          <div class="ai-progress-bar">
            <div class="ai-progress-fill" style="width: {resolveTotal ? (resolveDone / resolveTotal) * 100 : 0}%"></div>
          </div>
          <span class="ai-progress-label">Searching {resolveDone}/{resolveTotal}</span>
        </div>
      {/if}
      <button class="ai-find" onclick={handleFind} disabled={resolving || input.trim().length === 0}>
        {#if resolving}
          <svg class="spin" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <path d="M21 12a9 9 0 1 1-6.2-8.56" />
          </svg>
          Searching...
        {:else}
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <circle cx="11" cy="11" r="8" />
            <line x1="21" y1="21" x2="16.65" y2="16.65" />
          </svg>
          Find songs
        {/if}
      </button>
      <button class="ai-cancel" onclick={handleClose}>Close</button>
    </div>

    {#if results.length > 0}
      <div class="ai-results">
        <div class="ai-results-head">
          <span class="ai-results-count">
            {resolvedCount} of {results.length} found
            {#if resolvedCount > 0}
              &middot; {checkedCount} selected
            {/if}
          </span>
          {#if resolvedCount > 0}
            <div class="ai-addall-row">
              <button class="ai-addall ai-addall-secondary" onclick={handleAddAllToQueue} disabled={addingQueue || checkedCount === 0} use:tooltip={{ text: "Append checked songs to the play queue", placement: "bottom" }}>
                <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" aria-hidden="true">
                  <line x1="3" y1="6" x2="15" y2="6" />
                  <line x1="3" y1="12" x2="15" y2="12" />
                  <line x1="3" y1="18" x2="11" y2="18" />
                  <line x1="17" y1="13" x2="17" y2="21" />
                  <line x1="13" y1="17" x2="21" y2="17" />
                </svg>
                {addingQueue ? "Adding..." : `Add all to queue (${checkedCount})`}
              </button>
              <button class="ai-addall" onclick={handleAddAll} disabled={addingAll || checkedCount === 0}>
                {addingAll ? "Adding..." : `Add all (${checkedCount})`}
              </button>
            </div>
          {/if}
        </div>
        <div class="ai-list">
          {#each results as row, i (row.sug.line + "-" + i)}
            <div
              class="ai-row"
              class:ai-missing={row.state === "missing"}
              class:ai-pending={row.state === "pending"}
              animate:flip={{ duration: 200 }}
            >
              {#if row.state === "resolved" && row.track}
                <button
                  class="ai-check"
                  class:checked={row.checked}
                  onclick={() => toggleRow(row)}
                  role="checkbox"
                  aria-checked={row.checked}
                  aria-label="Include {row.track.title}"
                >
                  {#if row.checked}
                    <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3">
                      <polyline points="20 6 9 17 4 12" />
                    </svg>
                  {/if}
                </button>
                <TrackArt track={row.track} size={38} badge={false} />
                <button class="ai-row-main" onclick={() => handlePlay(row)} aria-label="Play {row.track.title}" use:tooltip={{ text: `Play ${row.track.title}`, placement: "top" }}>
                  <span class="ai-row-title">{row.track.title}</span>
                  <span class="ai-row-artist">{row.track.artist}</span>
                </button>
                {#if row.quality === "title-only"}
                  <span class="ai-quality" use:tooltip={{ text: "Matched on title only - verify before adding", placement: "top" }}>title match</span>
                {/if}
                <button class="ai-act" onclick={() => handleAddToPlaylist(row)} aria-label="Add {row.track.title} to {playlistName}" use:tooltip={{ text: `Add to ${playlistName}`, placement: "top" }}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                    <line x1="12" y1="5" x2="12" y2="19" /><line x1="5" y1="12" x2="19" y2="12" />
                  </svg>
                </button>
                <button class="ai-act" onclick={() => handlePlayNext(row)} aria-label="Play {row.track.title} next" use:tooltip={{ text: "Play next", placement: "top" }}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                    <polyline points="17 1 21 5 17 9" />
                    <path d="M3 11V9a4 4 0 0 1 4-4h14" />
                    <polyline points="7 23 3 19 7 15" />
                    <path d="M21 13v2a4 4 0 0 1-4 4H3" />
                  </svg>
                </button>
                <button class="ai-act" onclick={() => handleAddToQueue(row)} aria-label="Add {row.track.title} to queue" use:tooltip={{ text: "Add to queue", placement: "top" }}>
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                    <line x1="8" y1="6" x2="21" y2="6" />
                    <line x1="8" y1="12" x2="21" y2="12" />
                    <line x1="8" y1="18" x2="21" y2="18" />
                    <line x1="3" y1="6" x2="3.01" y2="6" />
                    <line x1="3" y1="12" x2="3.01" y2="12" />
                    <line x1="3" y1="18" x2="3.01" y2="18" />
                  </svg>
                </button>
              {:else if row.state === "missing"}
                <span class="ai-check ai-check-missing" aria-hidden="true"></span>
                <span class="ai-row-missing-art" aria-hidden="true">?</span>
                <div class="ai-row-main">
                  <span class="ai-row-title">{row.sug.title}</span>
                  <span class="ai-row-artist">{row.sug.artist || "unknown artist"}</span>
                </div>
                <span class="ai-notfound">not found</span>
              {:else}
                <span class="ai-check ai-check-missing" aria-hidden="true"></span>
                <span class="ai-row-missing-art ai-art-spin" aria-hidden="true">
                  <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                    <path d="M21 12a9 9 0 1 1-6.2-8.56" />
                  </svg>
                </span>
                <div class="ai-row-main">
                  <span class="ai-row-title">{row.sug.title}</span>
                  <span class="ai-row-artist">{row.sug.artist || "searching..."}</span>
                </div>
              {/if}
            </div>
          {/each}
        </div>
      </div>
    {/if}
  </div>
</div>

<style>
  .ai-backdrop {
    position: fixed;
    inset: 0;
    z-index: 200;
    background: color-mix(in srgb, var(--bg-base) 65%, transparent);
    backdrop-filter: blur(6px);
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }

  .ai-modal {
    display: flex;
    flex-direction: column;
    width: min(560px, 100%);
    max-height: min(82vh, 720px);
    background: var(--bg-elevated);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.07));
    border-radius: var(--radius-lg);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.02) inset;
    overflow: hidden;
  }

  .ai-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 16px 16px 12px;
    border-bottom: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.06));
  }

  .ai-head-icon {
    width: 34px;
    height: 34px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
    display: grid;
    place-items: center;
    flex-shrink: 0;
  }

  .ai-head-icon svg {
    width: 18px;
    height: 18px;
  }

  .ai-head-text {
    flex: 1;
    min-width: 0;
  }

  .ai-title {
    margin: 0;
    font-size: 0.95rem;
    font-weight: 600;
    color: var(--text-primary);
  }

  .ai-sub {
    margin: 2px 0 0;
    font-size: 0.75rem;
    color: var(--text-muted);
  }

  .ai-close {
    width: 28px;
    height: 28px;
    display: grid;
    place-items: center;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    transition: color 150ms ease, background 150ms ease;
    flex-shrink: 0;
  }

  .ai-close:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }

  .ai-close svg { width: 14px; height: 14px; }

  .ai-body {
    padding: 14px 16px 4px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    overflow-y: auto;
  }

  .ai-label {
    font-size: 0.72rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
  }

  .ai-prompt,
  .ai-answer {
    width: 100%;
    resize: vertical;
    background: var(--bg-overlay);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.06));
    border-radius: var(--radius);
    color: var(--text-primary);
    font-family: inherit;
    font-size: 0.8rem;
    line-height: 1.5;
    padding: 8px 10px;
    outline: none;
    transition: border-color 200ms ease;
  }

  .ai-prompt {
    color: var(--text-secondary);
    min-height: 96px;
  }

  .ai-answer {
    min-height: 96px;
  }

  .ai-prompt:focus,
  .ai-answer:focus {
    border-color: var(--accent-dim);
  }

  .ai-foot {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 16px 14px;
  }

  .ai-progress {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    min-width: 0;
  }

  .ai-progress-bar {
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: var(--bg-overlay);
    overflow: hidden;
  }

  .ai-progress-fill {
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transition: width 240ms var(--ease-out-expo);
  }

  .ai-progress-label {
    font-size: 0.72rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }

  .ai-find {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 16px;
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--bg-base);
    font-size: 0.82rem;
    font-weight: 600;
    transition: transform 150ms var(--ease-spring), filter 150ms ease, opacity 150ms ease;
  }

  .ai-find:hover:not(:disabled) {
    filter: brightness(1.08);
    transform: scale(1.02);
  }

  .ai-find:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .ai-find svg { width: 14px; height: 14px; }

  .ai-cancel {
    padding: 8px 12px;
    border-radius: var(--radius);
    color: var(--text-muted);
    font-size: 0.82rem;
    transition: color 150ms ease, background 150ms ease;
  }

  .ai-cancel:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }

  .spin { animation: ai-spin 800ms linear infinite; }

  @keyframes ai-spin {
    to { transform: rotate(360deg); }
  }

  .ai-results {
    border-top: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.06));
    display: flex;
    flex-direction: column;
    min-height: 0;
    flex: 1;
  }

  .ai-results-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 10px 16px;
  }

  .ai-results-count {
    font-size: 0.75rem;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .ai-addall-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-shrink: 0;
  }

  .ai-addall svg {
    width: 13px;
    height: 13px;
  }

  .ai-addall-secondary {
    background: transparent;
    color: var(--accent-light);
    border: 1px solid color-mix(in srgb, var(--accent-dim) 70%, transparent);
  }

  .ai-addall-secondary:hover:not(:disabled) {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    color: var(--accent);
  }

  .ai-addall {
    padding: 5px 12px;
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--bg-base);
    font-size: 0.76rem;
    font-weight: 600;
    transition: filter 150ms ease, transform 150ms var(--ease-spring), opacity 150ms ease;
  }

  .ai-addall:hover:not(:disabled) {
    filter: brightness(1.08);
    transform: scale(1.02);
  }

  .ai-addall:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .ai-list {
    overflow-y: auto;
    padding: 0 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .ai-row {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px;
    border-radius: var(--radius);
    transition: background 160ms ease;
  }

  .ai-row:hover {
    background: var(--bg-overlay);
  }

  .ai-row:hover .ai-row-main {
    transform: translateX(2px);
  }

  .ai-row-main {
    transition: transform 180ms var(--ease-out-expo);
  }

  .ai-row.ai-pending {
    opacity: 0.6;
  }

  .ai-row.ai-missing {
    opacity: 0.45;
  }

  .ai-check {
    width: 18px;
    height: 18px;
    border-radius: 5px;
    border: 1.5px solid var(--text-muted);
    display: grid;
    place-items: center;
    color: var(--bg-base);
    flex-shrink: 0;
    transition: background 140ms ease, border-color 140ms ease;
  }

  .ai-check svg { width: 11px; height: 11px; }

  .ai-check.checked {
    background: var(--accent);
    border-color: var(--accent);
  }

  .ai-check-missing {
    border-style: dashed;
    opacity: 0.5;
  }

  .ai-row-missing-art {
    width: 38px;
    height: 38px;
    border-radius: var(--radius-sm);
    background: var(--bg-overlay);
    display: grid;
    place-items: center;
    color: var(--text-muted);
    font-size: 0.9rem;
    font-weight: 700;
    flex-shrink: 0;
  }

  .ai-art-spin svg {
    width: 14px;
    height: 14px;
    animation: ai-spin 800ms linear infinite;
  }

  .ai-row-main {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
    text-align: left;
  }

  .ai-row-title {
    font-size: 0.85rem;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ai-row-artist {
    font-size: 0.75rem;
    color: var(--text-secondary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .ai-quality {
    font-size: 0.68rem;
    color: var(--accent-light);
    border: 1px solid color-mix(in srgb, var(--accent-dim) 60%, transparent);
    border-radius: 999px;
    padding: 1px 7px;
    flex-shrink: 0;
  }

  .ai-notfound {
    font-size: 0.72rem;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .ai-act {
    width: 26px;
    height: 26px;
    display: grid;
    place-items: center;
    color: var(--text-muted);
    border-radius: var(--radius-sm);
    flex-shrink: 0;
    transition: color 150ms ease, background 150ms ease, transform 150ms ease;
    opacity: 0;
  }

  .ai-row:hover .ai-act,
  .ai-row:focus-within .ai-act {
    opacity: 1;
  }

  .ai-act:hover {
    color: var(--accent);
    background: var(--bg-overlay);
    transform: scale(1.1);
  }

  .ai-act svg { width: 13px; height: 13px; }
</style>
