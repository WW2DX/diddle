<script lang="ts">
  import { qsoLog } from "$lib/qsoLog.svelte";
  import { bandFromHz, fmtMhz } from "$lib/bands";
  import { activeContest, historyExchange } from "$lib/contests";
  import { scpSearch, setFreq, historyLookup, type RigState } from "$lib/tci";
  import { rttyConfig } from "$lib/rttyConfig.svelte";
  import { rfFromAudio, dialForRf, parseFreqInput } from "$lib/freq";
  import { settings } from "$lib/settings.svelte";
  import { macroState } from "$lib/macros.svelte";
  import { entryBus } from "$lib/entry.svelte";
  import { cty } from "$lib/ctyStore.svelte";
  import { usStateZone } from "$lib/cty";
  import { scoreStore } from "$lib/score.svelte";
  import { chime } from "$lib/sound";

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
  // S&P: whether we've thrown our call at him yet. Reset with `exchSent`.
  let callSent = $state(false);
  // The callsign we last transmitted under (F2 in Run, F4 with a call in
  // S&P). Editing the Call field to something else means a new station and
  // restarts the sequence; typing into an empty field — including after a
  // blind S&P call, which went out with no callsign at all — does not.
  let sentCall = $state("");
  // True while Exch holds a value pre-filled from the call-history file (and
  // not yet edited by the operator). Lets a call change replace/clear it.
  let exchFromHistory = $state(false);
  let histTimer: ReturnType<typeof setTimeout> | null = null;

  // Look the call up in the N1MM-style history file and pre-fill Exch if
  // the contest can predict it (name/state/zone — never serials).
  // Also predicts what the history file can't: in CQ WW the zone comes from
  // the country file (history files don't carry it), so a DX station's
  // exchange is there before he sends it.
  function lookupHistory(c: string) {
    if (histTimer) clearTimeout(histTimer);
    if (c.length < 3 || /^[0-9.]+$/.test(c)) return;
    histTimer = setTimeout(async () => {
      let ex = "";
      try {
        const rec = await historyLookup(c);
        ex = rec ? historyExchange(contest, rec) : "";
      } catch (e) {
        console.error("history_lookup failed", e);
      }
      if (normalizeCall(call) !== c) return; // call changed meanwhile
      ex = predictExchange(c, ex);
      if (ex && (exchRcvd.trim() === "" || exchFromHistory)) {
        exchRcvd = ex;
        exchFromHistory = true;
      } else if (!ex && exchFromHistory) {
        exchRcvd = "";
        exchFromHistory = false;
      }
    }, 120);
  }

  // Fill in the CQ zone from the country file when the history exchange
  // doesn't start with one. For a US station whose history gives his state,
  // the state decides — the call area is only a guess at where he is.
  function predictExchange(c: string, ex: string): string {
    if (settings.activeContest !== "cqww-rtty" || /^\d/.test(ex.trim())) return ex;
    const hit = cty.lookup(c);
    if (!hit) return ex;
    let zone = hit.cq;
    if (hit.entity.prefix === "K") {
      const z = ex.split(/\s+/).map(usStateZone).find((v) => v !== undefined);
      if (z !== undefined) zone = z;
    }
    return `${String(zone).padStart(2, "0")} ${ex}`.trim();
  }



  let contest = $derived(activeContest());
  let needsExch = $derived(contest.requiresExchange !== false);
  let sentString = $derived(contest.buildSent(qsoLog.nextSerial));
  // The frequency we log and display is where our *TX mark tone* sits on
  // the air (dial − mark in DIGL), which is what other loggers and the
  // cluster report — not the bare dial reading, and not the RX mark, which
  // AFC may have walked off to follow the other station's drift.
  let qsoFreqHz = $derived(rfFromAudio(rig.freq, rttyConfig.txMarkHz, rig.mode));
  let band = $derived(bandFromHz(qsoFreqHz));
  // A bare number in the Call field is a frequency (kHz), N1MM/WriteLog
  // style: Enter QSYs the radio there instead of running ESM.
  let isFreqEntry = $derived(/^\d+(\.\d+)?$/.test(call) && call.length >= 3);
  let freqEntryHz = $derived(isFreqEntry ? parseFreqInput(call) : null);
  let dupe = $derived(
    !isFreqEntry && call.length >= 3 && qsoLog.isDupe(call, band),
  );
  // Multipliers the station being entered would add — shown live, and the
  // reward (chime + message) when the QSO is logged.
  let liveNewMults = $derived(
    !isFreqEntry && call.length >= 3 && !dupe
      ? scoreStore.newMults(normalizeCall(call), band, exchRcvd.trim())
      : [],
  );
  let canLog = $derived(
    !isFreqEntry && call.length >= 3 && (!needsExch || exchRcvd.length > 0),
  );

  // Retune so the typed frequency lands on the TX mark tone, then clear.
  // A deliberate QSY also re-aligns the decoder with TX, discarding AFC drift.
  async function qsyToTyped() {
    if (!freqEntryHz) return;
    try {
      await setFreq(dialForRf(freqEntryHz, rttyConfig.txMarkHz, rig.mode));
      await rttyConfig.setMark(rttyConfig.txMarkHz);
      clearForm();
    } catch (e) {
      console.error("set_freq failed", e);
    }
  }

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
    lookupHistory(s);
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
  // The Run steps turn on `exchSent`, not on whether Exch happens to hold
  // something: a call-history pre-fill (CQ WW zone, NAQP name/state) used
  // to skip the exchange and send TU to a station we had said nothing to.
  //
  //   S&P:   empty         → F4 (send our call) — jump in on a CQ before
  //                          we've copied who it is; Call keeps focus
  //          call, no exch → F4 (send our call) + focus Exch
  //          call + exch   → F2 (send our exchange) + log
  //
  // S&P stops there, N1MM-style: the TU is the running station's to send,
  // and ours (F3) ends in CQ — which on his run frequency is the last
  // thing anybody wants.
  // Every step updates the sequence state *before* its message goes out
  // (fire() expands tokens synchronously), so a second Enter while the
  // first is still on the air moves on instead of repeating it — F-keys
  // now chain rather than being refused while transmitting.
  function esmEnter() {
    const c = normalizeCall(call);
    const ex = exchRcvd.trim();

    if (settings.spMode) {
      if (c.length === 0) {
        // Search & Pounce, blind: answer his CQ with our call and stay in
        // the Call field to type his when he comes back.
        macroState.fire("F4"); // "DE <MYCALL>"
        callSent = true;
        queueMicrotask(() => callInput?.focus());
      } else if (!callSent || (needsExch && ex.length === 0)) {
        // Our call — and again on each Enter until he comes back to us.
        macroState.fire("F4", { call: c }); // "DE <MYCALL>"
        callSent = true;
        sentCall = c;
        queueMicrotask(() => exchInput?.focus());
      } else {
        macroState.fire("F2", { call: c }); // our exchange
        exchSent = true;
        logQso();
      }
      return;
    }

    // Run.
    if (c.length === 0) {
      macroState.fire("F1");
    } else if (!exchSent || (needsExch && ex.length === 0)) {
      // Our exchange — and again on a bare Enter while we're still waiting
      // for his, which is how you ask for a repeat without an F-key.
      macroState.fire("F2", { call: c });
      exchSent = true;
      sentCall = c;
      queueMicrotask(() => exchInput?.focus());
    } else if (entryBus.nextQueue.length) {
      // Callers stacked: the stack-TU macro thanks this one, logs him
      // (<LOGIT>), loads the next (<POPSTACK>) and sends him our exchange,
      // all in one transmission.
      macroState.send(stackTuText(), { call: c });
    } else {
      macroState.fire("F3", { call: c });
      logQso();
    }
  }

  // The stack-TU macro as ESM sends it. One without <POPSTACK> (F8 still
  // holding something else) falls back to TU + log + the next caller's F2
  // exchange, and one without <LOGIT> gets it just before <POPSTACK>, so
  // the QSO is never left unlogged.
  function stackTuText(): string {
    const m = macroState.macros.find((x) => x.key === settings.stackTuKey);
    if (m && /<POPSTACK>/i.test(m.text)) {
      return /<LOGIT>/i.test(m.text) ? m.text : m.text.replace(/<POPSTACK>/i, "<LOGIT><POPSTACK>");
    }
    const f2 = macroState.macros.find((x) => x.key === "F2")?.text ?? "<CALL> 599 <SERIAL>";
    return `TU <CALL><LOGIT><CRLF><POPSTACK>${f2}`;
  }

  // What the <LOGIT> and <POPSTACK> macro tokens do — whether ESM sent the
  // macro or the operator pressed its F-key.
  $effect(() => {
    entryBus.actions = {
      logIt: () => {
        if (!canLog) return false;
        logQso();
        return true;
      },
      popStack: () => {
        const next = entryBus.popNext();
        if (!next) return undefined;
        loadCall(next);
        // The rest of the macro is his exchange, so the next Enter is TU.
        exchSent = true;
        sentCall = next;
        return next;
      },
    };
    return () => {
      entryBus.actions = null;
    };
  });


  // The phase label shown next to the entry fields so the operator knows
  // what Enter will do.
  type Phase = { cls: "cq" | "excg" | "tu" | "idle"; label: string };
  let esmPhase = $derived.by<Phase>(() => {
    const hasCall = call.trim().length > 0;
    const hasExch = exchRcvd.trim().length > 0;
    if (isFreqEntry) {
      return {
        cls: "idle",
        label: freqEntryHz ? `↵ QSY ${fmtMhz(freqEntryHz)}` : "↵ QSY ?",
      };
    }
    if (settings.spMode) {
      if (!hasCall || !callSent || (needsExch && !hasExch))
        return { cls: "cq", label: "S&P · ↵ Call" };
      return { cls: "tu", label: "S&P · ↵ Excg+Log" };
    }
    if (!hasCall) return { cls: "cq", label: "Run · ↵ CQ" };
    if (!exchSent || (needsExch && !hasExch))
      return { cls: "excg", label: "Run · ↵ Excg" };
    return {
      cls: "tu",
      label: entryBus.nextQueue.length
        ? `Run · ↵ ${settings.stackTuKey} TU+Log+Next`
        : "Run · ↵ TU+Log",
    };
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
    if (c) loadCall(c, entryBus.requestedFocus);
  });

  // A caller right-clicked onto the stack: take keyboard focus back from
  // the RX window / waterfall, so Enter carries on with the station in
  // Call instead of pressing the call chip that was just clicked.
  let lastQueueToken = 0;
  $effect(() => {
    const t = entryBus.queueToken;
    if (t === lastQueueToken) return;
    lastQueueToken = t;
    queueMicrotask(() => (exchSent ? exchInput : callInput)?.focus());
  });

  // Put a callsign in the Call field as a fresh station (clicked spot,
  // stack) and get ready for his exchange — or, for a caller right-clicked
  // into an empty Call, leave focus on Call so the next Enter sends him ours.
  function loadCall(c: string, focus: "call" | "exch" = "exch") {
    noteCallChanged(c);
    call = c;
    exchRcvd = "";
    exchFromHistory = false;
    suggestions = [];
    suggestionIdx = -1;
    lookupHistory(c);
    queueMicrotask(() => (focus === "call" ? callInput : exchInput)?.focus());
  }

  // An exchange word clicked in the decoder window. Builds the exchange a
  // piece at a time, N1MM-style: the first click replaces an empty or
  // history-filled Exch (his zone may differ from last year's), later clicks
  // append (zone, then state), and a word already there isn't doubled.
  let lastExchToken = 0;
  $effect(() => {
    const t = entryBus.exchToken;
    if (t === lastExchToken) return;
    lastExchToken = t;
    const w = entryBus.requestedExchWord.trim().toUpperCase();
    if (!w) return;
    const cur = exchRcvd.trim();
    if (!cur || exchFromHistory) {
      exchRcvd = w;
    } else if (!cur.split(/\s+/).includes(w)) {
      exchRcvd = `${cur} ${w}`;
    }
    exchFromHistory = false;
    queueMicrotask(() => {
      exchInput?.focus();
      const n = exchInput?.value.length ?? 0;
      exchInput?.setSelectionRange(n, n);
    });
  });

  // TU (F3) sent by hand in Run with ESM on: log the QSO, as ESM's TU step
  // does. If there's a call but no exchange, say so rather than leave the
  // operator thinking it was logged (CQ WW DX stations send only a zone —
  // it still has to go in Exch).
  let lastTuToken = 0;
  let notice = $state<string | null>(null);
  let noticeTimer: ReturnType<typeof setTimeout> | null = null;
  let noticeKind = $state<"warn" | "mult">("warn");
  function flashNotice(msg: string, kind: "warn" | "mult" = "warn") {
    notice = msg;
    noticeKind = kind;
    if (noticeTimer) clearTimeout(noticeTimer);
    noticeTimer = setTimeout(() => (notice = null), 4000);
  }
  $effect(() => {
    const t = entryBus.tuToken;
    if (t === lastTuToken) return;
    lastTuToken = t;
    if (!settings.esm || settings.spMode) return;
    if (canLog) {
      logQso();
    } else if (normalizeCall(call).length >= 3) {
      flashNotice("TU sent — NOT logged: Exch is empty");
      queueMicrotask(() => exchInput?.focus());
    }
  });

  function normalizeCall(s: string): string {
    return s
      .toUpperCase()
      .replace(/[^A-Z0-9\/]/g, "")
      .slice(0, 12);
  }

  // A callsign other than the one we've already transmitted under is a new
  // station: restart the ESM sequence. Nothing sent under a callsign yet
  // (fresh form, or a blind S&P call) → nothing to restart, so typing his
  // call in doesn't make Enter send ours a second time.
  function noteCallChanged(c: string) {
    if (sentCall && c !== sentCall) {
      exchSent = false;
      callSent = false;
      sentCall = "";
    }
  }

  function logQso() {
    const c = normalizeCall(call);
    if (!c) return;
    if (needsExch && !exchRcvd) return;
    // Build the sent exchange via the active contest's formatter (e.g.
    // serial+zone for CQ WW; name+state for NAQP). Store both the legible
    // sent string and the raw rcvd string so exports can re-format.
    const sent = contest.buildSent(qsoLog.nextSerial);
    const gained = scoreStore.newMults(c, band, exchRcvd.trim());
    if (gained.length) {
      flashNotice(`NEW MULT — ${gained.join(" · ")}`, "mult");
      if (settings.multBell) chime();
    }
    qsoLog.add({
      id: crypto.randomUUID(),
      ts: Date.now(),
      call: c,
      freqHz: qsoFreqHz,
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
    callSent = false;
    sentCall = "";
    exchFromHistory = false;
    queueMicrotask(() => callInput?.focus());
  }

  function clearForm() {
    call = "";
    exchRcvd = "";
    rstRcvd = "599";
    exchSent = false;
    callSent = false;
    sentCall = "";
    exchFromHistory = false;
    callInput?.focus();
  }

  // Digits (and a dot) only → the operator is typing a frequency; keep it
  // verbatim. Otherwise normalise as a callsign.
  function normalizeEntry(s: string): string {
    const t = s.toUpperCase().trim();
    if (/^[0-9.]+$/.test(t)) return t.slice(0, 12);
    return normalizeCall(t);
  }

  function onCallInput(e: Event) {
    const t = e.target as HTMLInputElement;
    call = normalizeEntry(t.value);
    noteCallChanged(call);
    if (/^[0-9.]+$/.test(call)) {
      suggestions = [];
      suggestionIdx = -1;
      return;
    }
    refreshSuggestions(call);
    lookupHistory(call);
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

    if (e.key === "Enter" && e.ctrlKey) {
      // Log It — handled by onGlobalKey so it works from any focus.
      return;
    } else if (e.key === "Enter") {
      e.preventDefault();
      if (isFreqEntry) {
        qsyToTyped();
      } else if (settings.esm) {
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
    } else if (e.key === "Tab") {
      // Tab / Shift+Tab cycle strictly within the entry fields
      // (Call → RST → Exch → Call) instead of wandering off to buttons.
      e.preventDefault();
      const order = [callInput, rstInput, exchInput];
      const i = order.indexOf(e.target as HTMLInputElement);
      const next = order[(i + (e.shiftKey ? order.length - 1 : 1)) % order.length];
      next?.focus();
      if (next === callInput) next?.select();
    } else if (e.key === " ") {
      // Space toggles between Call and Exch. From Exch it only jumps when
      // the field is empty — exchanges like NAQP's "JOHN MA" need literal
      // spaces once you've started typing (use Shift+Tab to get back then).
      const t = e.target as HTMLElement;
      if (t === callInput) {
        e.preventDefault();
        exchInput?.focus();
      } else if (
        t === rstInput ||
        (t === exchInput && exchRcvd.trim().length === 0)
      ) {
        e.preventDefault();
        callInput?.focus();
        callInput?.select();
      }
    }
  }

  // Global entry shortcuts — active regardless of focus:
  //   Ctrl+Enter  Log It: log Call + Exch as they stand, send nothing
  //               (N1MM-style; the way out of a botched ESM sequence)
  //   Ctrl/Alt+W  wipe the entry fields
  //   Alt+U       toggle Run / S&P
  //   Ctrl+D      delete the most recent QSO (press twice to confirm)
  // e.code, not e.key, so macOS Alt dead-keys don't hide the shortcut.
  // Cmd combos are left alone (Cmd+W/Q/D belong to the OS).
  let pendingDelete = $state<{ id: string; call: string } | null>(null);
  let pendingDeleteTimer: ReturnType<typeof setTimeout> | null = null;

  function deleteLastQso() {
    const last = qsoLog.qsos[qsoLog.qsos.length - 1];
    if (pendingDeleteTimer) clearTimeout(pendingDeleteTimer);
    if (!last) {
      pendingDelete = null;
      return;
    }
    if (pendingDelete?.id === last.id) {
      qsoLog.remove(last.id);
      pendingDelete = null;
    } else {
      pendingDelete = { id: last.id, call: last.call };
      pendingDeleteTimer = setTimeout(() => (pendingDelete = null), 3000);
    }
  }

  function onGlobalKey(e: KeyboardEvent) {
    if (e.metaKey) return;
    if (e.key === "Enter" && e.ctrlKey && !e.altKey) {
      // Don't steal Ctrl+Enter from another panel's text field (log edit,
      // Ctrl+Q/Ctrl+N popups, settings); the entry fields are fine.
      const t = e.target as HTMLElement | null;
      const foreignField =
        t instanceof HTMLInputElement || t instanceof HTMLTextAreaElement
          ? !(t === callInput || t === rstInput || t === exchInput)
          : false;
      if (foreignField) return;
      e.preventDefault();
      if (canLog) logQso();
    } else if (e.code === "KeyW" && (e.ctrlKey || e.altKey)) {
      e.preventDefault();
      clearForm();
    } else if (e.code === "KeyU" && e.altKey && !e.ctrlKey) {
      e.preventDefault();
      settings.toggleSpMode();
    } else if (e.code === "KeyD" && e.ctrlKey && !e.altKey) {
      e.preventDefault();
      deleteLastQso();
    }
  }

  $effect(() => {
    window.addEventListener("keydown", onGlobalKey);
    return () => window.removeEventListener("keydown", onGlobalKey);
  });
</script>

<section class="panel" style={settings.fontStyle("entry")}>
  <header class="head">
    <h2>Entry</h2>
    <div class="ctx">
      <span class="dim">band</span>
      <span class="band">{band}</span>
      <span class="dim" title="Mark-tone RF (dial − mark in DIGL) — what gets logged">freq</span>
      <span class="num" title="dial {fmtMhz(rig.freq)}">{fmtMhz(qsoFreqHz)}</span>
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
      {#if liveNewMults.length}
        <span class="mult-chip" title="This station would be a new multiplier">NEW: {liveNewMults.join(" · ")}</span>
      {/if}
      {#if notice}
        <span class={noticeKind === "mult" ? "mult-notice" : "del-pending"}>{notice}</span>
      {/if}
      {#if pendingDelete}
        <span class="del-pending">Ctrl+D again deletes {pendingDelete.call}</span>
      {/if}
    </div>
  </header>

  <div class="row">
    <div class="field call-field">
      <label for="call">Call</label>
      <input
        id="call"
        bind:this={callInput}
        class:dupe={dupe}
        class:newmult={liveNewMults.length > 0}
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
        oninput={() => (exchFromHistory = false)}
        onkeydown={onKey}
        class:from-history={exchFromHistory}
        spellcheck="false"
        placeholder={contest.rcvdPlaceholder}
        title={exchFromHistory ? "pre-filled from call history — type to override" : ""}
      />
    </div>

    <div class="sent">
      <div class="sent-row">
        <span class="dim">sent</span>
        <span class="num">{sentString}</span>
      </div>
      <div class="contest-label">{contest.name}</div>
    </div>

    <button
      class="log-btn"
      disabled={!canLog}
      onclick={logQso}
      title="Log what's in Call and Exch now, without sending anything (Ctrl+Enter). Works the same with ESM on or off."
    >
      Log it <span class="kbd">Ctrl+↵</span>
    </button>
  </div>

  {#if !settings.spMode}
    <div class="stack" class:active={entryBus.nextQueue.length > 0}>
      <span
        class="stack-label"
        title="Right-click callers in the RX window or on the waterfall to stack them. With ESM, Enter at the TU step sends {settings.stackTuKey}: TU, log, then the next caller and our exchange. Click a call to drop it."
      >Stack</span>
      {#each entryBus.nextQueue as n, i}
        <button type="button" tabindex="-1" onmousedown={(e) => e.preventDefault()} class="next-chip" class:first={i === 0} title="Drop {n} from the stack" onclick={() => entryBus.dropNext(n)}>{n} ×</button>
      {:else}
        <span class="stack-empty">empty — right-click callers to stack them</span>
      {/each}
      {#if entryBus.nextQueue.length > 1}
        <button type="button" tabindex="-1" onmousedown={(e) => e.preventDefault()} class="stack-clear" onclick={() => entryBus.clearNext()}>clear</button>
      {/if}
    </div>
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

  .head {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 12px;
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
    flex-wrap: wrap;
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

  .mult-chip, .mult-notice {
    background: #3f2a5f;
    border: 1px solid #c084fc;
    color: #e9d5ff;
    border-radius: 3px;
    padding: 1px 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    font-weight: 600;
  }
  .mult-notice { animation: multflash 0.6s ease-out 3; }
  @keyframes multflash { 50% { background: #7c3aed; } }
  input.newmult:not(.dupe) { border-color: #c084fc; color: #e9d5ff; }

  .del-pending {
    background: #4a1f1f;
    border: 1px solid #f87171;
    color: #f87171;
    padding: 2px 8px;
    border-radius: 3px;
    font-weight: 600;
    font-size: 11px;
    margin-left: 8px;
    white-space: nowrap;
  }

  .stack {
    display: flex;
    align-items: center;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 10px;
    padding: 4px 8px;
    border: 1px dashed #2a3036;
    border-radius: 4px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }
  .stack.active { border: 1px solid #3a5a8a; background: #141d27; }
  .stack-label { color: #8a949d; font-size: 10px; text-transform: uppercase; letter-spacing: 1px; font-weight: 600; }
  .stack.active .stack-label { color: #92c5fa; }
  .stack-empty { color: #4f565c; font-size: 11px; }
  .stack-clear {
    margin-left: auto;
    background: none;
    border: 1px solid #2a3036;
    border-radius: 3px;
    color: #8a949d;
    font-size: 10px;
    padding: 1px 6px;
    cursor: pointer;
  }
  .stack-clear:hover { color: #f87171; border-color: #f87171; }
  .next-chip {
    background: #1c2a3a;
    border: 1px solid #3a5a8a;
    color: #92c5fa;
    border-radius: 3px;
    padding: 1px 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    cursor: pointer;
  }
  .next-chip.first { font-weight: 700; border-color: #60a5fa; }
  .next-chip:hover { border-color: #f87171; color: #f87171; }

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

  /* With a big Entry font the fields shrink to the panel and "sent" / Log it
     wrap underneath, instead of the row running off under the bandmap. */
  .row {
    display: flex;
    flex-wrap: wrap;
    align-items: flex-end;
    gap: 10px;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex: 1 1 8em;
    min-width: 0;
  }
  .field.call-field { flex: 2 1 10em; position: relative; }
  /* Three digits of the Entry font, plus padding. */
  .field.small { flex: 0 0 calc(var(--win-size, 18px) * 2.4 + 24px); }

  label {
    color: #8a949d;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  input {
    box-sizing: border-box;
    width: 100%;
    min-width: 0;
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 8px 10px;
    font-family: var(--win-font, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: var(--win-size, 18px);
    font-variant-numeric: var(--win-zero, normal);
    font-weight: 500;
  }

  input:focus {
    outline: none;
    border-color: #4a90e2;
    background: #0e1418;
  }

  /* A size up from the other fields, and it follows the Entry font size. */
  .call-field input {
    font-size: calc(var(--win-size, 18px) * 1.22);
    font-weight: 600;
    letter-spacing: 1px;
  }
  /* Already worked on this band — the same red used for dupes everywhere. */
  .call-field input.dupe {
    color: #f87171;
    border-color: #f87171;
    background: #1c0f0f;
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
  /* Exch pre-filled from the call-history file — tinted until edited. */
  input.from-history { color: #fbbf24; }
</style>
