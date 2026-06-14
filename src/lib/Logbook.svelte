<script lang="ts">
  import { tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { save as saveDialog } from "@tauri-apps/plugin-dialog";
  import { qsoLog } from "$lib/qsoLog.svelte";
  import { bandFromHz, fmtMhz } from "$lib/bands";
  import { toAdif, toCabrillo } from "$lib/exports";
  import { settings } from "$lib/settings.svelte";
  import type { Qso } from "$lib/types";

  type EditField = "call" | "freq" | "sent" | "rcvd";
  let editing = $state<{ id: string; field: EditField } | null>(null);
  let editValue = $state("");
  let editInputEl: HTMLInputElement | null = $state(null);

  function isEditing(id: string, field: EditField): boolean {
    return editing?.id === id && editing.field === field;
  }

  async function startEdit(q: Qso, field: EditField) {
    editing = { id: q.id, field };
    editValue = initialValue(q, field);
    await tick();
    editInputEl?.focus();
    editInputEl?.select();
  }

  function initialValue(q: Qso, field: EditField): string {
    switch (field) {
      case "call": return q.call;
      case "freq": return fmtMhz(q.freqHz);
      case "sent": return `${q.rstSent} ${q.exchSent}`.trim();
      case "rcvd": return `${q.rstRcvd} ${q.exchRcvd}`.trim();
    }
  }

  function cancelEdit() {
    editing = null;
    editValue = "";
  }

  function commitEdit() {
    if (!editing) return;
    const { id, field } = editing;
    const patch: Partial<Qso> = {};
    switch (field) {
      case "call":
        patch.call = editValue.trim().toUpperCase().replace(/[^A-Z0-9/]/g, "");
        break;
      case "freq": {
        const hz = parseFreq(editValue);
        if (hz !== null) {
          patch.freqHz = hz;
          patch.band = bandFromHz(hz);
        }
        break;
      }
      case "sent": {
        const [rst, ...rest] = editValue.trim().split(/\s+/);
        patch.rstSent = rst || "";
        patch.exchSent = rest.join(" ");
        break;
      }
      case "rcvd": {
        const [rst, ...rest] = editValue.trim().split(/\s+/);
        patch.rstRcvd = rst || "";
        patch.exchRcvd = rest.join(" ");
        break;
      }
    }
    qsoLog.update(id, patch);
    cancelEdit();
  }

  // Accept the common ways an operator types a frequency:
  //   "14.080.500" → MHz.kHz.Hz (matches fmtMhz output, round-trips)
  //   "14080.5"    → kHz
  //   "14.0805"    → MHz (any decimal w/ a single dot)
  //   "14080500"   → raw Hz
  function parseFreq(raw: string): number | null {
    const s = raw.trim();
    if (!s) return null;
    if (/^\d+\.\d{1,3}\.\d{1,3}$/.test(s)) {
      const [mhz, khz, hz] = s.split(".").map(Number);
      return mhz * 1_000_000 + khz * 1000 + hz;
    }
    const n = parseFloat(s);
    if (!isFinite(n) || n <= 0) return null;
    if (s.includes(".")) return Math.round(n * 1_000_000); // MHz w/ decimal
    if (n < 100_000) return Math.round(n * 1000);          // kHz integer
    return Math.round(n);                                  // raw Hz
  }

  function handleEditKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commitEdit();
    } else if (e.key === "Escape") {
      e.preventDefault();
      cancelEdit();
    }
  }

  function fmtTime(ts: number): string {
    const d = new Date(ts);
    return d
      .toISOString()
      .slice(11, 19)
      .replace(/:/g, "");
  }

  let exporting = $state(false);
  let exportMsg = $state<string | null>(null);

  function defaultFilename(ext: string): string {
    const call = (settings.myCall || "diddle").toLowerCase();
    const now = new Date();
    const y = now.getUTCFullYear();
    const m = (now.getUTCMonth() + 1).toString().padStart(2, "0");
    const d = now.getUTCDate().toString().padStart(2, "0");
    return `${call}-${y}${m}${d}.${ext}`;
  }

  async function exportTo(
    ext: string,
    label: string,
    build: () => string,
  ) {
    if (qsoLog.qsos.length === 0) return;
    exporting = true;
    exportMsg = null;
    try {
      const path = await saveDialog({
        defaultPath: defaultFilename(ext),
        filters: [{ name: label, extensions: [ext] }],
      });
      if (!path) {
        exporting = false;
        return;
      }
      const content = build();
      await invoke("save_file_text", { path, content });
      exportMsg = `Saved ${qsoLog.qsos.length} QSOs to ${path}`;
    } catch (e: any) {
      exportMsg = `Export failed: ${e}`;
    } finally {
      exporting = false;
    }
  }

  function exportAdif() {
    exportTo("adi", "ADIF", () => toAdif(qsoLog.qsos));
  }
  function exportCabrillo() {
    exportTo("log", "Cabrillo", () => toCabrillo(qsoLog.qsos));
  }
</script>

<section class="panel">
  <header class="head">
    <h2>Log <span class="count">({qsoLog.qsos.length})</span></h2>
    <div class="tools">
      {#if qsoLog.qsos.length > 0}
        <button
          class="ghost"
          onclick={exportAdif}
          disabled={exporting}
          title="Export log as ADIF"
        >
          Export ADIF
        </button>
        <button
          class="ghost"
          onclick={exportCabrillo}
          disabled={exporting}
          title="Export log as Cabrillo (contest submission)"
        >
          Export Cabrillo
        </button>
        <button
          class="ghost danger"
          onclick={() => qsoLog.clear()}
          title="Wipe log (in memory + disk)"
        >
          Clear
        </button>
      {/if}
    </div>
  </header>

  {#if exportMsg}
    <div class="export-msg">{exportMsg}</div>
  {/if}

  {#if qsoLog.qsos.length === 0}
    <div class="empty">No QSOs yet. Type a callsign + exchange + Enter to log.</div>
  {:else}
    <div class="table-wrap">
      <table>
        <thead>
          <tr>
            <th class="num">#</th>
            <th>Time (UTC)</th>
            <th>Call</th>
            <th>Band</th>
            <th class="num">Freq</th>
            <th>Sent</th>
            <th>Rcvd</th>
            <th></th>
          </tr>
        </thead>
        <tbody>
          {#each [...qsoLog.qsos].reverse() as q (q.id)}
            <tr>
              <td class="num dim">{q.serialSent}</td>
              <td class="mono">{fmtTime(q.ts)}</td>
              <td class="call editable" onclick={() => startEdit(q, "call")}>
                {#if isEditing(q.id, "call")}
                  <input
                    bind:this={editInputEl}
                    bind:value={editValue}
                    onblur={commitEdit}
                    onkeydown={handleEditKey}
                    onclick={(e) => e.stopPropagation()}
                  />
                {:else}
                  {q.call}
                {/if}
              </td>
              <td class="band">{q.band}</td>
              <td
                class="num mono editable"
                onclick={() => startEdit(q, "freq")}
              >
                {#if isEditing(q.id, "freq")}
                  <input
                    bind:this={editInputEl}
                    bind:value={editValue}
                    onblur={commitEdit}
                    onkeydown={handleEditKey}
                    onclick={(e) => e.stopPropagation()}
                  />
                {:else}
                  {fmtMhz(q.freqHz)}
                {/if}
              </td>
              <td class="mono editable" onclick={() => startEdit(q, "sent")}>
                {#if isEditing(q.id, "sent")}
                  <input
                    bind:this={editInputEl}
                    bind:value={editValue}
                    onblur={commitEdit}
                    onkeydown={handleEditKey}
                    onclick={(e) => e.stopPropagation()}
                  />
                {:else}
                  {q.rstSent} {q.exchSent}
                {/if}
              </td>
              <td class="mono editable" onclick={() => startEdit(q, "rcvd")}>
                {#if isEditing(q.id, "rcvd")}
                  <input
                    bind:this={editInputEl}
                    bind:value={editValue}
                    onblur={commitEdit}
                    onkeydown={handleEditKey}
                    onclick={(e) => e.stopPropagation()}
                  />
                {:else}
                  {q.rstRcvd} {q.exchRcvd}
                {/if}
              </td>
              <td class="actions">
                <button
                  class="del"
                  onclick={() => qsoLog.remove(q.id)}
                  title="Remove QSO">×</button
                >
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
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

  .count { color: #6b7176; font-weight: 400; }

  .tools button.ghost {
    background: transparent;
    border: 1px solid #3a4452;
    color: #8a949d;
    padding: 3px 10px;
    border-radius: 3px;
    cursor: pointer;
    font-size: 11px;
  }
  .tools button.danger:hover {
    border-color: #f87171;
    color: #f87171;
  }

  .empty {
    color: #6b7176;
    font-size: 12px;
    font-style: italic;
    padding: 16px 0;
  }

  .export-msg {
    color: #4ade80;
    font-size: 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    padding: 6px 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tools {
    display: flex;
    gap: 4px;
  }

  .table-wrap {
    max-height: 280px;
    overflow-y: auto;
    border-radius: 4px;
    border: 1px solid #1f2429;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  thead th {
    text-align: left;
    color: #6b7176;
    background: #0e1113;
    padding: 6px 10px;
    font-weight: 600;
    text-transform: uppercase;
    font-size: 10px;
    letter-spacing: 0.5px;
    position: sticky;
    top: 0;
    border-bottom: 1px solid #2a2f33;
  }

  th.num, td.num { text-align: right; }

  tbody td {
    padding: 6px 10px;
    border-bottom: 1px solid #1a1e21;
  }

  tr:last-child td { border-bottom: none; }
  tr:hover td { background: #1c2024; }

  .mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  .dim { color: #6b7176; }
  .call { font-weight: 600; color: #e6e6e6; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  .band { color: #fbbf24; font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }

  .actions { text-align: right; width: 30px; }

  .del {
    background: transparent;
    border: none;
    color: #5a636c;
    cursor: pointer;
    font-size: 16px;
    line-height: 1;
    padding: 0 4px;
  }
  .del:hover { color: #f87171; }

  .editable {
    cursor: text;
  }
  .editable:hover {
    background: #20262b;
    box-shadow: inset 0 0 0 1px #3a4452;
  }
  .editable input {
    background: #0c0e10;
    border: 1px solid #4a90e2;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 2px 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    width: 100%;
    box-sizing: border-box;
  }
  .editable input:focus {
    outline: none;
  }
  td.num.editable input {
    text-align: right;
  }
</style>
