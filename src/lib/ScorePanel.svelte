<script lang="ts">
  // Band summary + score for the active contest: QSOs, points and
  // multipliers by band, totals, and the claimed score; the QSO rate; and
  // the multipliers still needed on the band the entry form is on.
  import { scoreStore } from "$lib/score.svelte";
  import { cty } from "$lib/ctyStore.svelte";
  import { activeContest } from "$lib/contests";
  import { entryBus } from "$lib/entry.svelte";

  let s = $derived(scoreStore.score);
  let contest = $derived(activeContest());
  let perBand = $derived(s.kinds.filter((k) => k.perBand));
  let once = $derived(s.kinds.filter((k) => !k.perBand));
  // "123 Prefixes", or "(40 Zones + 98 Ctys + 52 St/Pr)" — each kind named,
  // so the multiplier count is never just a bare number.
  let multBreakdown = $derived.by(() => {
    const parts = s.kinds.map((k) => `${s.mults[k.key].toLocaleString()} ${k.label}`);
    return parts.length > 1 ? `(${parts.join(" + ")})` : parts[0] ?? "";
  });
  let rate = $derived(scoreStore.rate);
  let needed = $derived(scoreStore.needed.filter((n) => n.needed.length > 0));
  let band = $derived(entryBus.currentBand);
</script>

<section class="panel">
  <header>
    <h2>Score <span class="dim">· {contest.name}</span></h2>
    <div class="total">
      {#if s.kinds.length}
        <span class="dim">{s.points.toLocaleString()} pts × {multBreakdown} =</span>
      {:else if s.qsos}
        <span class="dim">{s.points.toLocaleString()} pts =</span>
      {/if}
      <span class="score">{s.score.toLocaleString()}</span>
    </div>
  </header>

  <div class="rate" title="QSOs in the last 10 and 60 minutes; the hourly rate is the last 10 minutes × 6">
    <span><span class="dim">Rate</span> <b>{rate.perHour10}</b><span class="dim">/h</span></span>
    <span><span class="dim">10 min</span> <b>{rate.last10}</b></span>
    <span><span class="dim">60 min</span> <b>{rate.last60}</b></span>
  </div>

  {#if !cty.db}
    <div class="warn">No country file loaded — countries, zones and points can't be worked out.</div>
  {/if}

  {#if s.bands.length === 0}
    <div class="empty">No QSOs yet.</div>
  {:else}
    <table>
      <thead>
        <tr>
          <th>Band</th>
          <th class="num">QSOs</th>
          <th class="num">Pts</th>
          {#each perBand as k}<th class="num">{k.label}</th>{/each}
        </tr>
      </thead>
      <tbody>
        {#each s.bands as b}
          <tr>
            <td class="band">{b.band}</td>
            <td class="num">{b.qsos}</td>
            <td class="num">{b.points}</td>
            {#each perBand as k}<td class="num">{b.mults[k.key]}</td>{/each}
          </tr>
        {/each}
      </tbody>
      <tfoot>
        <tr>
          <td>Total</td>
          <td class="num">{s.qsos}</td>
          <td class="num">{s.points}</td>
          {#each perBand as k}<td class="num">{s.mults[k.key]}</td>{/each}
        </tr>
      </tfoot>
    </table>
    {#if once.length}
      <div class="once">
        {#each once as k}
          <span class="once-kind"><span class="dim">{k.label}</span> <b>{s.mults[k.key]}</b></span>
        {/each}
        <span class="dim">(counted once per contest)</span>
      </div>
    {/if}
    {#if s.dupes}
      <div class="dim small">{s.dupes} dupe{s.dupes === 1 ? "" : "s"} not counted</div>
    {/if}
  {/if}

  {#if band && needed.length}
    <div class="needed">
      <h3>Needed{needed.some((n) => n.kind.perBand) ? ` on ${band}` : ""}</h3>
      {#each needed as n}
        <div class="need-row">
          <span class="dim need-kind">
            {n.kind.label}{#if n.elsewhere}{" "}<span title="Worked on another band but not on {band}">(other bands)</span>{/if}
            · {n.needed.length}
          </span>
          <span class="need-vals">{n.needed.join(" ")}</span>
        </div>
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
    align-items: baseline;
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
  .dim { color: #6b7176; font-weight: 400; text-transform: none; letter-spacing: 0; }
  .small { font-size: 11px; margin-top: 4px; }
  .total { display: flex; align-items: baseline; gap: 8px; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; }
  .score { color: #4ade80; font-size: 20px; font-weight: 700; }
  .warn { color: #fbbf24; font-size: 12px; margin-bottom: 6px; }
  .empty { color: #6b7176; font-size: 12px; font-style: italic; }
  table {
    border-collapse: collapse;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    min-width: min(320px, 100%);
  }
  th, td { padding: 3px 12px 3px 0; text-align: left; }
  th { color: #6b7176; font-weight: 500; font-size: 10px; text-transform: uppercase; }
  .num { text-align: right; }
  td.band { color: #fbbf24; }
  tbody td { color: #c5d1de; }
  tfoot td { color: #e6e6e6; font-weight: 600; border-top: 1px solid #2a2f33; }
  .rate { display: flex; gap: 14px; margin-bottom: 8px; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; color: #e6e6e6; }
  .rate b { color: #92c5fa; }
  .needed { margin-top: 10px; border-top: 1px solid #2a2f33; padding-top: 6px; max-height: 160px; overflow-y: auto; }
  h3 { margin: 0 0 4px; font-size: 10px; text-transform: uppercase; letter-spacing: 1px; color: #6b7176; font-weight: 600; }
  .need-row { display: flex; gap: 8px; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 11px; line-height: 1.5; }
  .need-kind { flex: 0 0 auto; white-space: nowrap; }
  .need-vals { color: #fbbf24; word-break: break-word; }
  .once { display: flex; gap: 14px; margin-top: 6px; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; font-size: 12px; color: #e6e6e6; }
</style>
