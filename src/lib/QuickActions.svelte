<script lang="ts">
  // Log quick actions on global shortcuts:
  //   Ctrl+N — attach/edit a note on a logged QSO (newest by default)
  //   Ctrl+Q — quick-edit a logged callsign (newest by default)
  // ↑/↓ steps back/forward through the log while either popup is open
  // (Ctrl+Q again also steps older, N1MM-style). Enter saves, Esc cancels.
  // Cmd combos are never claimed (Cmd+N/Q belong to the OS).
  import { qsoLog } from "$lib/qsoLog.svelte";

  type Mode = "note" | "edit";
  let mode = $state<Mode | null>(null);
  let text = $state("");
  // Offset from the newest QSO: 0 = most recent, 1 = one before, …
  let idx = $state(0);
  let inputEl = $state<HTMLInputElement | undefined>(undefined);

  let target = $derived(
    mode !== null && qsoLog.qsos.length > 0
      ? qsoLog.qsos[qsoLog.qsos.length - 1 - idx]
      : undefined,
  );

  function open(m: Mode) {
    if (qsoLog.qsos.length === 0) return;
    mode = m;
    idx = 0;
    loadText();
  }

  function loadText() {
    const q = qsoLog.qsos[qsoLog.qsos.length - 1 - idx];
    if (!q) return;
    text = mode === "note" ? (q.note ?? "") : q.call;
  }

  function step(d: number) {
    const n = qsoLog.qsos.length;
    const next = Math.min(Math.max(idx + d, 0), n - 1);
    if (next === idx) return;
    idx = next;
    loadText();
    inputEl?.select();
  }

  function commit() {
    if (!target) return;
    if (mode === "note") {
      qsoLog.update(target.id, { note: text.trim() || undefined });
    } else {
      const call = text.toUpperCase().replace(/[^A-Z0-9/]/g, "").slice(0, 12);
      if (call.length < 3) return; // don't save a mangled call
      qsoLog.update(target.id, { call });
    }
    mode = null;
  }

  function onGlobalKey(e: KeyboardEvent) {
    if (e.metaKey || !e.ctrlKey || e.altKey) return;
    if (e.code === "KeyN") {
      e.preventDefault();
      if (mode === "note") step(1);
      else open("note");
    } else if (e.code === "KeyQ") {
      e.preventDefault();
      if (mode === "edit") step(1);
      else open("edit");
    }
  }

  function onInputKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      mode = null;
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      step(1);
    } else if (e.key === "ArrowDown") {
      e.preventDefault();
      step(-1);
    }
  }

  function onInput(e: Event) {
    const v = (e.target as HTMLInputElement).value;
    text = mode === "edit" ? v.toUpperCase() : v;
  }

  function fmtTime(ts: number): string {
    return new Date(ts).toISOString().slice(11, 16) + "z";
  }

  $effect(() => {
    window.addEventListener("keydown", onGlobalKey);
    return () => window.removeEventListener("keydown", onGlobalKey);
  });

  $effect(() => {
    if (mode !== null) {
      inputEl?.focus();
      inputEl?.select();
    }
  });
</script>

{#if mode !== null && target}
  <div class="overlay">
    <div class="box" role="dialog" aria-label={mode === "note" ? "Add note" : "Quick edit call"}>
      <header>
        <h2>{mode === "note" ? "Note" : "Edit call"}</h2>
        <span class="qso">
          #{target.serialSent} · {target.call} · {target.band} · {fmtTime(target.ts)}
          {#if idx > 0}<span class="back">({idx} back)</span>{/if}
        </span>
        <span class="hint">
          <span class="kbd">↵</span> save ·
          <span class="kbd">esc</span> cancel ·
          <span class="kbd">↑↓</span> other QSO
        </span>
      </header>
      <input
        bind:this={inputEl}
        value={text}
        oninput={onInput}
        onkeydown={onInputKey}
        placeholder={mode === "note" ? "Note for this QSO…" : "CALLSIGN"}
        class:call={mode === "edit"}
        spellcheck="false"
        autocomplete="off"
      />
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
    padding-top: 22vh;
    background: rgba(0, 0, 0, 0.45);
  }

  .box {
    width: min(560px, 90vw);
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

  .qso {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    color: #c5d1de;
  }
  .back { color: #fbbf24; }

  .hint {
    margin-left: auto;
    color: #6b7176;
    font-size: 11px;
    white-space: nowrap;
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

  input {
    width: 100%;
    box-sizing: border-box;
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 8px 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 15px;
  }
  input.call {
    font-size: 20px;
    font-weight: 600;
    letter-spacing: 1px;
  }
  input:focus {
    outline: none;
    border-color: #4a90e2;
    background: #0e1418;
  }
</style>
