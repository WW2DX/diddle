<script lang="ts">
  import { qsoLog } from "$lib/qsoLog.svelte";
  import { bandFromHz, fmtMhz } from "$lib/bands";
  import { activeContest } from "$lib/contests";
  import { scpSearch, type RigState } from "$lib/tci";
  import { settings } from "$lib/settings.svelte";
  import { macroState } from "$lib/macros.svelte";
  import { entryBus } from "$lib/entry.svelte";

  let { rig }: { rig: RigState } = $props();

  let callInput: HTMLInputElement | undefined;
  let rstInput: HTMLInputElement | undefined;
  let exchInput: HTMLInputElement | undefined;

  let call = $state("");
  let rstRcvd = $state("599");
  let exchRcvd = $state("");
  // Whether we've already sent our exchange to this station. Lets ESM advance
  // to the log step even when the received exchange is optional (S&P, or the
  // General QSO profile). Reset whenever we move on to a new callsign.
  let exchSent = $state(false);

  let contest = $derived(activeContest());
  let needsExch = $derived(contest.requiresExchange !== false);
  let sentString = $derived(contest.buildSent(qsoLog.nextSerial));
  let band = $derived(bandFromHz(rig.freq));
  let dupe = $derived(call.length >= 3 && qsoLog.isDupe(call, band));
  let canLog = $derived(
    call.length >= 3 && (!needsExch || exchRcvd.length > 0),
  );

  // SCP suggestions — debounced as the user types in the call field.
  let suggestions = $state<string[]>([]);
  let suggestionIdx = $state(-1);
  let scpTimer: ReturnType<typeof setTimeout> | null = null;

  function refreshSuggestions(q: string) {
    if (scpTimer) clearTimeout(scpTimer);
    scpTimer = setTimeout(async () => {
      if (q.length < 2) {
        suggestions = [];
        suggestionIdx = -1;
        return;
      }
      try {
        suggestions = await scpSearch(q, 8);
        suggestionIdx = -1;
      } catch (e) {
        console.error("scp_search failed", e);
      }
    }, 80);
  }

  function acceptSuggestion(s: string) {
    call = s;
    suggestions = [];
    suggestionIdx = -1;
    queueMicrotask(() => exchInput?.focus());
  }

  // ESM (Enter Sends Message) — N1MM-style stepped Enter. The steps differ
  // between Run (you're calling CQ) and Search & Pounce (you're answering
  // someone else's CQ). Wired to F-keys (not labels) so renaming a macro
  // label in Settings doesn't break Enter.
  //
  //   Run:   empty → F1 (CQ)
  //          call  → F2 (send exchange) + focus Exch
  //          +exch → F3 (TU) + log
  //
  //   S&P:   call, no exch → F4 (send our call) + focus Exch
  //          call + exch   → F2 (send our exchange)
  //          (again)       → F3 (TU) + log
  async function esmEnter() {
    const c = normalizeCall(call);
    const ex = exchRcvd.trim();

    if (settings.spMode) {
      // Search & Pounce — nothing to send until we've grabbed a call.
      if (c.length === 0) return;
      if (!exchSent && ex.length === 0) {
        await macroState.fire("F4", { call: c }); // "DE <MYCALL>"
        queueMicrotask(() => exchInput?.focus());
      } else if (!exchSent) {
        await macroState.fire("F2", { call: c }); // our exchange
        exchSent = true;
      } else {
        await macroState.fire("F3"); // TU
        logQso();
      }
      return;
    }

    // Run.
    if (c.length === 0) {
      await macroState.fire("F1");
    } else if (needsExch && ex.length === 0) {
      await macroState.fire("F2", { call: c });
      queueMicrotask(() => exchInput?.focus());
    } else if (!needsExch && !exchSent) {
      // Ragchew: send our info, then a second Enter logs (exch optional).
      await macroState.fire("F2", { call: c });
      exchSent = true;
      queueMicrotask(() => exchInput?.focus());
    } else {
      await macroState.fire("F3");
      logQso();
    }
  }

  // The phase label shown next to the entry fields so the operator knows
  // what Enter will do.
  type Phase = { cls: "cq" | "excg" | "tu" | "idle"; label: string };
  let esmPhase = $derived.by<Phase>(() => {
    const hasCall = call.trim().length > 0;
    const hasExch = exchRcvd.trim().length > 0;
    if (settings.spMode) {
      if (!hasCall) return { cls: "idle", label: "S&P · grab a call" };
      if (!exchSent && !hasExch) return { cls: "cq", label: "S&P · ↵ Call" };
      if (!exchSent) return { cls: "excg", label: "S&P · ↵ Excg" };
      return { cls: "tu", label: "S&P · ↵ TU+Log" };
    }
    if (!hasCall) return { cls: "cq", label: "Run · ↵ CQ" };
    if (needsExch && !hasExch) return { cls: "excg", label: "Run · ↵ Excg" };
    if (!needsExch && !exchSent) return { cls: "excg", label: "Run · ↵ Excg" };
    return { cls: "tu", label: "Run · ↵ TU+Log" };
  });

  // Mirror the live Call field out to the shared bus so F-key macros (with no
  // per-QSO context) can still expand <CALL>.
  $effect(() => {
    entryBus.currentCall = call;
  });

  // A callsign clicked in the decoder window (or another panel) lands here.
  // Copy it into the Call field, restart the ESM sequence, and focus Exch so
  // the next Enter answers/sends.
  let lastBusToken = 0;
  $effect(() => {
    const t = entryBus.token;
    if (t === lastBusToken) return;
    lastBusToken = t;
    const c = normalizeCall(entryBus.requestedCall);
    if (!c) return;
    call = c;
    exchRcvd = "";
    exchSent = false;
    suggestions = [];
    suggestionIdx = -1;
    queueMicrotask(() => exchInput?.focus());
  });

  function normalizeCall(s: string): string {
    return s
      .toUpperCase()
      .replace(/[^A-Z0-9\/]/g, "")
      .slice(0, 12);
  }

  function logQso() {
    const c = normalizeCall(call);
    if (!c) return;
    if (needsExch && !exchRcvd) return;
    // Build the sent exchange via the active contest's formatter (e.g.
    // serial+zone for CQ WW; name+state for NAQP). Store both the legible
    // sent string and the raw rcvd string so exports can re-format.
    const sent = contest.buildSent(qsoLog.nextSerial);
    qsoLog.add({
      id: crypto.randomUUID(),
      ts: Date.now(),
      call: c,
      freqHz: rig.freq,
      band,
      mode: rig.mode || "USB",
      rstSent: "599",
      rstRcvd: rstRcvd.trim() || "599",
      exchSent: sent.replace(/^599\s*/, ""), // strip leading RST if present
      exchRcvd: exchRcvd.trim(),
      serialSent: qsoLog.nextSerial,
    });
    call = "";
    exchRcvd = "";
    rstRcvd = "599";
    exchSent = false;
    queueMicrotask(() => callInput?.focus());
  }

  function clearForm() {
    call = "";
    exchRcvd = "";
    rstRcvd = "599";
    exchSent = false;
    callInput?.focus();
  }

  function onCallInput(e: Event) {
    const t = e.target as HTMLInputElement;
    call = normalizeCall(t.value);
    // Editing the callsign means a new station — restart the ESM sequence.
    exchSent = false;
    refreshSuggestions(call);
  }

  function onKey(e: KeyboardEvent) {
    // Suggestion navigation while typing in the Call field.
    if ((e.target as HTMLElement) === callInput && suggestions.length > 0) {
      if (e.key === "ArrowDown") {
        e.preventDefault();
        suggestionIdx = (suggestionIdx + 1) % suggestions.length;
        return;
      }
      if (e.key === "ArrowUp") {
        e.preventDefault();
        suggestionIdx =
          suggestionIdx <= 0 ? suggestions.length - 1 : suggestionIdx - 1;
        return;
      }
      if (e.key === "Tab" && suggestionIdx >= 0) {
        e.preventDefault();
        acceptSuggestion(suggestions[suggestionIdx]);
        return;
      }
      if (e.key === "Enter" && suggestionIdx >= 0) {
        e.preventDefault();
        acceptSuggestion(suggestions[suggestionIdx]);
        return;
      }
    }

    if (e.key === "Enter") {
      e.preventDefault();
      if (settings.esm) {
        esmEnter();
      } else {
        logQso();
      }
    } else if (e.key === "Escape") {
      e.preventDefault();
      if (suggestions.length > 0) {
        suggestions = [];
        suggestionIdx = -1;
      } else {
        clearForm();
      }
    } else if (e.key === " " && (e.target as HTMLElement) === callInput) {
      e.preventDefault();
      exchInput?.focus();
    }
  }
</script>

<section class="panel">
  <header class="head">
    <h2>Entry</h2>
    <div class="ctx">
      <span class="dim">band</span>
      <span class="band">{band}</span>
      <span class="dim">freq</span>
      <span class="num">{fmtMhz(rig.freq)}</span>
      <span class="dim">mode</span>
      <span class="num">{(rig.mode || "—").toUpperCase()}</span>
      <button
        type="button"
        class="sp-toggle"
        class:sp={settings.spMode}
        onclick={() => settings.toggleSpMode()}
        title="Toggle Run / Search & Pounce (ESM behavior)"
      >
        {settings.spMode ? "S&P" : "RUN"}
      </button>
      {#if settings.esm}
        <span class="esm-chip esm-{esmPhase.cls}">
          {esmPhase.label}
        </span>
      {/if}
      {#if dupe}
        <span class="dupe-flag">DUPE</span>
      {/if}
    </div>
  </header>

  <div class="row">
    <div class="field call-field">
      <label for="call">Call</label>
      <input
        id="call"
        bind:this={callInput}
        value={call}
        oninput={onCallInput}
        onkeydown={onKey}
        spellcheck="false"
        autocomplete="off"
        placeholder=""
      />
      {#if suggestions.length > 0}
        <div class="suggestions">
          {#each suggestions as s, i}
            <button
              type="button"
              class="suggestion"
              class:active={i === suggestionIdx}
              onclick={() => acceptSuggestion(s)}
            >
              {s}
            </button>
          {/each}
        </div>
      {/if}
    </div>

    <div class="field small">
      <label for="rst">RST</label>
      <input
        id="rst"
        bind:this={rstInput}
        bind:value={rstRcvd}
        onkeydown={onKey}
        spellcheck="false"
        maxlength="4"
      />
    </div>

    <div class="field">
      <label for="exch">Exch</label>
      <input
        id="exch"
        bind:this={exchInput}
        bind:value={exchRcvd}
        onkeydown={onKey}
        spellcheck="false"
        placeholder={contest.rcvdPlaceholder}
      />
    </div>

    <div class="sent">
      <div class="sent-row">
        <span class="dim">sent</span>
        <span class="num">{sentString}</span>
      </div>
      <div class="contest-label">{contest.name}</div>
    </div>

    <button class="log-btn" disabled={!canLog} onclick={logQso}>
      Log <span class="kbd">↵</span>
    </button>
  </div>
</section>

<style>
  .panel {
    background: #181c1f;
    border: 1px solid #262b30;
    border-radius: 8px;
    padding: 12px 16px;
    margin-bottom: 12px;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 10px;
  }

  h2 {
    margin: 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: #8a949d;
    font-weight: 600;
  }

  .ctx {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }
  .dim { color: #6b7176; font-size: 10px; text-transform: uppercase; }
  .band { color: #fbbf24; font-weight: 600; }
  .num { color: #c5d1de; }

  .dupe-flag {
    background: #f87171;
    color: #1a0a0a;
    padding: 2px 8px;
    border-radius: 3px;
    font-weight: 700;
    font-size: 11px;
    margin-left: 8px;
    letter-spacing: 1px;
  }

  .esm-chip {
    padding: 2px 8px;
    border-radius: 3px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.3px;
    margin-left: 4px;
  }
  .esm-cq   { background: #2a3f5f; color: #92c5fa; border: 1px solid #3a5a8a; }
  .esm-excg { background: #5f4f2a; color: #fbbf24; border: 1px solid #8a6a3a; }
  .esm-tu   { background: #2a5a3f; color: #a0d8b8; border: 1px solid #3a8a5f; }
  .esm-idle { background: #23282d; color: #8a949d; border: 1px solid #3a4452; }

  .sp-toggle {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.5px;
    padding: 2px 8px;
    border-radius: 3px;
    cursor: pointer;
    margin-left: 4px;
    /* Run (default) */
    background: #2a3f5f;
    color: #92c5fa;
    border: 1px solid #3a5a8a;
  }
  .sp-toggle.sp {
    /* Search & Pounce */
    background: #5a2a4f;
    color: #f0a8d8;
    border: 1px solid #8a3a7a;
  }
  .sp-toggle:hover {
    filter: brightness(1.2);
  }

  .row {
    display: flex;
    align-items: flex-end;
    gap: 10px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1;
  }
  .field.call-field { flex: 2; position: relative; }
  .field.small { flex: 0 0 80px; }

  label {
    color: #8a949d;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  input {
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 8px 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 18px;
    font-weight: 500;
  }

  input:focus {
    outline: none;
    border-color: #4a90e2;
    background: #0e1418;
  }

  .call-field input {
    font-size: 22px;
    font-weight: 600;
    letter-spacing: 1px;
  }

  .sent {
    align-self: flex-end;
    padding-bottom: 6px;
  }
  .sent-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 16px;
  }
  .contest-label {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 10px;
    color: #6b7176;
    margin-top: 2px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
  }

  .log-btn {
    background: #2a5a3f;
    border: 1px solid #3a8a5f;
    color: #e6e6e6;
    padding: 8px 18px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 13px;
    font-weight: 600;
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .log-btn:hover:not(:disabled) {
    background: #357050;
  }
  .log-btn:disabled {
    opacity: 0.35;
    cursor: not-allowed;
  }

  .kbd {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    color: #a0d8b8;
    font-size: 12px;
    border: 1px solid #3a8a5f;
    padding: 0 4px;
    border-radius: 2px;
    background: rgba(0, 0, 0, 0.2);
  }

  .suggestions {
    position: absolute;
    top: 100%;
    left: 0;
    right: 0;
    margin-top: 4px;
    background: #0e1418;
    border: 1px solid #2e3a4a;
    border-radius: 4px;
    box-shadow: 0 6px 20px rgba(0, 0, 0, 0.5);
    z-index: 10;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .suggestion {
    background: transparent;
    border: none;
    color: #c5d1de;
    text-align: left;
    padding: 6px 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 14px;
    font-weight: 600;
    cursor: pointer;
    border-bottom: 1px solid #1a1f24;
  }
  .suggestion:last-child {
    border-bottom: none;
  }
  .suggestion:hover,
  .suggestion.active {
    background: #2a3f5f;
    color: #fff;
  }
</style>
