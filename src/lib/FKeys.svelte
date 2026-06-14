<script lang="ts">
  import { onMount } from "svelte";
  import { settings } from "$lib/settings.svelte";
  import { macroState } from "$lib/macros.svelte";

  onMount(() => settings.load());

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape" && macroState.txing) {
      // ESC during TX aborts the in-flight transmission and drops PTT.
      e.preventDefault();
      macroState.abort();
      return;
    }
    if (!/^F[1-8]$/.test(e.key)) return;
    e.preventDefault();
    macroState.fire(e.key);
  }

  $effect(() => {
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });
</script>

<section class="panel">
  <header>
    <h2>F-keys <span class="dim">(macros)</span></h2>
    <div class="settings">
      <label>
        my call
        <input
          type="text"
          value={settings.myCall}
          oninput={(e) => settings.setMyCall((e.target as HTMLInputElement).value)}
          placeholder="W1AW"
          spellcheck="false"
        />
      </label>
      {#if macroState.txing}
        <span class="tx-indicator">● TX</span>
        <button
          class="abort"
          onclick={() => macroState.abort()}
          title="Abort transmission (ESC)"
        >
          Abort
        </button>
      {/if}
    </div>
  </header>

  <div class="grid">
    {#each macroState.macros as m}
      <button
        onclick={() => macroState.fire(m.key)}
        title={macroState.expand(m.text)}
        disabled={macroState.txing}
      >
        <span class="key">{m.key}</span>
        <span class="lbl">{m.label}</span>
      </button>
    {/each}
  </div>

  {#if macroState.lastSent}
    <div class="last-sent">
      <span class="dim">sent:</span>
      <span class="mono">{macroState.lastSent}</span>
    </div>
  {/if}
  {#if macroState.lastError}
    <div class="err">{macroState.lastError}</div>
  {/if}
</section>

<style>
  .panel {
    background: #181c1f;
    border: 1px solid #262b30;
    border-radius: 8px;
    padding: 12px 16px;
    margin-bottom: 12px;
  }

  header {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 8px;
  }

  h2 {
    margin: 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: #8a949d;
    font-weight: 600;
  }
  .dim { color: #6b7176; font-weight: 400; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }

  .settings {
    display: flex;
    align-items: center;
    gap: 12px;
    margin-left: auto;
    font-size: 11px;
    color: #8a949d;
  }
  .settings label {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .settings input {
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 3px 8px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    text-transform: uppercase;
    width: 110px;
  }
  .settings input:focus { outline: none; border-color: #4a90e2; }

  .tx-indicator {
    color: #f87171;
    font-weight: 700;
    letter-spacing: 1px;
    animation: pulse 0.8s infinite;
  }
  @keyframes pulse { 50% { opacity: 0.4; } }

  button.abort {
    background: #4a1f1f;
    border: 1px solid #f87171;
    color: #f87171;
    padding: 3px 12px;
    border-radius: 3px;
    cursor: pointer;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.5px;
  }
  button.abort:hover {
    background: #f87171;
    color: #1a0606;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(8, 1fr);
    gap: 6px;
  }

  button {
    background: #1f2630;
    border: 1px solid #2e3a4a;
    color: #c5d1de;
    border-radius: 4px;
    padding: 8px 10px;
    cursor: pointer;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }
  button:hover:not(:disabled) {
    background: #2a3445;
    border-color: #3a5a8a;
  }
  button:disabled { opacity: 0.4; cursor: not-allowed; }

  .key { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 10px; color: #6b7176; letter-spacing: 0.5px; }
  .lbl { font-size: 12px; font-weight: 600; }

  .last-sent {
    margin-top: 8px;
    font-size: 11px;
    display: flex;
    align-items: baseline;
    gap: 6px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; color: #c5d1de; }

  .err {
    margin-top: 6px;
    color: #f87171;
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
</style>
