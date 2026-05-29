<script lang="ts">
  import { fly, scale } from "svelte/transition";
  import { flip } from "svelte/animate";
  import { quintOut } from "svelte/easing";
  import { downloads } from "../state/downloads.svelte";
  import { downloadTrack } from "../ipc/bridge";

  interface ActiveItem {
    id: string;
    title: string;
    artist: string;
    thumbnail: string;
    status: string;
    percent: number;
  }

  let active = $derived.by<ActiveItem[]>(() => {
    const out: ActiveItem[] = [];
    for (const [id, p] of downloads.progress) {
      const m = downloads.getMeta(id);
      out.push({
        id,
        title: m?.title ?? "Track",
        artist: m?.artist ?? "",
        thumbnail: m?.thumbnail ?? "",
        status: p.status,
        percent: p.percent,
      });
    }
    return out;
  });

  let failed = $derived.by<ActiveItem[]>(() => {
    const out: ActiveItem[] = [];
    for (const id of downloads.failedIds) {
      const m = downloads.getMeta(id);
      out.push({
        id,
        title: m?.title ?? "Track",
        artist: m?.artist ?? "",
        thumbnail: m?.thumbnail ?? "",
        status: "error",
        percent: 0,
      });
    }
    return out;
  });

  let count = $derived(active.length);
  let total = $derived(downloads.sessionTotal);
  let doneCount = $derived(downloads.sessionDone);
  let failCount = $derived(downloads.sessionFailed);
  let overall = $derived(downloads.sessionPercent);
  let collapsed = $state(false);

  let finished = $state(false);
  let prevCount = 0;
  let finishTimer: ReturnType<typeof setTimeout> | null = null;

  $effect(() => {
    const c = count;
    if (c === 0 && prevCount > 0) {
      finished = true;
      if (finishTimer) clearTimeout(finishTimer);
      finishTimer = setTimeout(() => {
        finished = false;
      }, 4200);
    } else if (c > 0) {
      finished = false;
      if (finishTimer) {
        clearTimeout(finishTimer);
        finishTimer = null;
      }
    }
    prevCount = c;
  });

  let summaryFailed = $derived(finished && count === 0 && failCount > 0);

  function statusLabel(s: string, pct: number): string {
    switch (s) {
      case "queued":
        return "Queued";
      case "downloading":
        return `Downloading ${Math.round(pct)}%`;
      case "converting":
        return "Converting";
      default:
        return "Preparing";
    }
  }

  async function retry(id: string) {
    const t = downloads.getMeta(id);
    if (!t) return;
    downloads.clearFailed(id);
    downloads.register(t);
    try {
      await downloadTrack(t.id);
    } catch {
      // failure is re-surfaced through the download event stream
    }
  }
</script>

{#if count > 0 || finished}
  <section
    class="dl-activity"
    class:done={finished && count === 0 && !summaryFailed}
    class:has-error={summaryFailed}
    transition:fly={{ y: 24, duration: 320, easing: quintOut }}
    aria-live="polite"
  >
    <header class="dl-head">
      <span class="dl-head-icon">
        {#if summaryFailed}
          <svg class="warn-mark" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9" /><line x1="12" y1="8" x2="12" y2="12.5" /><line x1="12" y1="16" x2="12" y2="16" /></svg>
        {:else if finished && count === 0}
          <svg class="check-mark" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"><polyline points="20 6 9 17 4 12" /></svg>
        {:else}
          <span class="dl-orbit"><i></i></span>
        {/if}
      </span>
      <div class="dl-head-text">
        <span class="dl-title">
          {#if finished && count === 0}
            {summaryFailed ? "Downloads finished" : "All downloads complete"}
          {:else}
            Downloading {total} {total === 1 ? "track" : "tracks"}
          {/if}
        </span>
        <span class="dl-sub">
          {#if finished && count === 0}
            {doneCount} saved{#if failCount > 0} · {failCount} failed{/if}
          {:else}
            {doneCount}/{total} · {overall}%{#if failCount > 0} · {failCount} failed{/if}
          {/if}
        </span>
      </div>
      {#if count > 0}
        <button
          class="dl-collapse"
          onclick={() => (collapsed = !collapsed)}
          aria-label={collapsed ? "Expand" : "Collapse"}
        >
          <svg class:flip={collapsed} viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" stroke-linejoin="round"><polyline points="6 9 12 15 18 9" /></svg>
        </button>
      {/if}
    </header>

    {#if count > 0}
      <div class="dl-track-bar"><div class="dl-track-fill" style="width: {overall}%"></div></div>
    {/if}

    {#if !collapsed && (count > 0 || failed.length > 0)}
      <ul class="dl-list">
        {#each active as item (item.id)}
          <li class="dl-item" animate:flip={{ duration: 260 }} transition:scale={{ start: 0.92, duration: 220 }}>
            <div class="dl-thumb-wrap">
              {#if item.thumbnail}
                <img class="dl-thumb" src={item.thumbnail} alt="" />
              {:else}
                <span class="dl-thumb dl-thumb-empty"></span>
              {/if}
              <span class="dl-thumb-veil" class:converting={item.status === "converting"}></span>
            </div>
            <div class="dl-meta">
              <span class="dl-name">{item.title}</span>
              <span class="dl-state">{statusLabel(item.status, item.percent)}</span>
            </div>
            <div class="dl-mini">
              <div
                class="dl-mini-fill"
                class:indeterminate={item.status !== "downloading"}
                style="width: {item.status === 'downloading' ? item.percent : 100}%"
              ></div>
            </div>
          </li>
        {/each}
        {#each failed as item (item.id)}
          <li class="dl-item dl-item-error" animate:flip={{ duration: 260 }} transition:scale={{ start: 0.92, duration: 220 }}>
            <div class="dl-thumb-wrap">
              {#if item.thumbnail}
                <img class="dl-thumb" src={item.thumbnail} alt="" />
              {:else}
                <span class="dl-thumb dl-thumb-empty"></span>
              {/if}
              <span class="dl-thumb-veil error"></span>
            </div>
            <div class="dl-meta">
              <span class="dl-name">{item.title}</span>
              <span class="dl-state error">Failed</span>
            </div>
            <button class="dl-retry" onclick={() => retry(item.id)} aria-label="Retry download">
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" stroke-linejoin="round"><polyline points="23 4 23 10 17 10" /><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10" /></svg>
            </button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
{/if}

<style>
  .dl-activity {
    width: 100%;
    background: color-mix(in srgb, var(--bg-elevated) 92%, transparent);
    backdrop-filter: blur(18px) saturate(150%);
    border: 1px solid var(--border-subtle, rgba(255, 255, 255, 0.07));
    border-radius: var(--radius-lg);
    box-shadow: 0 18px 48px rgba(0, 0, 0, 0.5), 0 0 0 1px rgba(255, 255, 255, 0.02) inset;
    pointer-events: auto;
    overflow: hidden;
  }

  .dl-activity.done {
    border-color: color-mix(in srgb, var(--success) 40%, transparent);
  }

  .dl-activity.has-error {
    border-color: color-mix(in srgb, var(--error) 45%, transparent);
  }

  .has-error .dl-head-icon {
    background: color-mix(in srgb, var(--error) 18%, transparent);
    color: var(--error);
  }

  .warn-mark {
    width: 18px;
    height: 18px;
    animation: scaleIn 320ms var(--ease-out-expo) both;
  }

  .dl-head {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 14px 12px;
  }

  .dl-head-icon {
    width: 34px;
    height: 34px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 50%;
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    color: var(--accent);
  }

  .done .dl-head-icon {
    background: color-mix(in srgb, var(--success) 18%, transparent);
    color: var(--success);
  }

  .dl-orbit {
    width: 18px;
    height: 18px;
    position: relative;
    display: block;
  }

  .dl-orbit::before {
    content: "";
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 2px solid color-mix(in srgb, var(--accent) 30%, transparent);
  }

  .dl-orbit i {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 2px solid transparent;
    border-top-color: var(--accent);
    animation: dl-orbit 850ms linear infinite;
  }

  .check-mark {
    width: 17px;
    height: 17px;
    stroke-dasharray: 24;
    stroke-dashoffset: 24;
    animation: check-draw 460ms var(--ease-out-expo) forwards;
  }

  .dl-head-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .dl-title {
    font-size: 0.85rem;
    font-weight: 600;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dl-sub {
    font-size: 0.72rem;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .dl-collapse {
    width: 26px;
    height: 26px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 8px;
    color: var(--text-muted);
    transition: color 160ms ease, background 160ms ease;
  }

  .dl-collapse:hover {
    color: var(--text-primary);
    background: var(--bg-overlay);
  }

  .dl-collapse svg {
    width: 15px;
    height: 15px;
    transition: transform 220ms var(--ease-out-expo);
  }

  .dl-collapse svg.flip {
    transform: rotate(-180deg);
  }

  .dl-track-bar {
    height: 3px;
    background: var(--bg-overlay);
    overflow: hidden;
  }

  .dl-track-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), var(--accent-light));
    border-radius: 0 2px 2px 0;
    transition: width 320ms var(--ease-out-expo);
  }

  .dl-list {
    list-style: none;
    margin: 0;
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
    max-height: 256px;
    overflow-y: auto;
  }

  .dl-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px;
    border-radius: var(--radius);
    transition: background 160ms ease;
  }

  .dl-item:hover {
    background: var(--bg-overlay);
  }

  .dl-thumb-wrap {
    position: relative;
    width: 38px;
    height: 38px;
    flex-shrink: 0;
  }

  .dl-thumb {
    width: 100%;
    height: 100%;
    border-radius: var(--radius-sm);
    object-fit: cover;
    background: var(--bg-overlay);
    display: block;
  }

  .dl-thumb-empty {
    background: linear-gradient(135deg, var(--bg-overlay), var(--bg-surface));
  }

  .dl-thumb-veil {
    position: absolute;
    inset: 0;
    border-radius: var(--radius-sm);
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 55%, transparent) inset;
    opacity: 0;
  }

  .dl-thumb-veil.converting {
    opacity: 1;
    animation: dl-pulse 1.1s ease-in-out infinite;
  }

  .dl-thumb-veil.error {
    opacity: 1;
    box-shadow: 0 0 0 2px color-mix(in srgb, var(--error) 60%, transparent) inset;
  }

  .dl-item-error .dl-name {
    color: var(--text-secondary);
  }

  .dl-state.error {
    color: var(--error);
    font-weight: 600;
  }

  .dl-retry {
    width: 28px;
    height: 28px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 8px;
    color: var(--text-muted);
    transition: color 160ms ease, background 160ms ease, transform 200ms var(--ease-out-expo);
  }

  .dl-retry:hover {
    color: var(--accent-light);
    background: var(--bg-overlay);
    transform: rotate(-90deg);
  }

  .dl-retry svg {
    width: 15px;
    height: 15px;
  }

  .dl-meta {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .dl-name {
    font-size: 0.8rem;
    font-weight: 500;
    color: var(--text-primary);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dl-state {
    font-size: 0.7rem;
    color: var(--text-secondary);
    font-variant-numeric: tabular-nums;
  }

  .dl-mini {
    width: 46px;
    height: 4px;
    flex-shrink: 0;
    border-radius: 3px;
    background: var(--bg-overlay);
    overflow: hidden;
  }

  .dl-mini-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 280ms var(--ease-out-expo);
  }

  .dl-mini-fill.indeterminate {
    width: 40% !important;
    background: linear-gradient(90deg, transparent, var(--accent), transparent);
    animation: dl-slide 1.1s ease-in-out infinite;
  }

  @keyframes dl-orbit {
    to { transform: rotate(360deg); }
  }

  @keyframes check-draw {
    to { stroke-dashoffset: 0; }
  }

  @keyframes dl-pulse {
    0%, 100% { opacity: 0.35; }
    50% { opacity: 1; }
  }

  @keyframes dl-slide {
    0% { transform: translateX(-120%); }
    100% { transform: translateX(280%); }
  }
</style>
