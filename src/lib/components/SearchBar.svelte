<script lang="ts">
  import { search, searchLocal } from "../ipc/bridge";
  import { searchState } from "../state/search.svelte";

  let debounceTimer: ReturnType<typeof setTimeout> | undefined;

  async function handleInput() {
    const q = searchState.query.trim();
    if (!q) {
      searchState.results = [];
      return;
    }

    try {
      const local = await searchLocal(q);
      if (local.length > 0) searchState.results = local;
    } catch {}

    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => remoteSearch(q), 400);
  }

  async function remoteSearch(q: string) {
    if (!q) return;
    searchState.searching = true;
    try {
      const res = await search(q);
      searchState.results = res.tracks;
    } catch (e) {
      console.error("search failed:", e);
    } finally {
      searchState.searching = false;
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      searchState.query = "";
      searchState.results = [];
    }
  }
</script>

<div class="search-bar">
  <svg class="search-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
    <circle cx="11" cy="11" r="8" />
    <line x1="21" y1="21" x2="16.65" y2="16.65" />
  </svg>
  <input
    type="text"
    placeholder="Search tracks..."
    bind:value={searchState.query}
    oninput={handleInput}
    onkeydown={handleKeydown}
  />
  {#if searchState.searching}
    <div class="dot-loader">
      <span></span><span></span><span></span>
    </div>
  {/if}
</div>

<style>
  .search-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    max-width: 520px;
    background: var(--bg-elevated);
    border-radius: var(--radius);
    padding: 6px 14px;
    transition: outline 200ms ease, box-shadow 300ms ease;
    outline: 2px solid transparent;
  }

  .search-bar:focus-within {
    outline-color: var(--accent-dim);
    box-shadow: 0 0 20px rgba(212, 160, 23, 0.12);
  }

  .search-icon {
    width: 18px;
    height: 18px;
    color: var(--text-muted);
    flex-shrink: 0;
    transition: color 200ms ease, transform 300ms var(--ease-spring);
  }

  .search-bar:focus-within .search-icon {
    color: var(--accent);
    transform: scale(1.1);
  }

  input {
    flex: 1;
    background: transparent;
    border: none;
    outline: none;
    font-size: 0.9rem;
    color: var(--text-primary);
  }

  input::placeholder {
    color: var(--text-muted);
  }

  .dot-loader {
    display: flex;
    gap: 4px;
    align-items: center;
  }

  .dot-loader span {
    width: 5px;
    height: 5px;
    border-radius: 50%;
    background: var(--accent);
    animation: dotPulse 1.2s ease-in-out infinite;
  }

  .dot-loader span:nth-child(2) { animation-delay: 0.15s; }
  .dot-loader span:nth-child(3) { animation-delay: 0.3s; }
</style>
