<script lang="ts">
  import { setFreq, type RigState } from "$lib/tci";
  import { cluster, type ClusterSpot } from "$lib/cluster.svelte";
  import { spots as decoderSpots } from "$lib/spots.svelte";
  import { qsoLog } from "$lib/qsoLog.svelte";
  import { rttyConfig } from "$lib/rttyConfig.svelte";
  import { bandFromHz, fmtMhz } from "$lib/bands";

  let { rig }: { rig: RigState } = $props();

  type Source = "cluster" | "decoder" | "log";

  interface BandmapRow {
    call: string;
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
  let rows = $derived.by<BandmapRow[]>(() => {
    const map = new Map<string, BandmapRow>();
    const band = currentBand;
    const workedCallsThisBand = new Set(
      qsoLog.qsos
        .filter((q) => q.band === band)
        .map((q) => q.call.toUpperCase()),
    );

    // Worked stations (always-on, even when no spots exist)
    for (const q of qsoLog.qsos) {
      if (q.band !== band) continue;
      const key = q.call.toUpperCase();
      if (!map.has(key)) {
        map.set(key, {
          call: key,
          freqHz: q.freqHz,
          source: "log",
          timestamp: q.ts,
          worked: true,
        });
      }
    }

    // Multi-decoder spots — current band only.
    for (const s of decoderSpots.spots) {
      const freqHz = (rig.freq || 0) + s.audio_hz;
      if (bandFromHz(freqHz) !== band) continue;
      const key = s.call.toUpperCase();
      const entry: BandmapRow = {
        call: key,
        freqHz,
        source: "decoder",
        timestamp: s.timestamp_ms,
        worked: workedCallsThisBand.has(key),
      };
      const existing = map.get(key);
      if (!existing || existing.source === "log") {
        map.set(key, entry);
      }
    }

    // Cluster spots — current band only.
    for (const s of cluster.spots) {
      if (s.band !== band) continue;
      const key = s.dx_call.toUpperCase();
      const entry: BandmapRow = {
        call: key,
        freqHz: s.freq_hz,
        source: "cluster",
        timestamp: s.timestamp_ms,
        comment: s.comment,
        worked: workedCallsThisBand.has(key),
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
      Bandmap <span class="dim">· {currentBand}</span>
    </h2>
    <div class="legend">
      <span class="src-tag cluster">●</span> cluster
      <span class="src-tag decoder">●</span> decoder
      <span class="src-tag log">●</span> worked
      <span class="dim">({rows.length})</span>
    </div>
  </header>

  {#if rows.length === 0}
    <div class="empty">
      No spots on {currentBand} yet. Connect to a DX cluster (Settings) or
      let the multi-decoder find some signals on this band.
    </div>
  {:else}
    <div class="rows">
      {#each rows as r (r.call + "@" + r.freqHz)}
        <button
          class="row src-{r.source}"
          class:worked={r.worked}
          onclick={() => qsyTo(r)}
          title={r.comment || `QSY to ${fmtMhz(r.freqHz)}`}
        >
          <span class="src-tag {r.source}">●</span>
          <span class="freq">{fmtMhz(r.freqHz)}</span>
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
</style>
