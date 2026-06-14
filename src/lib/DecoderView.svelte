<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { onRtty, onTxEcho, scpContainsAny } from "$lib/tci";
  import { rttyConfig } from "$lib/rttyConfig.svelte";
  import { cluster } from "$lib/cluster.svelte";
  import TuningScope from "$lib/TuningScope.svelte";

  // Cap the total characters retained to keep DOM fast.
  const MAX_CHARS = 8000;

  // The decoder window is a sequence of runs so received and transmitted
  // text can be colored differently while sharing one scrollback. TX runs
  // are echoed live as our own signal goes on the air.
  type Run = { tx: boolean; s: string };
  let runs = $state<Run[]>([]);
  let autoScroll = $state(true);
  let filterNoise = $state(true);
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
    // Coalesce into the trailing run when the kind matches, so a long RX
    // stream doesn't fragment into thousands of spans.
    const last = runs[runs.length - 1];
    if (last && last.tx === tx) {
      last.s += s;
    } else {
      runs.push({ tx, s });
    }
    // Trim oldest runs to keep the total retained characters bounded.
    let total = runs.reduce((n, r) => n + r.s.length, 0);
    while (total > MAX_CHARS && runs.length > 0) {
      const over = total - MAX_CHARS;
      if (runs[0].s.length <= over) {
        total -= runs[0].s.length;
        runs.shift();
      } else {
        runs[0].s = runs[0].s.slice(over);
        total -= over;
      }
    }
    if (autoScroll) {
      queueMicrotask(() => {
        if (scrollEl) scrollEl.scrollTop = scrollEl.scrollHeight;
      });
    }
  }

  // Line-buffered filter pipeline. We process completed lines (terminated
  // by CR/LF) so we can score the whole line in context. A very long line
  // with no break is force-flushed so we don't accumulate forever.
  let lineBuf = "";
  let processing = false;

  async function appendChunk(chunk: string) {
    if (!filterNoise) {
      appendText(chunk);
      return;
    }
    lineBuf += chunk;
    if (processing) return;
    processing = true;
    try {
      // Keep draining; new chunks may have arrived while we awaited.
      while (await flushOneLine()) {
        // loop
      }
    } finally {
      processing = false;
    }
  }

  /// Pull one complete line (or force-flush an over-long buffer) and run it
  /// through the classifier. Returns true if more work *might* be pending.
  async function flushOneLine(): Promise<boolean> {
    const idx = lineBuf.search(/[\r\n]/);
    if (idx < 0) {
      if (lineBuf.length > 200) {
        const line = lineBuf;
        lineBuf = "";
        await classifyAndEmit(line);
      }
      return false;
    }
    const line = lineBuf.slice(0, idx);
    let n = 1;
    while (idx + n < lineBuf.length && /[\r\n]/.test(lineBuf[idx + n])) n++;
    lineBuf = lineBuf.slice(idx + n);
    if (line.trim().length > 0) await classifyAndEmit(line);
    return true;
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
    lineBuf = "";
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
      <label>
        <input type="checkbox" bind:checked={autoScroll} />
        auto-scroll
      </label>
      <button class="ghost" onclick={clear}>clear</button>
    </div>
  </header>
  <div class="rx-body">
    <TuningScope />
    <div class="rx-text" bind:this={scrollEl}
      >{#each runs as run}<span class:tx={run.tx}>{run.s}</span>{/each}{#if runs.length === 0}{" "}{/if}</div
    >
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

  .rx-text {
    flex: 1;
    background: #0c0e10;
    border: 1px solid #1f2429;
    border-radius: 4px;
    min-height: 150px;
    max-height: 220px;
    overflow-y: auto;
    padding: 8px 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 14px;
    color: #c5d1de;
    line-height: 1.5;
    white-space: pre-wrap;
    word-break: break-word;
  }

  /* Transmitted text echoed live as it goes on the air. */
  .rx-text .tx {
    color: #ff9e64;
    font-weight: 600;
  }
</style>
