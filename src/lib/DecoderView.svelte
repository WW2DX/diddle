<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { onRtty, onTxEcho, scpContainsAny } from "$lib/tci";
  import { rttyConfig } from "$lib/rttyConfig.svelte";
  import { cluster } from "$lib/cluster.svelte";
  import { settings, HISTORY_MIN, HISTORY_MAX } from "$lib/settings.svelte";
  import { entryBus } from "$lib/entry.svelte";
  import TuningScope from "$lib/TuningScope.svelte";

  // Hard safety ceiling on retained characters, independent of the line-based
  // history setting — guards against a station that never sends a line break.
  const HARD_CHAR_CAP = 2_000_000;

  // The decoder window is a sequence of runs so received and transmitted
  // text can be colored differently while sharing one scrollback. TX runs
  // are echoed live as our own signal goes on the air. Callsigns inside any
  // run are detected at render time and turned into clickable chips.
  type Run = { tx: boolean; s: string };
  let runs = $state<Run[]>([]);
  // Whether new content sticks the view to the bottom. Flipped off when the
  // operator scrolls up to read back, and on again when they return to the
  // bottom — so scrollback isn't yanked away mid-read.
  // Default OFF: show the raw decoder stream as it arrives so the operator
  // sees real-time copy. When on, only lines with a known call/marker survive.
  let autoScroll = $state(true);
  let filterNoise = $state(false);
  let scrollEl: HTMLDivElement | undefined;
  let unlisten: (() => void) | null = null;
  let unlistenTx: (() => void) | null = null;

  const SHIFT_PRESETS = [170, 200, 425, 850];
  const BAUD_PRESETS = [45.45, 50, 75, 100];

  // Common operating tokens — if any of these appear in a line, treat it as
  // a real transmission even if no callsign validates. Covers contest
  // exchange shorthand and Q-codes.
  const MARKERS = new Set([
    "CQ", "DE", "TEST", "QRZ", "TU", "73", "88",
    "599", "5NN", "59", "5N",
    "K", "KN", "SK",
    "TNX", "THX", "QSL", "QSO", "QRX", "QRZ",
    "BK", "BREAK", "AGN", "PSE",
    "RST", "RTTY",
  ]);
  // Maidenhead grid square (4 or 6 char).
  const GRID_RE = /^[A-R]{2}\d{2}([A-X]{2})?$/;
  // Callsign shapes — kept liberal; SCP confirms membership.
  const CALL_RE = /^([A-Z0-9]{1,3}\/)?[A-Z]{1,2}\d{1,3}[A-Z]{1,4}(\/[A-Z0-9]{1,3})?$/;

  function appendText(s: string, tx = false) {
    // Coalesce into the trailing run when the kind matches, so a long stream
    // doesn't fragment into thousands of runs.
    const last = runs[runs.length - 1];
    if (last && last.tx === tx) {
      last.s += s;
    } else {
      runs.push({ tx, s });
    }
    trimHistory();
    if (autoScroll) {
      queueMicrotask(() => {
        if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
      });
    }
  }

  function pickCall(c: string) {
    entryBus.setCall(c);
  }

  // Render segments: the runs flattened and split around callsign-shaped
  // tokens so every displayed call — RX (filtered or raw) and TX echo alike —
  // becomes a clickable chip. Non-call text stays in large chunks so the DOM
  // stays light. Recomputed only when the scrollback changes.
  type Seg = { tx: boolean; s: string; call?: string };
  const TOKEN_RE = /[A-Za-z0-9/]+/g;

  function isCallToken(tok: string): boolean {
    const c = tok.toUpperCase();
    return c.length >= 3 && CALL_RE.test(c) && !MARKERS.has(c);
  }

  function buildSegments(rs: Run[]): Seg[] {
    const segs: Seg[] = [];
    let buf = "";
    let bufTx = false;
    const flush = () => {
      if (buf) {
        segs.push({ tx: bufTx, s: buf });
        buf = "";
      }
    };
    for (const run of rs) {
      if (run.tx !== bufTx) {
        flush();
        bufTx = run.tx;
      }
      const s = run.s;
      let last = 0;
      let m: RegExpExecArray | null;
      TOKEN_RE.lastIndex = 0;
      while ((m = TOKEN_RE.exec(s)) !== null) {
        if (!isCallToken(m[0])) continue;
        buf += s.slice(last, m.index);
        flush();
        segs.push({ tx: bufTx, s: m[0], call: m[0].toUpperCase() });
        last = m.index + m[0].length;
      }
      buf += s.slice(last);
    }
    flush();
    return segs;
  }

  let segments = $derived(buildSegments(runs));

  /// Drop the oldest lines once the retained history exceeds the operator's
  /// line budget. A "line" is a newline-terminated run of text; the live,
  /// unterminated tail (`pendingLine`) is not counted. A hard character cap
  /// is also enforced as a backstop.
  function trimHistory() {
    const maxLines = settings.decodeHistoryLines;
    let lines = 0;
    for (const r of runs) {
      for (let i = 0; i < r.s.length; i++) {
        if (r.s.charCodeAt(i) === 10) lines++;
      }
    }
    // Drop whole leading lines until we're within the line budget.
    let dropLines = lines - maxLines;
    while (dropLines > 0 && runs.length > 0) {
      const r = runs[0];
      const nl = r.s.indexOf("\n");
      if (nl === -1) {
        // Partial head of the oldest line — its terminating newline is in a
        // later run; discard this fragment and keep going.
        runs.shift();
        continue;
      }
      r.s = r.s.slice(nl + 1);
      dropLines--;
      if (r.s.length === 0) runs.shift();
    }
    // Backstop: never let a newline-less stream grow without bound.
    let total = runs.reduce((n, r) => n + r.s.length, 0);
    while (total > HARD_CHAR_CAP && runs.length > 0) {
      const over = total - HARD_CHAR_CAP;
      if (runs[0].s.length <= over) {
        total -= runs[0].s.length;
        runs.shift();
      } else {
        runs[0].s = runs[0].s.slice(over);
        total -= over;
      }
    }
  }

  /// Track whether the operator is parked at the bottom. Reading back up the
  /// scrollback pauses auto-scroll; returning to the bottom resumes it.
  function onScroll() {
    if (!scrollEl) return;
    const dist = scrollEl.scrollHeight - scrollEl.scrollTop - scrollEl.clientHeight;
    autoScroll = dist < 8;
  }

  function jumpToBottom() {
    if (!scrollEl) return;
    scrollEl.scrollTop = scrollEl.scrollHeight;
    autoScroll = true;
  }

  // Noise filter pipeline. The in-progress line is shown live in `pendingLine`
  // so decodes print character-by-character in real time. When a line
  // completes (CR/LF, or it grows past the force-flush length) it's scored in
  // context: kept lines are committed to the scrollback, junk is dropped.
  let pendingLine = $state("");
  let classifyQueue: string[] = [];
  let classifying = false;

  function scrollSoon() {
    if (!autoScroll) return;
    queueMicrotask(() => {
      if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
    });
  }

  async function appendChunk(chunk: string) {
    if (!filterNoise) {
      // Filter off: stream straight to the scrollback, nothing held back.
      if (pendingLine) {
        appendText(pendingLine);
        pendingLine = "";
      }
      appendText(chunk);
      return;
    }
    pendingLine += chunk;
    // Peel completed lines off the front synchronously so `pendingLine` only
    // ever holds the live, not-yet-terminated text.
    let idx: number;
    while ((idx = pendingLine.search(/[\r\n]/)) >= 0) {
      let n = 1;
      while (idx + n < pendingLine.length && /[\r\n]/.test(pendingLine[idx + n])) n++;
      classifyQueue.push(pendingLine.slice(0, idx));
      pendingLine = pendingLine.slice(idx + n);
    }
    // Force-finalize an over-long unterminated line so a station that never
    // sends CR/LF still gets scored and committed.
    if (pendingLine.length > 200) {
      classifyQueue.push(pendingLine);
      pendingLine = "";
    }
    scrollSoon();
    drainClassify();
  }

  /// Classify queued completed lines in order. Async (SCP lookups) but
  /// serialized so lines commit in the order they were received.
  async function drainClassify() {
    if (classifying) return;
    classifying = true;
    try {
      while (classifyQueue.length > 0) {
        const line = classifyQueue.shift()!;
        if (line.trim().length > 0) await classifyAndEmit(line);
      }
    } finally {
      classifying = false;
    }
  }

  async function classifyAndEmit(rawLine: string) {
    const line = rawLine.replace(/[\r\n]+/g, " ").trim();
    if (line.length === 0) return;

    // Char-class ratio over non-space chars. Pure-noise lines from Baudot
    // FIGS toggling come back loaded with $&?'":,.()/; — reject if more
    // than half of the line is junk.
    const non_space = line.replace(/\s+/g, "");
    const cleanCount = (non_space.match(/[A-Z0-9\/]/gi) ?? []).length;
    const ratio = non_space.length === 0 ? 0 : cleanCount / non_space.length;
    if (ratio < 0.5) return;

    const tokens = line
      .toUpperCase()
      .split(/[^A-Z0-9\/]+/)
      .filter((t) => t.length >= 2);

    // Marker / grid hits — cheap, no IPC.
    for (const t of tokens) {
      if (MARKERS.has(t) || GRID_RE.test(t)) {
        appendText(line + "\n");
        return;
      }
    }

    // Cluster watchlist — calls we already know are on the band.
    const clusterCalls = new Set(
      cluster.spots.map((s) => s.dx_call.toUpperCase()),
    );
    for (const t of tokens) {
      if (clusterCalls.has(t)) {
        appendText(line + "\n");
        return;
      }
    }

    // SCP validation for call-shape tokens.
    const candidates = tokens.filter((t) => CALL_RE.test(t));
    if (candidates.length > 0) {
      try {
        const matched = await scpContainsAny(candidates);
        if (matched) {
          appendText(line + "\n");
          return;
        }
      } catch (e) {
        // SCP unreachable — fall through and drop (matches default behavior
        // of being skeptical when we can't verify).
        console.error("scpContainsAny failed", e);
      }
    }
    // Otherwise: looked clean but had no recognizable operating content.
    // Drop silently.
  }

  onMount(async () => {
    await rttyConfig.load();
    unlisten = await onRtty(appendChunk);
    // TX echo bypasses the noise filter — it's our own text, always shown.
    unlistenTx = await onTxEcho((c) => appendText(c, true));
  });

  onDestroy(() => {
    unlisten?.();
    unlistenTx?.();
  });

  function clear() {
    runs = [];
    pendingLine = "";
    classifyQueue = [];
  }
</script>

<section class="panel">
  <header>
    <h2>RX decoder</h2>
    <div class="tones">
      <span class="tone-pair">
        <span class="mark-dot">●</span>
        <span class="num">{rttyConfig.markHz.toFixed(0)}</span>
        <span class="dim">/</span>
        <span class="num">{rttyConfig.spaceHz.toFixed(0)}</span>
        <span class="dim">Hz</span>
        <span class="space-dot">●</span>
      </span>
      <span class="shift-group">
        <span class="dim">shift:</span>
        {#each SHIFT_PRESETS as s}
          <button
            class="shift-btn"
            class:active={rttyConfig.shiftHz === s}
            onclick={() => rttyConfig.setShift(s)}
          >
            {s}
          </button>
        {/each}
      </span>
      <span class="shift-group">
        <span class="dim">baud:</span>
        {#each BAUD_PRESETS as b}
          <button
            class="shift-btn"
            class:active={Math.abs(rttyConfig.baud - b) < 0.01}
            onclick={() => rttyConfig.setBaud(b)}
          >
            {b}
          </button>
        {/each}
      </span>
      <label class="reverse-label">
        <input
          type="checkbox"
          checked={rttyConfig.reverse}
          onchange={(e) => rttyConfig.setReverse((e.target as HTMLInputElement).checked)}
        />
        REV
      </label>
    </div>
    <div class="meta">
      <label
        title="Show only lines with a known callsign (SCP/cluster) or contest marker"
      >
        <input type="checkbox" bind:checked={filterNoise} />
        filter noise
      </label>
      <label title="Number of decoded lines kept before the oldest scroll off for good">
        history
        <input
          class="history-input"
          type="number"
          min={HISTORY_MIN}
          max={HISTORY_MAX}
          step="100"
          value={settings.decodeHistoryLines}
          onchange={(e) =>
            settings.setDecodeHistoryLines(
              (e.target as HTMLInputElement).valueAsNumber,
            )}
        />
        lines
      </label>
      <label title="Keep the newest decodes pinned to the bottom; scrolling up pauses it">
        <input
          type="checkbox"
          checked={autoScroll}
          onchange={(e) =>
            (e.target as HTMLInputElement).checked
              ? jumpToBottom()
              : (autoScroll = false)}
        />
        auto-scroll
      </label>
      <button class="ghost" onclick={clear}>clear</button>
    </div>
  </header>
  <div class="rx-body">
    <TuningScope />
    <div class="rx-wrap">
      <div class="rx-text" bind:this={scrollEl} onscroll={onScroll}
        >{#each segments as seg}{#if seg.call}<button class="call-chip" class:tx={seg.tx} title={`Load ${seg.call} into the entry form`} onclick={() => pickCall(seg.call!)}>{seg.s}</button>{:else}<span class:tx={seg.tx}>{seg.s}</span>{/if}{/each}{#if pendingLine}<span class="pending">{pendingLine}</span>{/if}{#if segments.length === 0 && !pendingLine}{" "}{/if}</div
      >
      {#if !autoScroll}
        <button class="jump-btn" onclick={jumpToBottom} title="Jump to latest">
          ↓ latest
        </button>
      {/if}
    </div>
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

  header {
    display: flex;
    align-items: center;
    gap: 14px;
    margin-bottom: 8px;
    flex-wrap: wrap;
  }

  h2 {
    margin: 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: #8a949d;
    font-weight: 600;
  }

  .tones {
    display: flex;
    align-items: center;
    gap: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    color: #8a949d;
  }

  .tone-pair {
    display: flex;
    align-items: center;
    gap: 4px;
  }
  .mark-dot { color: #4ade80; }
  .space-dot { color: #fbbf24; }

  .num { color: #e6e6e6; }

  .shift-group {
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .shift-btn {
    background: transparent;
    border: 1px solid #3a4452;
    color: #8a949d;
    padding: 2px 6px;
    border-radius: 3px;
    cursor: pointer;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 10px;
  }
  .shift-btn:hover { border-color: #5a6573; color: #c5d1de; }
  .shift-btn.active {
    background: #2a3f5f;
    border-color: #4a90e2;
    color: #e6e6e6;
  }

  .reverse-label {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
    color: #8a949d;
    font-size: 11px;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 12px;
    font-size: 11px;
    color: #8a949d;
    margin-left: auto;
  }

  .dim {
    color: #6b7176;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .meta label {
    display: flex;
    align-items: center;
    gap: 4px;
    cursor: pointer;
  }

  button.ghost {
    background: transparent;
    border: 1px solid #3a4452;
    color: #8a949d;
    padding: 3px 10px;
    border-radius: 3px;
    cursor: pointer;
    font-size: 11px;
  }
  button.ghost:hover {
    border-color: #5a6573;
    color: #c5d1de;
  }

  .rx-body {
    display: flex;
    gap: 12px;
    align-items: stretch;
  }

  .rx-wrap {
    flex: 1;
    position: relative;
    min-width: 0;
  }

  .rx-text {
    box-sizing: border-box;
    width: 100%;
    background: #0c0e10;
    border: 1px solid #1f2429;
    border-radius: 4px;
    height: 220px;
    min-height: 120px;
    /* Drag the bottom edge to read more of the scrollback at once. */
    resize: vertical;
    overflow-y: auto;
    padding: 8px 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 14px;
    color: #c5d1de;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  .history-input {
    width: 56px;
    background: #0c0e10;
    border: 1px solid #3a4452;
    color: #e6e6e6;
    border-radius: 3px;
    padding: 1px 4px;
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  /* Shown only while scrolled up, to jump back to live decodes. */
  .jump-btn {
    position: absolute;
    right: 12px;
    bottom: 10px;
    background: #2a3f5f;
    border: 1px solid #4a90e2;
    color: #e6e6e6;
    padding: 3px 10px;
    border-radius: 12px;
    cursor: pointer;
    font-size: 11px;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
  }
  .jump-btn:hover {
    background: #35507a;
  }

  /* Transmitted text echoed live as it goes on the air. */
  .rx-text .tx {
    color: #ff9e64;
    font-weight: 600;
  }

  /* Decoded callsign — click to load it into the entry form. Rendered inline
     so it flows with the surrounding monospaced text. */
  .rx-text .call-chip {
    display: inline;
    font: inherit;
    color: #7ef0a8;
    background: rgba(74, 222, 128, 0.16);
    border: none;
    border-radius: 2px;
    padding: 0 1px;
    margin: 0;
    cursor: pointer;
  }
  .rx-text .call-chip:hover,
  .rx-text .call-chip:focus-visible {
    background: #4ade80;
    color: #07120a;
    outline: none;
  }

  /* The in-progress decode line, shown live before it's scored/committed. */
  .rx-text .pending::after {
    content: "▋";
    color: #4ade80;
    margin-left: 1px;
    animation: caret-blink 1s step-start infinite;
  }
  @keyframes caret-blink {
    50% {
      opacity: 0;
    }
  }
</style>
