<script lang="ts">
  // Ad-hoc keyboard send — WriteLog Alt-K / N1MM Ctrl-K style.
  //
  // Word and Char modes go on the air as you type: the first keystroke keys
  // up and the rig diddles (LTRS idles) until there's text to send, so the
  // other station knows at once you're still there.
  //   Word — each word is sent when you hit Space (you can fix a word until
  //          then). Default.
  //   Char — each key is sent as it's typed (Backspace can't recall it).
  //   Line — the old way: compose, then Enter sends the whole line.
  // Enter sends whatever is left and unkeys once it's out; Esc aborts on the
  // spot. F-keys pressed meanwhile join the same transmission. Macro tokens
  // (<MYCALL>, <CALL>, <SERIAL>) expand in Word and Line modes.
  import { macroState } from "$lib/macros.svelte";

  type Mode = "word" | "char" | "line";
  const MODE_KEY = "diddle.adhocMode";

  let open = $state(false);
  let text = $state("");
  let inputEl = $state<HTMLInputElement | undefined>(undefined);
  let mode = $state<Mode>(loadMode());
  // What this live transmission has sent so far, for the "sent" line.
  let sentLine = $state("");

  function loadMode(): Mode {
    try {
      const m = localStorage.getItem(MODE_KEY);
      if (m === "word" || m === "char" || m === "line") return m;
    } catch {}
    return "word";
  }

  function setMode(m: Mode) {
    mode = m;
    try {
      localStorage.setItem(MODE_KEY, m);
    } catch {}
    inputEl?.focus();
  }

  // Session-only history of previous ad-hoc sends, newest first, recalled
  // with ArrowUp/ArrowDown like a shell.
  let history: string[] = [];
  let histIdx = -1;

  let expanded = $derived(macroState.expand(text));

  function remember(t: string) {
    t = t.trim();
    if (!t) return;
    if (history[0] !== t) history.unshift(t);
    if (history.length > 20) history.pop();
    histIdx = -1;
  }

  function toggle() {
    if (open && macroState.live) finish(); // closing mid-send finishes it
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

  // Put text on the live transmission, keying up first if we aren't yet.
  function goLive(s: string) {
    if (!macroState.live) {
      if (macroState.txing) return; // a macro is on the air; wait for it
      sentLine = "";
      macroState.startLive(s);
    } else {
      macroState.pushLive(s);
    }
    sentLine += s;
  }

  function finish() {
    if (text) goLive(mode === "char" ? text : macroState.expand(text));
    text = "";
    if (inputEl) inputEl.value = "";
    if (macroState.live) {
      remember(sentLine);
      macroState.finishLive();
    }
  }

  async function sendLine() {
    const t = text.trim();
    if (t.length === 0 || macroState.onAir) return;
    remember(t);
    text = "";
    await macroState.send(t);
  }

  function onInput(e: Event) {
    const el = e.target as HTMLInputElement;
    // RTTY is Baudot — uppercase as typed, like the Call field.
    const v = el.value.toUpperCase();
    if (mode === "line") {
      text = v;
      return;
    }
    if (mode === "char") {
      // Everything typed goes straight out; the field stays empty.
      el.value = "";
      text = "";
      if (v) goLive(v);
      return;
    }
    // Word: send up to the last space, keep the word being typed.
    const cut = v.lastIndexOf(" ");
    if (cut >= 0) {
      goLive(macroState.expand(v.slice(0, cut + 1)));
      text = v.slice(cut + 1);
      el.value = text;
    } else {
      text = v;
      if (v && !macroState.live) goLive(""); // key up on the first keystroke
    }
  }

  function onInputKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      if (mode === "line" && !macroState.live) sendLine();
      else finish();
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (macroState.onAir) {
        // Abort on the spot; keep the window open. (The global F-keys
        // handler also aborts; abort is idempotent.)
        macroState.abort();
        text = "";
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

  const PLACEHOLDER: Record<Mode, string> = {
    word: "TYPE — KEYS UP AT ONCE, EACH WORD GOES ON SPACE…",
    char: "TYPE — EACH KEY GOES ON THE AIR AS TYPED…",
    line: "TYPE AND HIT ENTER TO TRANSMIT…",
  };

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
        <div class="modes" role="group" aria-label="Send mode">
          {#each ["word", "char", "line"] as const as m}
            <button
              type="button"
              class:on={mode === m}
              onclick={() => setMode(m)}
              title={m === "word"
                ? "Live: each word goes out when you press Space; diddles fill the gaps"
                : m === "char"
                  ? "Live: each key goes out as you type it; diddles fill the gaps"
                  : "Compose the whole line, Enter sends it"}
            >{m}</button>
          {/each}
        </div>
        {#if macroState.onAir}
          <span class="tx-indicator">● {macroState.live ? "LIVE" : "TX"}</span>
        {/if}
        <span class="hint">
          <span class="kbd">↵</span> {mode === "line" && !macroState.live ? "send" : "end"} ·
          <span class="kbd">esc</span> {macroState.onAir ? "abort" : "close"} ·
          <span class="kbd">↑</span> history
        </span>
      </header>
      <input
        bind:this={inputEl}
        value={text}
        oninput={onInput}
        onkeydown={onInputKey}
        placeholder={PLACEHOLDER[mode]}
        spellcheck="false"
        autocomplete="off"
      />
      {#if mode !== "char" && expanded !== text && text.trim().length > 0}
        <div class="preview"><span class="dim">will send:</span> {expanded}</div>
      {/if}
      {#if sentLine && (macroState.live || mode !== "line")}
        <div class="preview"><span class="dim">{macroState.live ? "on air:" : "sent:"}</span> {sentLine}</div>
      {:else if macroState.lastSent}
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

  .modes { display: flex; gap: 4px; }
  .modes button {
    background: transparent;
    border: 1px solid #3a4452;
    color: #8a949d;
    border-radius: 3px;
    padding: 1px 8px;
    font-size: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    text-transform: uppercase;
    cursor: pointer;
  }
  .modes button:hover { color: #c5d1de; }
  .modes button.on { border-color: #4a90e2; color: #92c5fa; background: #1c2a3a; }

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
