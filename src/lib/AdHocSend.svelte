<script lang="ts">
  // Ad-hoc keyboard send — WriteLog Alt-K / N1MM Ctrl-K style. A popup where
  // the operator types free text and Enter puts it on the air. The window
  // stays open for repeated sends until ESC (or the shortcut again) closes it.
  //
  // The backend synthesizes each message's waveform up-front, so this is a
  // compose-then-send line buffer, not char-at-a-time streaming — a natural
  // fit for RTTY anyway. Macro tokens (<MYCALL>, <CALL>, <SERIAL>) expand.
  import { macroState } from "$lib/macros.svelte";

  let open = $state(false);
  let text = $state("");
  let inputEl = $state<HTMLInputElement | undefined>(undefined);

  // Session-only history of previous ad-hoc sends, newest first, recalled
  // with ArrowUp/ArrowDown like a shell.
  let history: string[] = [];
  let histIdx = -1;

  let expanded = $derived(macroState.expand(text));

  function toggle() {
    open = !open;
    if (open) histIdx = -1;
  }

  // Focus lands on the input whenever the popup opens (the effect runs
  // after the {#if} block has rendered, unlike a microtask).
  $effect(() => {
    if (open) inputEl?.focus();
  });

  function onGlobalKey(e: KeyboardEvent) {
    // Ctrl-K (N1MM), Alt-K (WriteLog), Cmd-K. e.code so macOS Alt dead-keys
    // don't hide the shortcut.
    if (e.code === "KeyK" && (e.ctrlKey || e.altKey || e.metaKey)) {
      e.preventDefault();
      toggle();
    }
  }

  async function sendNow() {
    const t = text.trim();
    if (t.length === 0 || macroState.txing) return;
    if (history[0] !== t) history.unshift(t);
    if (history.length > 20) history.pop();
    histIdx = -1;
    text = "";
    await macroState.send(t);
  }

  function onInputKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      sendNow();
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (macroState.txing) {
        // Let ESC mean "abort" while on the air (the global F-keys handler
        // also aborts; abort is idempotent). Keep the window open.
        macroState.abort();
      } else {
        open = false;
      }
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      if (history.length === 0) return;
      histIdx = Math.min(histIdx + 1, history.length - 1);
      text = history[histIdx];
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      if (histIdx <= 0) {
        histIdx = -1;
        text = "";
      } else {
        histIdx -= 1;
        text = history[histIdx];
      }
    }
  }

  function onInput(e: Event) {
    // RTTY is Baudot — uppercase as typed, like the Call field.
    text = (e.target as HTMLInputElement).value.toUpperCase();
  }

  $effect(() => {
    window.addEventListener("keydown", onGlobalKey);
    return () => window.removeEventListener("keydown", onGlobalKey);
  });
</script>

{#if open}
  <div class="overlay">
    <div class="box" role="dialog" aria-label="Ad-hoc send">
      <header>
        <h2>Ad-hoc send</h2>
        {#if macroState.txing}
          <span class="tx-indicator">● TX</span>
        {/if}
        <span class="hint">
          <span class="kbd">↵</span> send ·
          <span class="kbd">esc</span> {macroState.txing ? "abort" : "close"} ·
          <span class="kbd">↑</span> history
        </span>
      </header>
      <input
        bind:this={inputEl}
        value={text}
        oninput={onInput}
        onkeydown={onInputKey}
        placeholder="TYPE AND HIT ENTER TO TRANSMIT…"
        spellcheck="false"
        autocomplete="off"
      />
      {#if expanded !== text && text.trim().length > 0}
        <div class="preview"><span class="dim">will send:</span> {expanded}</div>
      {/if}
      {#if macroState.lastSent}
        <div class="preview"><span class="dim">sent:</span> {macroState.lastSent}</div>
      {/if}
      {#if macroState.lastError}
        <div class="err">{macroState.lastError}</div>
      {/if}
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    z-index: 100;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 18vh;
    background: rgba(0, 0, 0, 0.45);
  }

  .box {
    width: min(640px, 90vw);
    background: #181c1f;
    border: 1px solid #3a5a8a;
    border-radius: 8px;
    padding: 12px 16px;
    box-shadow: 0 12px 40px rgba(0, 0, 0, 0.6);
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 10px;
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

  .hint {
    margin-left: auto;
    color: #6b7176;
    font-size: 11px;
  }

  .kbd {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    border: 1px solid #3a4452;
    padding: 0 4px;
    border-radius: 2px;
    background: rgba(0, 0, 0, 0.25);
    color: #8a949d;
    font-size: 10px;
  }

  .tx-indicator {
    color: #f87171;
    font-weight: 700;
    font-size: 12px;
    letter-spacing: 1px;
    animation: pulse 0.8s infinite;
  }
  @keyframes pulse { 50% { opacity: 0.4; } }

  input {
    width: 100%;
    box-sizing: border-box;
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 10px 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 18px;
    font-weight: 500;
    letter-spacing: 0.5px;
  }
  input:focus {
    outline: none;
    border-color: #4a90e2;
    background: #0e1418;
  }

  .preview {
    margin-top: 8px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    color: #c5d1de;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .dim { color: #6b7176; font-size: 10px; text-transform: uppercase; }

  .err {
    margin-top: 6px;
    color: #f87171;
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
</style>
