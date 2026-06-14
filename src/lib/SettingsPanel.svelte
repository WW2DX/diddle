<script lang="ts">
  import { onMount } from "svelte";
  import { open as openDialog } from "@tauri-apps/plugin-dialog";
  import { settings } from "$lib/settings.svelte";
  import { CONTESTS, activeContest } from "$lib/contests";
  import {
    scpAutoDownload,
    scpLoadFile,
    scpStatus,
    type ScpStatus,
  } from "$lib/tci";
  import { cluster } from "$lib/cluster.svelte";
  import { macroState } from "$lib/macros.svelte";

  // Show the format hint based on the current selection.
  let formatHint = $derived(activeContest().exchangeFormat);

  let scp = $state<ScpStatus>({ count: 0, source: "" });
  let scpLoading = $state(false);
  let scpError = $state<string | null>(null);

  onMount(async () => {
    try {
      scp = await scpStatus();
    } catch (e) {
      console.error("scpStatus failed", e);
    }
    // Auto-reload the saved SCP file on startup so the user doesn't have
    // to re-pick it every session.
    if (settings.scpPath && scp.source === "starter") {
      await loadScpFromPath(settings.scpPath);
    } else if (!settings.scpPath && scp.source === "starter") {
      // First launch with no SCP picked yet — pull MASTER.SCP from
      // supercheckpartial.com so the operator has a full callsign database
      // without having to find and download it by hand.
      await autoDownloadScp();
    }
  });

  async function autoDownloadScp() {
    scpLoading = true;
    scpError = null;
    try {
      const result = await scpAutoDownload();
      scp = result.status;
      settings.setScpPath(result.path);
    } catch (e: any) {
      scpError = `Auto-download failed (using starter list): ${e}`;
    } finally {
      scpLoading = false;
    }
  }

  async function loadScpFromPath(path: string) {
    scpLoading = true;
    scpError = null;
    try {
      scp = await scpLoadFile(path);
      settings.setScpPath(path);
    } catch (e: any) {
      scpError = String(e);
    } finally {
      scpLoading = false;
    }
  }

  async function pickScpFile() {
    const path = await openDialog({
      title: "Choose MASTER.SCP file",
      multiple: false,
      filters: [{ name: "SCP / text", extensions: ["scp", "txt"] }],
    });
    if (!path || typeof path !== "string") return;
    await loadScpFromPath(path);
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
        Run-mode Enter cycles: empty→CQ, call→Excg, both→TU+Log
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
      <button onclick={autoDownloadScp} disabled={scpLoading}>
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
  </div>

  <div class="macros">
    <div class="macros-head">
      <span class="scp-label">F-key macros</span>
      <span class="hint">
        <span class="mono">&lt;MYCALL&gt;</span>
        <span class="mono">&lt;CALL&gt;</span>
        <span class="mono">&lt;SERIAL&gt;</span> are substituted at send time.
      </span>
      <button class="ghost macro-reset-all" onclick={() => macroState.resetAll()}>
        Reset all
      </button>
    </div>
    <div class="macro-rows">
      {#each macroState.macros as m, i (m.key)}
        <div class="macro-row">
          <span class="macro-key">{m.key}</span>
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
