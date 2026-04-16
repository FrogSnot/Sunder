<script lang="ts">
  import { onMount } from "svelte";
  import { nav, type Tab } from "../state/nav.svelte";
  import { checkForUpdates, openUrl } from "../ipc/bridge";

  const tabs: { id: Tab; label: string; icon: string }[] = [
    { id: "search", label: "Search", icon: "M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z" },
    { id: "explore", label: "Explore", icon: "M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5" },
    { id: "queue", label: "Queue", icon: "M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01" },
    { id: "playlists", label: "Playlists", icon: "M9 18V5l12-2v13M6 18a3 3 0 100-6 3 3 0 000 6zM18 16a3 3 0 100-6 3 3 0 000 6z" },
  ];

  let update = $state<{ version: string; url: string } | null>(null);

  onMount(() => {
    const timer = setTimeout(async () => {
      const info = await checkForUpdates();
      if (info.available && info.version && info.url) {
        update = { version: info.version, url: info.url };
      }
    }, 5000);
    return () => clearTimeout(timer);
  });

  function setTab(tab: Tab) {
    nav.activeTab = tab;
    if (tab !== "playlist-detail") {
      nav.activePlaylistId = null;
    }
  }

  function isActive(tab: Tab): boolean {
    if (tab === "playlists") return nav.activeTab === "playlists" || nav.activeTab === "playlist-detail";
    return nav.activeTab === tab;
  }

  let activeIndex = $derived(tabs.findIndex(t => isActive(t.id)));
</script>

<nav class="sidebar">
  <div class="brand">
    <span class="brand-icon">&#9830;</span>
    <span class="brand-name">Sunder</span>
  </div>

  <div class="nav-items">
    {#if activeIndex >= 0}
      <div class="nav-pill" style="transform: translateY({activeIndex * 42}px)"></div>
    {/if}
    {#each tabs as tab (tab.id)}
      <button
        class="nav-btn"
        class:active={isActive(tab.id)}
        onclick={() => setTab(tab.id)}
      >
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
          <path d={tab.icon} />
        </svg>
        <span>{tab.label}</span>
      </button>
    {/each}
  </div>

  {#if update}
    <button class="update-hint" onclick={() => update && openUrl(update.url)}>
      <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
        <path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" />
        <polyline points="7 10 12 15 17 10" />
        <line x1="12" y1="15" x2="12" y2="3" />
      </svg>
      <span>{update.version} available</span>
    </button>
  {/if}
</nav>

<style>
  .sidebar {
    width: 200px;
    min-width: 200px;
    max-width: 200px;
    height: 100vh;
    background: var(--bg-surface);
    border-right: 1px solid var(--bg-overlay);
    display: flex;
    flex-direction: column;
    flex-shrink: 0;
    padding: 16px 0;
    overflow-y: auto;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 0 20px 20px;
  }

  .brand-icon {
    font-size: 1.4rem;
    color: var(--accent);
    display: inline-block;
  }

  .brand-name {
    font-size: 1.1rem;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-primary);
  }

  .nav-items {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 0 8px;
    position: relative;
  }

  .nav-pill {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 40px;
    background: var(--bg-elevated);
    border-radius: var(--radius);
    border-left: 3px solid var(--accent);
    transition: transform 400ms var(--ease-out-expo);
    pointer-events: none;
  }

  .nav-btn {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 0 12px;
    height: 40px;
    border-radius: var(--radius);
    font-size: 0.9rem;
    font-weight: 500;
    color: var(--text-secondary);
    transition: color 200ms ease, transform 150ms ease;
    position: relative;
    z-index: 1;
  }

  .nav-btn:hover {
    color: var(--text-primary);
  }

  .nav-btn:active {
    transform: scale(0.97);
  }

  .nav-btn.active {
    color: var(--accent);
  }

  .nav-btn svg {
    width: 18px;
    height: 18px;
    flex-shrink: 0;
  }

  .update-hint {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: auto 12px 12px;
    padding: 8px 12px;
    font-size: 0.75rem;
    font-weight: 600;
    color: var(--accent);
    background: var(--bg-elevated);
    border-radius: var(--radius);
    cursor: pointer;
    transition: background 200ms ease, color 200ms ease;
  }

  .update-hint:hover {
    background: var(--accent);
    color: #121212;
  }

  .update-hint:active {
    opacity: 0.8;
  }

  .update-hint svg {
    width: 14px;
    height: 14px;
    flex-shrink: 0;
  }


</style>
