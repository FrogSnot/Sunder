<script lang="ts">
  import { config } from "../state/config.svelte.ts";
  import { fade, fly } from "svelte/transition";

  const qualities = [
    { value: 0, label: "Best (256kbps+)" },
    { value: 2, label: "High (192kbps)" },
    { value: 5, label: "Medium (128kbps)" },
    { value: 7, label: "Low (96kbps)" },
  ];

  const languages = [
    { id: "en", name: "English" },
    { id: "es", name: "Spanish" },
    { id: "fr", name: "French" },
    { id: "de", name: "German" },
    { id: "it", name: "Italian" },
    { id: "ja", name: "Japanese" },
    { id: "ko", name: "Korean" },
    { id: "tr", name: "Turkish" },
  ];

  let savedMessage = $state(false);

  function triggerSave() {
    config.save();
    savedMessage = true;
    setTimeout(() => { savedMessage = false; }, 2000);
  }
</script>

<div class="settings-view" in:fade={{ duration: 250 }}>
  <header class="header">
    <h1>Settings</h1>
    <p class="subtitle">Personalize your Sunder experience</p>
  </header>

  <div class="sections">
    <!-- Playback Section -->
    <section class="section">
      <div class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M5 3l14 9-14 9V3z" />
        </svg>
        <h2>Playback</h2>
      </div>
      
      <div class="field">
        <div class="field-info">
          <label for="seek_step">Seek Step</label>
          <span class="desc">Seconds to skip when using arrow keys</span>
        </div>
        <div class="field-control">
          <input 
            type="number" 
            id="seek_step"
            bind:value={config.current.seek_step_secs} 
            onchange={triggerSave}
            min="1" max="60"
          />
          <span class="unit">s</span>
        </div>
      </div>

      <div class="field">
        <div class="field-info">
          <label for="vol_step">Volume Step</label>
          <span class="desc">Increment amount for volume changes</span>
        </div>
        <div class="field-control">
          <input 
            type="number" 
            id="vol_step"
            value={Math.round(config.current.volume_step * 100)} 
            onchange={(e) => {
              config.update({ volume_step: Number(e.currentTarget.value) / 100 });
            }}
            min="1" max="50"
          />
          <span class="unit">%</span>
        </div>
      </div>

      <div class="field">
        <div class="field-info">
          <label for="prefetch">Prefetch Distance</label>
          <span class="desc">How many upcoming tracks to pre-download</span>
        </div>
        <div class="field-control">
          <input 
            type="number" 
            id="prefetch"
            bind:value={config.current.prefetch_ahead} 
            onchange={triggerSave}
            min="0" max="5"
          />
        </div>
      </div>
    </section>

    <!-- Content Section -->
    <section class="section">
      <div class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <circle cx="11" cy="11" r="8" /><path d="M21 21l-4.35-4.35" />
        </svg>
        <h2>Content & Quality</h2>
      </div>

      <div class="field">
        <div class="field-info">
          <label for="quality">Audio Quality</label>
          <span class="desc">Lower is better quality (yt-dlp rank)</span>
        </div>
        <select id="quality" bind:value={config.current.audio_quality} onchange={triggerSave}>
          {#each qualities as q}
            <option value={q.value}>{q.label}</option>
          {/each}
        </select>
      </div>

      <div class="field">
        <div class="field-info">
          <label for="limit">Search Limit</label>
          <span class="desc">Number of results per source</span>
        </div>
        <input 
          type="range" 
          id="limit"
          bind:value={config.current.search_limit} 
          onchange={triggerSave}
          min="5" max="30" step="5"
        />
        <span class="value">{config.current.search_limit}</span>
      </div>
    </section>

    <!-- Features Section -->
    <section class="section">
      <div class="section-title">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <path d="M21 15a2 2 0 01-2 2H7l-4 4V5a2 2 0 012-2h14a2 2 0 012 2z" />
        </svg>
        <h2>Features</h2>
      </div>

      <div class="field">
        <div class="field-info">
          <label for="lyrics">Auto-Fetch Lyrics</label>
          <span class="desc">Automatically find lyrics when a song starts</span>
        </div>
        <div class="toggle-wrapper">
          <input 
            type="checkbox" 
            id="lyrics"
            bind:checked={config.current.lyrics_auto_fetch} 
            onchange={triggerSave}
          />
        </div>
      </div>

      <div class="field">
        <div class="field-info">
          <label for="lang">Subtitle Language</label>
          <span class="desc">Preferred language for YouTube transcripts</span>
        </div>
        <select id="lang" bind:value={config.current.subtitle_lang} onchange={triggerSave}>
          {#each languages as l}
            <option value={l.id}>{l.name}</option>
          {/each}
        </select>
      </div>
    </section>
  </div>

  {#if savedMessage}
    <div class="toast" transition:fly={{ y: 20 }}>
      Settings auto-saved
    </div>
  {/if}
</div>

<style>
  .settings-view {
    max-width: 800px;
    margin: 0 auto;
    padding-bottom: 40px;
  }

  .header {
    margin-bottom: 40px;
  }

  h1 {
    font-size: 2.5rem;
    font-weight: 800;
    margin-bottom: 8px;
    background: linear-gradient(135deg, var(--text-primary) 0%, var(--accent) 100%);
    background-clip: text;
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
  }

  .subtitle {
    color: var(--text-secondary);
    font-size: 1.1rem;
  }

  .sections {
    display: flex;
    flex-direction: column;
    gap: 32px;
  }

  .section {
    background: var(--bg-surface);
    border: 1px solid var(--bg-overlay);
    border-radius: var(--radius-lg);
    padding: 24px;
    display: flex;
    flex-direction: column;
    gap: 20px;
    transition: transform 200ms ease;
  }

  .section:hover {
    border-color: var(--accent-dim);
  }

  .section-title {
    display: flex;
    align-items: center;
    gap: 12px;
    color: var(--accent);
    margin-bottom: 8px;
  }

  .section-title h2 {
    font-size: 1.2rem;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .section-title svg {
    width: 20px;
    height: 20px;
  }

  .field {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 12px 0;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
  }

  .field:last-child {
    border-bottom: none;
  }

  .field-info {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  label {
    font-weight: 600;
    color: var(--text-primary);
    cursor: pointer;
  }

  .desc {
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .field-control {
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--bg-base);
    padding: 4px 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--bg-overlay);
  }

  input[type="number"] {
    width: 50px;
    background: transparent;
    border: none;
    outline: none;
    text-align: right;
    font-weight: 600;
    color: var(--accent);
  }

  .unit {
    color: var(--text-muted);
    font-size: 0.8rem;
    font-weight: 600;
  }

  select {
    background: var(--bg-base);
    border: 1px solid var(--bg-overlay);
    color: var(--text-primary);
    padding: 8px 12px;
    border-radius: var(--radius-sm);
    outline: none;
    cursor: pointer;
    font-weight: 500;
  }

  select:focus {
    border-color: var(--accent);
  }

  input[type="range"] {
    width: 150px;
    accent-color: var(--accent);
  }

  .value {
    min-width: 24px;
    font-weight: 700;
    color: var(--accent);
  }

  .toggle-wrapper input {
    width: 40px;
    height: 20px;
    appearance: none;
    background: var(--bg-overlay);
    border-radius: 20px;
    position: relative;
    cursor: pointer;
    transition: background 200ms ease;
  }

  .toggle-wrapper input:checked {
    background: var(--accent);
  }

  .toggle-wrapper input::before {
    content: "";
    position: absolute;
    width: 14px;
    height: 14px;
    background: white;
    border-radius: 50%;
    top: 3px;
    left: 3px;
    transition: transform 200ms ease;
  }

  .toggle-wrapper input:checked::before {
    transform: translateX(20px);
  }

  .toast {
    position: fixed;
    bottom: 100px;
    left: 50%;
    transform: translateX(-50%);
    background: var(--accent);
    color: black;
    padding: 8px 24px;
    border-radius: 20px;
    font-weight: 700;
    font-size: 0.9rem;
    box-shadow: 0 4px 12px rgba(0,0,0,0.3);
    z-index: 1000;
  }
</style>
