<script lang="ts">
  import { parseFreqInput } from "$lib/freq";
  import { tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { save as saveDialog, open as openDialog } from "@tauri-apps/plugin-dialog";
  import { qsoLog, type LogInfo } from "$lib/qsoLog.svelte";
  import { CONTESTS, activeContest } from "$lib/contests";
  import { importAdif } from "$lib/adifImport";
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
        const hz = parseFreqInput(editValue);
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
    const log = (qsoLog.meta?.name || "")
      .toLowerCase()
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "");
    if (log) return `${call}-${log}.${ext}`;
    const now = new Date();
    const y = now.getUTCFullYear();
    const m = (now.getUTCMonth() + 1).toString().padStart(2, "0");
    const d = now.getUTCDate().toString().padStart(2, "0");
    return `${call}-${y}${m}${d}.${ext}`;
  }

  // ---- Named logs: one per contest, any earlier one can be reopened ----

  function ymd(ts: number): string {
    return new Date(ts).toISOString().slice(0, 10);
  }
  function contestName(id: string): string {
    return CONTESTS.find((c) => c.id === id)?.name ?? "";
  }
  function logDates(l: LogInfo): string {
    if (l.firstTs == null || l.lastTs == null) return "empty";
    const a = ymd(l.firstTs), b = ymd(l.lastTs);
    return a === b ? a : `${a} – ${b}`;
  }
  // The open log's row shows live numbers, not the last listing's.
  function liveInfo(l: LogInfo): LogInfo {
    if (l.id !== qsoLog.meta?.id) return l;
    const ts = qsoLog.qsos.map((q) => q.ts);
    return {
      ...l,
      count: ts.length,
      firstTs: ts.length ? Math.min(...ts) : null,
      lastTs: ts.length ? Math.max(...ts) : null,
    };
  }

  let showLogs = $state(false);
  let naming = $state<"new" | "rename" | null>(null);
  let nameValue = $state("");
  let nameInputEl: HTMLInputElement | null = $state(null);
  let pendingDeleteId = $state<string | null>(null);
  let confirmClear = $state(false);
  let clearTimer: ReturnType<typeof setTimeout> | null = null;

  async function startNaming(kind: "new" | "rename") {
    naming = kind;
    nameValue =
      kind === "new" ? `${activeContest().name} ${ymd(Date.now())}` : qsoLog.meta?.name ?? "";
    await tick();
    nameInputEl?.focus();
    nameInputEl?.select();
  }

  async function commitName() {
    const n = nameValue.trim();
    if (!n) return;
    if (naming === "new") {
      if (await qsoLog.create(n, settings.activeContest)) {
        exportMsg = `New log "${n}" — ${activeContest().name}`;
      }
    } else if (naming === "rename" && qsoLog.meta) {
      await qsoLog.rename(qsoLog.meta.id, n);
    }
    naming = null;
  }

  function onNameKey(e: KeyboardEvent) {
    if (e.key === "Enter") {
      e.preventDefault();
      commitName();
    } else if (e.key === "Escape") {
      e.preventDefault();
      naming = null;
    }
  }

  // Opening a log brings back its contest too, so the score, mults and
  // Cabrillo export are figured the way they were.
  async function openLog(id: string) {
    cancelEdit();
    const meta = await qsoLog.open(id);
    if (meta?.contestId && meta.contestId !== settings.activeContest) {
      settings.setActiveContest(meta.contestId);
    }
    pendingDeleteId = null;
  }

  async function deleteLog(id: string) {
    if (pendingDeleteId !== id) {
      pendingDeleteId = id;
      return;
    }
    pendingDeleteId = null;
    await qsoLog.removeLog(id);
  }

  // Import an ADIF file (Diddle's own export, edited or not, or another
  // logger's) as a new log, named after the file.
  async function importAdifFile() {
    exportMsg = null;
    try {
      const path = await openDialog({
        multiple: false,
        filters: [{ name: "ADIF", extensions: ["adi", "adif", "ADI", "ADIF"] }],
      });
      if (!path || Array.isArray(path)) return;
      const text = await invoke<string>("read_file_text", { path });
      const r = importAdif(text, bandFromHz);
      if (r.qsos.length === 0) {
        exportMsg = `No QSOs found in ${path}`;
        return;
      }
      const contest =
        CONTESTS.find((c) => c.cabrilloName && c.cabrilloName === r.contestId.toUpperCase())?.id ??
        settings.activeContest;
      const name = String(path).split(/[\\/]/).pop()!.replace(/\.[^.]+$/, "");
      if (await qsoLog.create(name, contest, r.qsos)) {
        if (contest !== settings.activeContest) settings.setActiveContest(contest);
        exportMsg = `Imported ${r.qsos.length} QSOs into new log "${name}"${r.skipped ? ` (${r.skipped} records skipped: no call or date)` : ""}`;
      }
    } catch (e) {
      exportMsg = `Import failed: ${e}`;
    }
  }

  function clearLog() {
    if (!confirmClear) {
      confirmClear = true;
      if (clearTimer) clearTimeout(clearTimer);
      clearTimer = setTimeout(() => (confirmClear = false), 3000);
      return;
    }
    confirmClear = false;
    qsoLog.clear();
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

<section class="panel" style={settings.fontStyle("log")}>
  <header class="head">
    <div class="log-pick">
      <h2>Log <span class="count">({qsoLog.qsos.length})</span></h2>
      {#if naming}
        <input
          class="log-name"
          bind:this={nameInputEl}
          bind:value={nameValue}
          onkeydown={onNameKey}
          placeholder="log name"
          maxlength="60"
        />
        <button class="ghost" onclick={commitName}>{naming === "new" ? "Create" : "Rename"}</button>
        <button class="ghost" onclick={() => (naming = null)}>Cancel</button>
      {:else}
        <select
          class="log-select"
          value={qsoLog.meta?.id ?? ""}
          title="Open another log"
          onchange={(e) => openLog((e.target as HTMLSelectElement).value)}
        >
          {#each qsoLog.logs as l (l.id)}
            <option value={l.id}>{l.name}</option>
          {/each}
        </select>
        <button class="ghost" onclick={() => startNaming("new")} title="Start a new, empty log — for a new contest. The current log is kept and can be opened again from the list.">New log</button>
        <button class="ghost" class:on={showLogs} onclick={() => { showLogs = !showLogs; if (showLogs) qsoLog.refreshLogs(); }} title="All logs: open an earlier one, rename, delete, import ADIF">Logs…</button>
      {/if}
    </div>
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
          class:confirm={confirmClear}
          onclick={clearLog}
          title="Delete every QSO in this log (click twice). To start a new contest, use New log instead — it keeps this one."
        >
          {confirmClear ? "Click again to clear" : "Clear"}
        </button>
      {/if}
    </div>
  </header>

  {#if showLogs}
    <div class="logs-panel">
      <div class="logs-actions">
        <button class="ghost" onclick={() => startNaming("rename")} disabled={!qsoLog.meta}>Rename this log</button>
        <button class="ghost" onclick={importAdifFile} title="Read an ADIF file (a Diddle export, edited or not, or another logger's) into a new log">Import ADIF…</button>
      </div>
      <table class="logs">
        <thead>
          <tr><th>Log</th><th>Contest</th><th class="num">QSOs</th><th>Dates (UTC)</th><th></th></tr>
        </thead>
        <tbody>
          {#each qsoLog.logs.map(liveInfo) as l (l.id)}
            <tr class:current={l.id === qsoLog.meta?.id}>
              <td>{l.name}</td>
              <td class="dim">{contestName(l.contestId) || "—"}</td>
              <td class="num">{l.count}</td>
              <td class="mono dim">{logDates(l)}</td>
              <td class="actions">
                {#if l.id === qsoLog.meta?.id}
                  <span class="open-tag">open</span>
                {:else}
                  <button class="ghost" onclick={() => openLog(l.id)}>Open</button>
                  <button class="ghost danger" class:confirm={pendingDeleteId === l.id} onclick={() => deleteLog(l.id)}>
                    {pendingDeleteId === l.id ? "Sure?" : "Delete"}
                  </button>
                {/if}
              </td>
            </tr>
          {/each}
        </tbody>
      </table>
    </div>
  {/if}

  {#if qsoLog.lastError}
    <div class="export-msg err">{qsoLog.lastError}</div>
  {/if}
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
                  {#if q.note}
                    <span class="note-mark" title={q.note}>✎</span>
                  {/if}
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
  .tools button.danger:hover,
  button.danger.confirm {
    border-color: #f87171;
    color: #f87171;
  }

  .log-pick { display: flex; align-items: center; gap: 6px; min-width: 0; }
  .log-pick button.ghost,
  .logs-panel button.ghost {
    background: transparent;
    border: 1px solid #3a4452;
    color: #8a949d;
    padding: 3px 10px;
    border-radius: 3px;
    cursor: pointer;
    font-size: 11px;
    white-space: nowrap;
  }
  .log-pick button.ghost.on { border-color: #4a90e2; color: #e6e6e6; }
  .logs-panel button.danger:hover { border-color: #f87171; color: #f87171; }
  .log-select,
  .log-name {
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 3px 6px;
    font-size: 12px;
    max-width: 260px;
    min-width: 0;
  }
  .log-name { width: 240px; }

  .logs-panel {
    border: 1px solid #262b30;
    border-radius: 4px;
    padding: 8px;
    margin-bottom: 8px;
    background: #121518;
  }
  .logs-actions { display: flex; gap: 6px; margin-bottom: 6px; }
  table.logs { font-size: 12px; }
  table.logs tr.current td { color: #e6e6e6; background: #1a2230; }
  table.logs td.actions { display: flex; gap: 4px; justify-content: flex-end; }
  .open-tag { color: #4ade80; font-size: 11px; }
  .export-msg.err { color: #f87171; }

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
    font-size: var(--win-size, 12px);
    font-variant-numeric: var(--win-zero, normal);
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

  .mono { font-family: var(--win-font, ui-monospace, SFMono-Regular, Menlo, monospace); }
  .dim { color: #6b7176; }
  .call { font-weight: 600; color: #e6e6e6; font-family: var(--win-font, ui-monospace, SFMono-Regular, Menlo, monospace); }
  .band { color: #fbbf24; font-family: var(--win-font, ui-monospace, SFMono-Regular, Menlo, monospace); }

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

  .note-mark {
    color: #fbbf24;
    font-size: 11px;
    margin-left: 4px;
    cursor: help;
  }

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
