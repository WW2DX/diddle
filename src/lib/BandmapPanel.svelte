<script lang="ts">
  import { setFreq, type RigState } from "$lib/tci";
  import { cluster, type ClusterSpot } from "$lib/cluster.svelte";
  import { spots as decoderSpots } from "$lib/spots.svelte";
  import { qsoLog } from "$lib/qsoLog.svelte";
  import { rttyConfig } from "$lib/rttyConfig.svelte";
  import { bandFromHz, fmtMhz } from "$lib/bands";
  import { rfFromAudio, dialForRf } from "$lib/freq";
  import { settings } from "$lib/settings.svelte";

  let { rig }: { rig: RigState } = $props();

  type Source = "cluster" | "decoder" | "log";

  interface BandmapRow {
    call: string;
    band: string;
    freqHz: number;
    source: Source;
    timestamp: number;
    comment?: string;
    worked: boolean;
  }

  let currentBand = $derived(bandFromHz(rig.freq));

  // Decoder spots (from MultiDecoder) carry audio_hz. Anchor them to a
  // radio frequency snapshot taken when each spot arrived. For simplicity
  // we use the *current* rig.freq — works fine as long as the user isn't
  // mid-QSY when spots are generated.
  // Current band only (default) or every band — the latter shows mults
  // popping up elsewhere and how busy a band is before you QSY there.
  let allBands = $derived(settings.bandmapAllBands);

  let rows = $derived.by<BandmapRow[]>(() => {
    const map = new Map<string, BandmapRow>();
    const band = currentBand;
    const wants = (b: string) => allBands || b === band;
    // Worked-status is per band: "W1AW@20m".
    const worked = new Set(
      qsoLog.qsos.map((q) => `${q.call.toUpperCase()}@${q.band}`),
    );
    const keyOf = (call: string, b: string) => (allBands ? `${call}@${b}` : call);

    // Worked stations (always-on, even when no spots exist)
    for (const q of qsoLog.qsos) {
      if (!wants(q.band)) continue;
      const call = q.call.toUpperCase();
      const key = keyOf(call, q.band);
      if (!map.has(key)) {
        map.set(key, {
          call,
          band: q.band,
          freqHz: q.freqHz,
          source: "log",
          timestamp: q.ts,
          worked: true,
        });
      }
    }

    // Multi-decoder spots — always on the current band (they're audio offsets).
    for (const s of decoderSpots.spots) {
      const freqHz = rfFromAudio(rig.freq || 0, s.audio_hz, rig.mode);
      const b = bandFromHz(freqHz);
      if (!wants(b)) continue;
      const call = s.call.toUpperCase();
      const key = keyOf(call, b);
      const entry: BandmapRow = {
        call,
        band: b,
        freqHz,
        source: "decoder",
        timestamp: s.timestamp_ms,
        worked: worked.has(`${call}@${b}`),
      };
      const existing = map.get(key);
      if (!existing || existing.source === "log") {
        map.set(key, entry);
      }
    }

    // Cluster spots.
    for (const s of cluster.spots) {
      if (!wants(s.band)) continue;
      const call = s.dx_call.toUpperCase();
      const key = keyOf(call, s.band);
      const entry: BandmapRow = {
        call,
        band: s.band,
        freqHz: s.freq_hz,
        source: "cluster",
        timestamp: s.timestamp_ms,
        comment: s.comment,
        worked: worked.has(`${call}@${s.band}`),
      };
      const existing = map.get(key);
      // Cluster wins over decoder + log because it usually carries comment
      // and accurate frequency.
      if (!existing || existing.source !== "cluster") {
        map.set(key, entry);
      }
    }

    return [...map.values()].sort((a, b) => a.freqHz - b.freqHz);
  });

  async function qsyTo(row: BandmapRow) {
    if (!rig.freq) return;
    // Tune so the signal lands at the user's chosen mark tone (USB).
    const newVfo = Math.round(row.freqHz - rttyConfig.markHz);
    try {
      await setFreq(newVfo);
    } catch (e) {
      console.error("set_freq failed", e);
    }
  }

  function ago(ts: number): string {
    const s = Math.floor((Date.now() - ts) / 1000);
    if (s < 60) return `${s}s`;
    if (s < 3600) return `${Math.floor(s / 60)}m`;
    return `${Math.floor(s / 3600)}h`;
  }
</script>

<section class="panel">
  <header>
    <h2>
      Bandmap <span class="dim">· {allBands ? "all bands" : currentBand}</span>
    </h2>
    <div class="legend">
      <button
        class="band-toggle"
        class:on={allBands}
        onclick={() => settings.toggleBandmapAllBands()}
        title="Show spots from every band, or only the band the radio is on"
      >
        {allBands ? "all bands" : currentBand + " only"}
      </button>
      <span class="src-tag cluster">●</span> cluster
      <span class="src-tag decoder">●</span> decoder
      <span class="src-tag log">●</span> worked
      <span class="dim">({rows.length})</span>
    </div>
  </header>

  {#if rows.length === 0}
    <div class="empty">
      No spots {allBands ? "" : `on ${currentBand} `}yet. Connect to a DX cluster (Settings) or
      let the multi-decoder find some signals on this band.
    </div>
  {:else}
    <div class="rows">
      {#each rows as r (r.call + "@" + r.freqHz)}
        <button
          class="row src-{r.source}"
          class:worked={r.worked}
          class:offband={allBands && r.band !== currentBand}
          onclick={() => qsyTo(r)}
          title={r.comment || `QSY to ${fmtMhz(r.freqHz)}`}
        >
          <span class="src-tag {r.source}">●</span>
          <span class="freq">{fmtMhz(r.freqHz)}{#if allBands}<span class="band-col"> {r.band}</span>{/if}</span>
          <span class="call">{r.call}</span>
          <span class="comment">{r.comment || ""}</span>
          <span class="age">{ago(r.timestamp)}</span>
        </button>
      {/each}
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

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
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
  .dim { color: #6b7176; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }

  .legend {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 11px;
    color: #8a949d;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .src-tag {
    font-size: 10px;
  }
  .src-tag.cluster { color: #4a90e2; }
  .src-tag.decoder { color: #4ade80; }
  .src-tag.log     { color: #6b7176; }

  .empty {
    color: #6b7176;
    font-size: 12px;
    font-style: italic;
    padding: 16px 0;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 1px;
    max-height: 320px;
    overflow-y: auto;
    border: 1px solid #1f2429;
    border-radius: 4px;
    background: #0c0e10;
  }

  .row {
    display: grid;
    grid-template-columns: 14px 110px 100px 1fr 40px;
    align-items: center;
    gap: 10px;
    background: transparent;
    border: none;
    color: #c5d1de;
    padding: 5px 10px;
    cursor: pointer;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    text-align: left;
    border-bottom: 1px solid #161a1d;
  }
  .row:last-child { border-bottom: none; }
  .row:hover { background: #1c2024; }
  .row.worked { opacity: 0.55; }

  .freq { color: #e6e6e6; font-weight: 600; }
  .call { color: #fbbf24; font-weight: 600; letter-spacing: 0.5px; }
  .row.src-cluster .call { color: #92c5fa; }
  .row.src-decoder .call { color: #4ade80; }
  .row.worked .call { color: #6b7176; text-decoration: line-through; }

  .comment {
    color: #8a949d;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .age {
    color: #5a636c;
    font-size: 11px;
    text-align: right;
  }
  .band-col { color: #8a949d; font-weight: 400; font-size: 10px; }
  .row.offband .freq { color: #b8c2cc; }

  .band-toggle {
    background: transparent;
    border: 1px solid #3a4452;
    color: #8a949d;
    border-radius: 3px;
    padding: 2px 8px;
    font-size: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    cursor: pointer;
    margin-right: 6px;
  }
  .band-toggle:hover { color: #c5d1de; background: #1c2024; }
  .band-toggle.on { border-color: #4a90e2; color: #92c5fa; }

  .cmdline {
    margin-top: 8px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .cmd-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .prompt { font-size: 11px; }
  .cmd-row input {
    flex: 1;
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 5px 8px;
    font-family: inherit;
    font-size: 12px;
  }
  .cmd-row input:focus { outline: none; border-color: #4a90e2; }
  .cmd-row input:disabled { opacity: 0.5; }
  .cmd-row button.ghost {
    background: transparent;
    border: 1px solid #3a4452;
    color: #8a949d;
    border-radius: 3px;
    padding: 4px 10px;
    font-size: 11px;
    cursor: pointer;
  }
  .cmd-row button.ghost:hover:not(:disabled) { color: #c5d1de; background: #1c2024; }
  .cmd-row button.ghost:disabled { opacity: 0.4; cursor: default; }
  .cmd-error { color: #f87171; font-size: 11px; margin-top: 4px; }
  .tail {
    margin-top: 6px;
    max-height: 84px;
    overflow-y: auto;
    background: #0c0e10;
    border: 1px solid #1f2429;
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 11px;
    color: #8a949d;
  }
  .tail .line { white-space: pre-wrap; word-break: break-all; }
  .tail .line.tx { color: #92c5fa; }
</style>
