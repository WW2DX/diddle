<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { settings, FONT_WINS } from "$lib/settings.svelte";
  import { CONTESTS, activeContest } from "$lib/contests";
  import { listFonts, type FontFamily } from "$lib/tci";
  import { scpFile, historyFile } from "$lib/lookupFiles.svelte";
  import { cluster } from "$lib/cluster.svelte";
  import { macroState, keyLabel } from "$lib/macros.svelte";
  import { contestSetups } from "$lib/contestSetups.svelte";
  import { cty } from "$lib/ctyStore.svelte";
  import { chime } from "$lib/sound";

  // Show the format hint based on the current selection.
  let formatHint = $derived(activeContest().exchangeFormat);

  // SCP + call history live in lookupFiles (loaded at startup).
  let scp = $derived(scpFile.status);
  let scpLoading = $derived(scpFile.loading);
  let scpError = $derived(scpFile.error);
  let hist = $derived(historyFile.status);
  let histLoading = $derived(historyFile.loading);
  let histError = $derived(historyFile.error);

  async function pickHistoryFile() {
    const path = await openDialog({
      title: "Choose N1MM+ Call History file",
      multiple: false,
      filters: [{ name: "Call history / text", extensions: ["txt", "csv", "hist"] }],
    });
    if (!path || typeof path !== "string") return;
    await historyFile.load(path);
  }

  async function pickCtyFile() {
    const path = await openDialog({
      title: "Choose a country file (cty.dat or WL_CTY.DAT)",
      multiple: false,
      filters: [{ name: "Country file", extensions: ["dat", "DAT", "txt"] }],
    });
    if (!path || typeof path !== "string") return;
    await cty.loadFile(path);
  }

  // ---- Saved contest setups ----
  let setupName = $state("");
  let activeSetup = $derived(contestSetups.active);

  function saveSetup() {
    const name = setupName.trim() || activeSetup?.name || "";
    if (!name) return;
    contestSetups.saveAs(name);
    setupName = "";
  }

  async function pickSetup(id: string) {
    if (!id) {
      contestSetups.deactivate();
      return;
    }
    await contestSetups.activate(id);
  }

  // F-keys save as they're edited; the button makes it explicit (and
  // writes them into the active setup straight away).
  let macroSaveMsg = $state<string | null>(null);
  let macroSaveTimer: ReturnType<typeof setTimeout> | null = null;
  function saveMacros() {
    const ok = macroState.save();
    if (ok) contestSetups.syncActive();
    macroSaveMsg = ok
      ? `Saved ✓${activeSetup ? ` — also in setup “${activeSetup.name}”` : ""}`
      : "Couldn't save — see the log";
    if (macroSaveTimer) clearTimeout(macroSaveTimer);
    macroSaveTimer = setTimeout(() => (macroSaveMsg = null), 4000);
  }

  function deleteSetup() {
    if (!activeSetup) return;
    contestSetups.remove(activeSetup.id);
  }

  // Installed font families: fixed-pitch ones first, since contest windows
  // line up best in them, then everything else.
  let fonts = $state<FontFamily[]>([]);
  let monoFonts = $derived(fonts.filter((f) => f.mono));
  let otherFonts = $derived(fonts.filter((f) => !f.mono));

  onMount(async () => {
    listFonts()
      .then((fs) => (fonts = fs))
      .catch((e) => console.error("listFonts failed", e));
  });

  async function pickScpFile() {
    const path = await openDialog({
      title: "Choose MASTER.SCP file",
      multiple: false,
      filters: [{ name: "SCP / text", extensions: ["scp", "txt"] }],
    });
    if (!path || typeof path !== "string") return;
    await scpFile.load(path);
  }

  let clusterError = $state<string | null>(null);
  async function toggleCluster() {
    clusterError = null;
    try {
      if (
        cluster.state.kind === "connected" ||
        cluster.state.kind === "connecting"
      ) {
        await cluster.disconnect();
      } else {
        const login = settings.myCall || "TEST";
        await cluster.connect(
          settings.clusterHost,
          settings.clusterPort,
          login,
          settings.clusterLoginCommandList,
        );
      }
    } catch (e: any) {
      clusterError = String(e);
    }
  }
</script>

<section class="panel">
  <div class="grid">
    <div class="field">
      <label for="s-call">My call</label>
      <input
        id="s-call"
        type="text"
        value={settings.myCall}
        oninput={(e) =>
          settings.setMyCall((e.target as HTMLInputElement).value)}
        placeholder="W1AW"
      />
    </div>

    <div class="field">
      <label for="s-name">Name</label>
      <input
        id="s-name"
        type="text"
        value={settings.myName}
        oninput={(e) =>
          settings.setMyName((e.target as HTMLInputElement).value)}
        placeholder="JOHN"
      />
    </div>

    <div class="field small">
      <label for="s-state">State</label>
      <input
        id="s-state"
        type="text"
        value={settings.myState}
        oninput={(e) =>
          settings.setMyState((e.target as HTMLInputElement).value)}
        placeholder="MA"
        maxlength="4"
      />
    </div>

    <div class="field small">
      <label for="s-zone">CQ Zone</label>
      <input
        id="s-zone"
        type="text"
        value={settings.myZone}
        oninput={(e) =>
          settings.setMyZone((e.target as HTMLInputElement).value)}
        placeholder="5"
        maxlength="2"
      />
    </div>

    <div class="field small">
      <label for="s-grid">Grid</label>
      <input
        id="s-grid"
        type="text"
        value={settings.myGrid}
        oninput={(e) =>
          settings.setMyGrid((e.target as HTMLInputElement).value)}
        placeholder="FN42"
        maxlength="6"
      />
    </div>

    <div class="field contest">
      <label for="s-contest">Contest</label>
      <select
        id="s-contest"
        value={settings.activeContest}
        onchange={(e) =>
          settings.setActiveContest((e.target as HTMLSelectElement).value)}
      >
        {#each CONTESTS as c}
          <option value={c.id}>{c.name}</option>
        {/each}
      </select>
      <span class="hint">{formatHint}</span>
    </div>

    <div class="field esm-field">
      <label class="esm-toggle">
        <input
          type="checkbox"
          checked={settings.esm}
          onchange={(e) =>
            settings.setEsm((e.target as HTMLInputElement).checked)}
        />
        <span>ESM (Enter Sends Message)</span>
      </label>
      <span class="hint">
        Run: empty→CQ, call→Excg, his exch→TU+Log. S&amp;P: call→your call,
        his exch→Excg+Log (he sends the TU).
      </span>
    </div>
  </div>

  <div class="scp">
    <div class="scp-info">
      <span class="scp-label">SCP database</span>
      <span class="scp-count">{scp.count.toLocaleString()} callsigns</span>
      <span class="scp-source dim">
        from {scp.source.startsWith("file:") ? scp.source.slice(5) : scp.source}
      </span>
    </div>
    <div class="scp-actions">
      <button onclick={() => scpFile.autoDownload()} disabled={scpLoading}>
        {scpLoading ? "Working…" : "Update from web"}
      </button>
      <button class="ghost" onclick={pickScpFile} disabled={scpLoading}>
        Load file…
      </button>
      <span class="hint">
        Fetched from
        <span class="mono">supercheckpartial.com</span>
      </span>
    </div>
    {#if scpError}
      <div class="scp-error">{scpError}</div>
    {/if}
  </div>

  <div class="cluster">
    <div class="cluster-info">
      <span class="scp-label">DX cluster</span>
      <span class="cluster-state state-{cluster.state.kind}">
        {#if cluster.state.kind === "connected"}
          ● {cluster.state.host}:{cluster.state.port}
        {:else if cluster.state.kind === "connecting"}
          ◐ connecting to {cluster.state.host}:{cluster.state.port}…
        {:else if cluster.state.kind === "error"}
          ✕ {cluster.state.message}
        {:else}
          ○ disconnected
        {/if}
      </span>
      <span class="dim">{cluster.spots.length} spots cached</span>
    </div>

    <div class="cluster-row">
      <div class="field">
        <label for="c-host">Host</label>
        <input
          id="c-host"
          type="text"
          value={settings.clusterHost}
          oninput={(e) =>
            settings.setClusterHost((e.target as HTMLInputElement).value)}
          placeholder="dxc.k1ttt.net"
        />
      </div>
      <div class="field small">
        <label for="c-port">Port</label>
        <input
          id="c-port"
          type="number"
          value={settings.clusterPort}
          oninput={(e) =>
            settings.setClusterPort(
              parseInt((e.target as HTMLInputElement).value, 10),
            )}
          min="1"
          max="65535"
        />
      </div>
      <button class="cluster-btn" onclick={toggleCluster}>
        {cluster.state.kind === "connected" ||
        cluster.state.kind === "connecting"
          ? "Disconnect"
          : "Connect"}
      </button>
    </div>
    {#if clusterError}
      <div class="scp-error">{clusterError}</div>
    {/if}
    <div class="field login-cmds">
      <label for="c-login-cmds">Commands sent after login (one per line)</label>
      <textarea
        id="c-login-cmds"
        rows="3"
        value={settings.clusterLoginCommands}
        oninput={(e) =>
          settings.setClusterLoginCommands((e.target as HTMLTextAreaElement).value)}
        placeholder={"set/filter mode rtty\nset/filter band hf"}
        spellcheck="false"
      ></textarea>
      <span class="hint">
        Filters etc. — sent automatically each time Diddle logs in. Ad-hoc
        commands go in the <span class="mono">dx›</span> line under the bandmap.
      </span>
    </div>
  </div>

  <div class="cluster">
    <div class="cluster-info">
      <span class="scp-label">Call history</span>
      {#if hist.count > 0}
        <span class="scp-count">{hist.count.toLocaleString()} calls</span>
        <span class="scp-source dim">{hist.path}</span>
      {:else}
        <span class="dim">none loaded</span>
      {/if}
    </div>
    <div class="scp-actions">
      <button onclick={pickHistoryFile} disabled={histLoading}>
        {histLoading ? "Loading…" : "Load N1MM+ history file…"}
      </button>
      {#if hist.count > 0}
        <button class="ghost" onclick={() => historyFile.clear()}>Clear</button>
      {/if}
      <label class="stack-key" title="Which history column goes into Exch. Auto uses the contest's own fields (zone + state for CQ WW, name + state for NAQP…), or EXCH1 for a contest with none — so a file headed !!Order!!,Call,Exch1,UserText works for any contest.">
        Exch from
        <select
          value={settings.historyField}
          onchange={(e) => settings.setHistoryField((e.target as HTMLSelectElement).value)}
        >
          <option value="">auto</option>
          {#if settings.historyField && !hist.fields.some((f) => f.toLowerCase() === settings.historyField.toLowerCase())}
            <option value={settings.historyField}>{settings.historyField}</option>
          {/if}
          {#each hist.fields as f}
            <option value={f}>{f}</option>
          {/each}
        </select>
      </label>
      <span class="hint">
        Pre-fills Exch when a known call is typed or grabbed
        {#if hist.fields.length > 0}
          · fields: <span class="mono">{hist.fields.join(", ")}</span>
        {/if}
      </span>
    </div>
    {#if histError}
      <div class="scp-error">{histError}</div>
    {/if}
  </div>

  <div class="cluster setups">
    <div class="cluster-info">
      <span class="scp-label">Saved contest setups</span>
      <span class="hint">
        Contest + F-key messages + call history file. Pick one to load it;
        edits are saved back to the active setup.
      </span>
    </div>
    <div class="setup-row">
      <select
        value={activeSetup?.id || ""}
        onchange={(e) => pickSetup((e.target as HTMLSelectElement).value)}
      >
        <option value="">— none —</option>
        {#each contestSetups.setups as s (s.id)}
          <option value={s.id}>{s.name}</option>
        {/each}
      </select>
      <input
        type="text"
        bind:value={setupName}
        placeholder={activeSetup ? `rename / save as… (${activeSetup.name})` : "name, e.g. NAQP RTTY"}
        maxlength="40"
        onkeydown={(e) => e.key === "Enter" && saveSetup()}
      />
      <button class="cluster-btn" onclick={saveSetup} disabled={!setupName.trim() && !activeSetup}>
        {setupName.trim() ? "Save as" : "Save"}
      </button>
      {#if activeSetup}
        <button class="ghost del" onclick={deleteSetup} title="Delete this setup">✕</button>
      {/if}
    </div>
  </div>

  <div class="cluster">
    <div class="cluster-info">
      <span class="scp-label">Country file</span>
      {#if cty.db}
        <span class="scp-count">{cty.db.size} countries</span>
        <span class="scp-source dim">{cty.source === "custom" ? "your copy (downloaded / loaded)" : "bundled cty.dat"}</span>
      {:else}
        <span class="dim">not loaded</span>
      {/if}
    </div>
    <div class="scp-actions">
      <button onclick={() => cty.download()} disabled={cty.busy}>
        {cty.busy ? "Working…" : "Update from country-files.com"}
      </button>
      <button class="ghost" onclick={pickCtyFile} disabled={cty.busy}>Load cty.dat / WL_CTY.DAT…</button>
      {#if cty.source === "custom"}
        <button class="ghost" onclick={() => cty.reset()} disabled={cty.busy}>Use bundled</button>
      {/if}
      <span class="hint">Countries, CQ/ITU zones and continents — multipliers, points, zone prediction. AD1C, country-files.com.</span>
    </div>
    <label class="esm-toggle bell">
      <input
        type="checkbox"
        checked={settings.multBell}
        onchange={(e) => settings.setMultBell((e.target as HTMLInputElement).checked)}
      />
      Chime on a new multiplier
      <button type="button" class="ghost" onclick={(e) => { e.preventDefault(); chime(); }}>Test</button>
    </label>
    {#if cty.error}
      <div class="scp-error">{cty.error}</div>
    {/if}
  </div>

  <div class="cluster">
    <div class="cluster-info">
      <span class="scp-label">Display fonts</span>
      <span class="dim">per window · any installed font</span>
    </div>
    <div class="font-rows">
      {#each FONT_WINS as w}
        {@const f = settings.fonts[w.id]}
        <div class="font-row" style={settings.fontStyle(w.id)}>
          <span class="font-win">{w.label}</span>
          <select
            class="font-family"
            value={f.family}
            style={f.family ? `font-family: "${f.family}"` : ""}
            onchange={(e) => settings.setFont(w.id, { family: (e.target as HTMLSelectElement).value })}
          >
            <option value="">monospace (default)</option>
            {#if f.family && !fonts.some((x) => x.name === f.family)}
              <option value={f.family}>{f.family}</option>
            {/if}
            {#if monoFonts.length}
              <optgroup label="Fixed-pitch">
                {#each monoFonts as x (x.name)}
                  <option value={x.name}>{x.name}</option>
                {/each}
              </optgroup>
            {/if}
            {#if otherFonts.length}
              <optgroup label="Other fonts">
                {#each otherFonts as x (x.name)}
                  <option value={x.name}>{x.name}</option>
                {/each}
              </optgroup>
            {/if}
          </select>
          <input
            class="font-size"
            type="number"
            min="8"
            max="40"
            value={f.size || ""}
            placeholder={String(w.defaultSize)}
            onchange={(e) => {
              const v = parseInt((e.target as HTMLInputElement).value, 10);
              settings.setFont(w.id, { size: v >= 8 && v <= 40 ? v : 0 });
            }}
          />
          <span class="dim">px</span>
          <label class="font-zero" title="Ask the font for its slashed-zero variant. Only fonts that have one change (Source Code Pro, JetBrains Mono, Fira Code…). Unticked you get the font's own zero — Consolas and Menlo, for example, already slash theirs, so the box changes nothing there.">
            <input
              type="checkbox"
              checked={f.slashedZero}
              onchange={(e) => settings.setFont(w.id, { slashedZero: (e.target as HTMLInputElement).checked })}
            />
            force slashed 0
          </label>
          <span class="font-sample">K0ABC 599 05 NY</span>
        </div>
      {/each}
    </div>
  </div>

  <div class="macros">
    <div class="macros-head">
      <span class="scp-label">F-key macros</span>
      <span class="hint">
        <span class="mono">&lt;MYCALL&gt;</span>
        <span class="mono">&lt;NAME&gt;</span>
        <span class="mono">&lt;STATE&gt;</span>
        <span class="mono">&lt;CQZONE&gt;</span>
        <span class="mono">&lt;GRID&gt;</span> (yours, from above; grid as 4 characters)
        <span class="mono">&lt;CALL&gt;</span> (his)
        <span class="mono">&lt;SERIAL&gt;</span> are substituted at send time (case doesn't matter).
        <span class="mono">&lt;CRLF&gt;</span> new line ·
        <span class="mono">&lt;LOGIT&gt;</span> log the QSO ·
        <span class="mono">&lt;POPSTACK&gt;</span> load the next stacked caller (a later
        <span class="mono">&lt;CALL&gt;</span> is him).
      </span>
      <button class="ghost macro-reset-all" onclick={() => macroState.resetAll()}>
        Reset all
      </button>
    </div>
    <div class="macros-head">
      <label class="stack-key">
        ESM TU with callers stacked sends
        <select
          value={settings.stackTuKey}
          onchange={(e) => settings.setStackTuKey((e.target as HTMLSelectElement).value)}
        >
          {#each macroState.macros as m (m.key)}
            <option value={m.key}>{keyLabel(m.key)} · {m.label}</option>
          {/each}
        </select>
      </label>
      <span class="hint">
        e.g. <span class="mono">&lt;CRLF&gt;TU &lt;CALL&gt;&lt;LOGIT&gt; NOW&lt;CRLF&gt;&lt;POPSTACK&gt;&lt;CALL&gt; 599 05 NY&lt;CRLF&gt;</span>
        — without <span class="mono">&lt;POPSTACK&gt;</span> ESM sends TU, logs, then F2 to the next caller.
      </span>
    </div>
    <div class="macro-rows">
      {#each macroState.macros as m, i (m.key)}
        <div class="macro-row">
          <span class="macro-key">{keyLabel(m.key)}</span>
          <input
            class="macro-label"
            type="text"
            value={m.label}
            oninput={(e) =>
              macroState.setLabel(i, (e.target as HTMLInputElement).value)}
            placeholder="label"
            maxlength="10"
          />
          <input
            class="macro-text mono"
            type="text"
            value={m.text}
            oninput={(e) =>
              macroState.setText(i, (e.target as HTMLInputElement).value)}
            placeholder="message template"
            spellcheck="false"
          />
          <button
            class="ghost macro-reset"
            onclick={() => macroState.resetOne(i)}
            title="Restore default for this slot"
          >
            ↺
          </button>
        </div>
      {/each}
    </div>
    <div class="macro-save">
      <button class="cluster-btn" onclick={saveMacros}>Save F-keys</button>
      {#if macroSaveMsg}
        <span class="saved">{macroSaveMsg}</span>
      {:else}
        <span class="hint">Saved as you type{activeSetup ? `, and into the setup “${activeSetup.name}”` : ""} — this confirms it.</span>
      {/if}
    </div>
  </div>
</section>

<style>
  .panel {
    background: transparent;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(120px, 1fr));
    gap: 10px 14px;
    align-items: end;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .field.small {
    max-width: 110px;
  }
  .field.contest {
    grid-column: span 3;
  }
  label {
    color: #8a949d;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  input,
  select {
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 6px 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 13px;
  }
  input:focus,
  select:focus {
    outline: none;
    border-color: #4a90e2;
  }
  .hint {
    color: #6b7176;
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    margin-top: 2px;
  }
  .esm-field { grid-column: span 3; }
  .esm-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    color: #c5d1de;
    font-size: 12px;
    cursor: pointer;
    text-transform: none;
    letter-spacing: 0;
  }
  .scp {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid #2a2f33;
  }
  .scp-info {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 6px;
  }
  .scp-label {
    color: #8a949d;
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }
  .scp-count {
    color: #c5d1de;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 13px;
    font-weight: 600;
  }
  .dim {
    color: #5a636c;
  }
  .scp-source {
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 400px;
  }
  .scp-actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .scp-actions button {
    background: #2a3f5f;
    border: 1px solid #3a5a8a;
    color: #e6e6e6;
    padding: 5px 12px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
  }
  .scp-actions button:hover:not(:disabled) { background: #345080; }
  .scp-actions button:disabled { opacity: 0.5; cursor: not-allowed; }
  .scp-actions button.ghost {
    background: transparent;
    border-color: #3a4452;
    color: #8a949d;
  }
  .scp-actions button.ghost:hover:not(:disabled) {
    background: #1c2024;
    color: #c5d1de;
  }
  .mono { font-family: ui-monospace, SFMono-Regular, Menlo, monospace; }
  .scp-error {
    margin-top: 6px;
    color: #f87171;
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .cluster {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid #2a2f33;
  }
  .cluster-info {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 8px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }
  .cluster-state.state-connected { color: #4ade80; }
  .cluster-state.state-connecting { color: #fbbf24; }
  .cluster-state.state-error { color: #f87171; }
  .cluster-state.state-disconnected { color: #6b7176; }
  .cluster-row {
    display: flex;
    align-items: flex-end;
    gap: 10px;
  }
  .cluster-row .field {
    flex: 1;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }
  .cluster-row .field.small {
    flex: 0 0 90px;
  }
  .cluster-btn {
    background: #2a3f5f;
    border: 1px solid #3a5a8a;
    color: #e6e6e6;
    padding: 6px 14px;
    border-radius: 4px;
    cursor: pointer;
    font-size: 12px;
    align-self: flex-end;
  }
  .cluster-btn:hover { background: #345080; }
  .cluster-btn:disabled { opacity: 0.5; cursor: not-allowed; }
  .login-cmds { margin-top: 10px; }
  textarea {
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 6px 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    resize: vertical;
  }
  textarea:focus { outline: none; border-color: #4a90e2; }
  .setup-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .setup-row select { min-width: 180px; }
  .setup-row input { flex: 1; }
  .setup-row .del {
    background: transparent;
    border: 1px solid #3a4452;
    color: #8a949d;
    border-radius: 4px;
    padding: 5px 9px;
    cursor: pointer;
  }
  .setup-row .del:hover { color: #f87171; border-color: #f87171; }

  .bell { margin-top: 8px; }
  .font-rows { display: flex; flex-direction: column; gap: 6px; }
  .font-row {
    display: grid;
    grid-template-columns: 90px minmax(140px, 220px) 64px auto auto 1fr;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }
  .font-win { color: #c5d1de; }
  .font-row select.font-family,
  .font-row input.font-size {
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 3px 6px;
    font-size: 12px;
  }
  .font-zero { display: flex; align-items: center; gap: 4px; color: #8a949d; white-space: nowrap; }
  .font-sample {
    font-family: var(--win-font, ui-monospace, SFMono-Regular, Menlo, monospace);
    font-size: var(--win-size, 13px);
    font-variant-numeric: var(--win-zero, normal);
    color: #fbbf24;
    white-space: nowrap;
    overflow: hidden;
  }

  .macros {
    margin-top: 14px;
    padding-top: 12px;
    border-top: 1px solid #2a2f33;
  }
  .macros-head {
    display: flex;
    align-items: baseline;
    gap: 12px;
    margin-bottom: 8px;
  }
  .macros-head .hint {
    flex: 1;
  }
  .macro-save { display: flex; align-items: center; gap: 10px; margin-top: 8px; }
  .macro-save .saved { color: #4ade80; font-size: 12px; }
  .stack-key { display: flex; align-items: center; gap: 6px; white-space: nowrap; color: #8a949d; font-size: 12px; }
  .macros-head .mono {
    color: #c5d1de;
    margin-right: 6px;
  }
  .macro-rows {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .macro-row {
    display: grid;
    grid-template-columns: 36px 110px 1fr 28px;
    gap: 8px;
    align-items: center;
  }
  .macro-key {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    color: #8a949d;
    font-size: 11px;
    text-align: right;
  }
  .macro-label,
  .macro-text {
    background: #0c0e10;
    border: 1px solid #2a2f33;
    border-radius: 3px;
    color: #e6e6e6;
    padding: 4px 8px;
    font-size: 12px;
  }
  .macro-label {
    font-size: 12px;
  }
  .macro-text {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
  .macro-label:focus,
  .macro-text:focus {
    outline: none;
    border-color: #4a90e2;
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
  button.ghost:hover:not(:disabled) {
    background: #1c2024;
    color: #c5d1de;
  }
  .macro-reset {
    font-size: 14px;
    padding: 2px 6px;
    line-height: 1;
  }
</style>
