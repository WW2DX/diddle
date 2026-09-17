<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    playWav,
    stopWav,
    wavStatus,
    onWavStatus,
    type WavStatus,
    audioDevices,
    audioInputStart,
    audioInputStop,
    audioInputStatus,
    onAudioInStatus,
    type AudioDevice,
    type AudioInStatus,
    simStart,
    simStop,
    simStatus,
    simUpdate,
    onSimStatus,
    onSimLog,
    type SimConfig,
    type SimExchange,
    type SimMode,
    type SimStatus,
    type SimLogLine,
  } from "$lib/tci";
  import { settings } from "$lib/settings.svelte";
  import { activeContest } from "$lib/contests";

  // ------------------------------------------------------------------
  // Contest simulator
  // ------------------------------------------------------------------

  const SIM_KEY = "diddle.sim";

  const BANDS: { label: string; dial: number }[] = [
    { label: "80 m", dial: 3_580_000 },
    { label: "40 m", dial: 7_080_000 },
    { label: "20 m", dial: 14_080_000 },
    { label: "15 m", dial: 21_080_000 },
    { label: "10 m", dial: 28_080_000 },
  ];

  let simMode = $state<SimMode>("pileup");
  let activity = $state(2);
  let noise = $state(0.25);
  let background = $state(2);
  let spreadHz = $state(15);
  let signal = $state(0.6);
  let dialHz = $state(14_080_000);
  let useScp = $state(true);

  let sim = $state<SimStatus | null>(null);
  let simError = $state<string | null>(null);
  let simLog = $state<SimLogLine[]>([]);
  let logEl = $state<HTMLDivElement | null>(null);
  let simRunning = $derived(sim?.running ?? false);

  // Map the active contest profile onto what the simulated stations send.
  function exchangeForContest(id: string): SimExchange {
    switch (id) {
      case "cqww-rtty":
        return "zone";
      case "rtty-roundup":
        return "state";
      case "naqp-rtty":
        return "name-state";
      case "qso":
        return "ragchew";
      default:
        return "serial";
    }
  }

  function loadSimPrefs() {
    try {
      const raw = localStorage.getItem(SIM_KEY);
      if (!raw) return;
      const o = JSON.parse(raw);
      if (o.mode === "pileup" || o.mode === "playback") simMode = o.mode;
      if (typeof o.activity === "number") activity = o.activity;
      if (typeof o.noise === "number") noise = o.noise;
      if (typeof o.background === "number") background = o.background;
      if (typeof o.spreadHz === "number") spreadHz = o.spreadHz;
      if (typeof o.signal === "number") signal = o.signal;
      if (typeof o.dialHz === "number") dialHz = o.dialHz;
      if (typeof o.useScp === "boolean") useScp = o.useScp;
    } catch {}
  }

  function saveSimPrefs() {
    try {
      localStorage.setItem(
        SIM_KEY,
        JSON.stringify({ mode: simMode, activity, noise, background, spreadHz, signal, dialHz, useScp }),
      );
    } catch {}
  }

  function simConfig(): SimConfig {
    return {
      my_call: settings.myCall || "N0CALL",
      exchange: exchangeForContest(settings.activeContest),
      mode: simMode,
      activity,
      noise,
      background,
      spread_hz: spreadHz,
      signal,
      dial_hz: dialHz,
      use_scp: useScp,
    };
  }

  async function startSim() {
    simError = null;
    simLog = [];
    saveSimPrefs();
    try {
      await simStart(simConfig());
    } catch (e: any) {
      simError = String(e);
    }
  }

  async function stopSim() {
    simError = null;
    try {
      await simStop();
    } catch (e: any) {
      simError = String(e);
    }
  }

  // Sliders apply live while the simulator runs (debounced a little so a
  // drag doesn't flood the backend).
  let liveTimer: ReturnType<typeof setTimeout> | null = null;
  function liveUpdate() {
    saveSimPrefs();
    if (!simRunning) return;
    if (liveTimer) clearTimeout(liveTimer);
    liveTimer = setTimeout(() => {
      simUpdate({ noise, activity, background, spreadHz, signal }).catch((e) => {
        simError = String(e);
      });
    }, 120);
  }

  function fmtOffset(hz: number): string {
    const s = hz >= 0 ? "+" : "−";
    return `${s}${Math.abs(hz).toFixed(0)} Hz`;
  }

  function fmtTime(ms: number): string {
    const d = new Date(ms);
    return d.toISOString().slice(11, 19);
  }

  function exchangeLabel(k: SimExchange): string {
    switch (k) {
      case "serial":
        return "RST + serial";
      case "zone":
        return "RST + zone (US/VE + state)";
      case "state":
        return "RST + state (DX: serial)";
      case "name-state":
        return "name + state";
      case "ragchew":
        return "RST + name + QTH";
    }
  }

  let phaseText = $derived.by(() => {
    if (!sim || !sim.running) return "stopped";
    if (sim.ptt) return "you are transmitting";
    switch (sim.phase) {
      case "idle":
        return "quiet — send CQ";
      case "calling":
        return `${sim.callers.length} calling you`;
      case "exchanged":
        return `${sim.worked?.call ?? "?"} sent exchange — log it and send TU`;
      case "playback":
        return "scripted run playing";
    }
  });

  // ------------------------------------------------------------------
  // Audio-device input
  // ------------------------------------------------------------------

  const DEV_KEY = "diddle.audioInputDevice";
  let devices = $state<AudioDevice[]>([]);
  // Devices are listed only once you engage this section: enumerating
  // them is what makes macOS ask for microphone permission, and someone
  // who only wants the simulator shouldn't be asked.
  let scanned = $state(false);
  let device = $state<string>("");
  let audioIn = $state<AudioInStatus>({ kind: "idle" });
  let audioError = $state<string | null>(null);
  let audioRunning = $derived(audioIn.kind === "running");
  let peakPct = $derived(
    audioIn.kind === "running" ? Math.min(100, Math.round(audioIn.peak * 100)) : 0,
  );

  async function refreshDevices() {
    audioError = null;
    try {
      devices = await audioDevices();
      scanned = true;
      if (!device || !devices.some((d) => d.name === device)) {
        device = devices.find((d) => d.is_default)?.name ?? devices[0]?.name ?? "";
      }
    } catch (e: any) {
      audioError = String(e);
    }
  }

  async function startAudio() {
    audioError = null;
    if (!scanned) {
      await refreshDevices();
      if (audioError) return;
    }
    try {
      localStorage.setItem(DEV_KEY, device);
    } catch {}
    try {
      await audioInputStart(device || null);
    } catch (e: any) {
      audioError = String(e);
    }
  }

  async function stopAudio() {
    audioError = null;
    try {
      await audioInputStop();
    } catch (e: any) {
      audioError = String(e);
    }
  }

  // ------------------------------------------------------------------
  // WAV file
  // ------------------------------------------------------------------

  let wav = $state<WavStatus>({ kind: "idle" });
  let wavError = $state<string | null>(null);
  let wavPlaying = $derived(wav.kind === "playing");

  async function pickAndPlay() {
    wavError = null;
    const path = await open({
      title: "Load WAV with RTTY signal",
      multiple: false,
      filters: [{ name: "WAV audio", extensions: ["wav"] }],
    });
    if (!path || typeof path !== "string") return;
    try {
      await playWav(path);
    } catch (e: any) {
      wavError = String(e);
    }
  }

  async function stopWavFile() {
    try {
      await stopWav();
    } catch (e: any) {
      wavError = String(e);
    }
  }

  function baseName(p: string): string {
    const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
    return i >= 0 ? p.slice(i + 1) : p;
  }

  // ------------------------------------------------------------------

  const unlisten: Array<() => void> = [];

  onMount(async () => {
    loadSimPrefs();
    try {
      device = localStorage.getItem(DEV_KEY) || "";
    } catch {}
    try {
      sim = await simStatus();
      simLog = sim.log;
    } catch {}
    try {
      audioIn = await audioInputStatus();
    } catch {}
    try {
      wav = await wavStatus();
    } catch {}
    unlisten.push(await onSimStatus((s) => (sim = s)));
    unlisten.push(
      await onSimLog((l) => {
        simLog = [...simLog.slice(-79), l];
        queueMicrotask(() => {
          if (logEl) logEl.scrollTop = logEl.scrollHeight;
        });
      }),
    );
    unlisten.push(await onAudioInStatus((s) => (audioIn = s)));
    unlisten.push(await onWavStatus((s) => (wav = s)));
  });

  onDestroy(() => {
    for (const u of unlisten) u();
    if (liveTimer) clearTimeout(liveTimer);
  });
</script>

<div class="test">
  <!-- ============ Simulator ============ -->
  <section class="block">
    <header>
      <h3>Contest simulator</h3>
      <span class="dim">
        a synthetic band through the real decoder — no radio, no on-air activity needed
      </span>
    </header>

    <div class="grid">
      <div class="col">
        <label class="row">
          <span class="lbl">Mode</span>
          <span class="seg">
            <button
              class:on={simMode === "pileup"}
              disabled={simRunning}
              onclick={() => ((simMode = "pileup"), saveSimPrefs())}
              title="Stations answer your CQ and react to what you send"
            >Pileup</button>
            <button
              class:on={simMode === "playback"}
              disabled={simRunning}
              onclick={() => ((simMode = "playback"), saveSimPrefs())}
              title="Both sides of a run play back-to-back (RTTY Runner style); listen and log only — F-keys don't transmit"
            >Playback</button>
          </span>
        </label>

        <label class="row">
          <span class="lbl">Band</span>
          <select bind:value={dialHz} disabled={simRunning} onchange={saveSimPrefs}>
            {#each BANDS as b}
              <option value={b.dial}>{b.label}</option>
            {/each}
          </select>
          <span class="dim">pretend dial, so QSOs log with a band</span>
        </label>

        <label class="row">
          <span class="lbl">Calls</span>
          <input type="checkbox" bind:checked={useScp} disabled={simRunning} onchange={saveSimPrefs} />
          <span class="dim">real calls from Super Check Partial (spots + bandmap work)</span>
        </label>

        <div class="row">
          <span class="lbl">You</span>
          <span class="num">{settings.myCall || "N0CALL"}</span>
          <span class="dim">· {activeContest().name} → {exchangeLabel(exchangeForContest(settings.activeContest))}</span>
        </div>
      </div>

      <div class="col">
        <label class="row">
          <span class="lbl">Activity</span>
          <input type="range" min="1" max="9" step="1" bind:value={activity} oninput={liveUpdate} />
          <span class="num">{activity}</span>
          <span class="dim">callers per CQ</span>
        </label>
        <label class="row">
          <span class="lbl">Noise</span>
          <input type="range" min="0" max="1" step="0.05" bind:value={noise} oninput={liveUpdate} />
          <span class="num">{Math.round(noise * 100)}%</span>
        </label>
        <label class="row">
          <span class="lbl">Signal</span>
          <input type="range" min="0.1" max="1" step="0.05" bind:value={signal} oninput={liveUpdate} />
          <span class="num">{Math.round(signal * 100)}%</span>
        </label>
        <label class="row">
          <span class="lbl">Spread</span>
          <input type="range" min="0" max="60" step="5" bind:value={spreadHz} oninput={liveUpdate} />
          <span class="num">±{spreadHz} Hz</span>
          <span class="dim">callers off your tones</span>
        </label>
        <label class="row">
          <span class="lbl">Others</span>
          <input type="range" min="0" max="8" step="1" bind:value={background} oninput={liveUpdate} />
          <span class="num">{background}</span>
          <span class="dim">stations elsewhere in the passband</span>
        </label>
      </div>
    </div>

    <div class="row actions">
      {#if simRunning}
        <button class="stop" onclick={stopSim}>Stop simulator</button>
      {:else}
        <button class="primary" onclick={startSim}>Start simulator</button>
      {/if}
      <span class="phase" class:tx={sim?.ptt}>{phaseText}</span>
      {#if sim?.running}
        <span class="dim">QSOs: <span class="num">{sim.qso_count}</span></span>
      {/if}
    </div>
    {#if simError}
      <div class="err">{simError}</div>
    {/if}

    {#if sim?.running}
      <div class="truth">
        <div class="truth-col">
          <div class="truth-head">On your frequency <span class="dim">(what they really sent)</span></div>
          {#if sim.worked}
            <div class="st worked">
              <span class="call">{sim.worked.call}</span>
              <span class="ex">{sim.worked.exchange}</span>
              <span class="dim">{fmtOffset(sim.worked.offset_hz)}</span>
              <span class="tag">exchanged</span>
            </div>
          {/if}
          {#each sim.callers as c}
            <div class="st">
              <span class="call">{c.call}</span>
              <span class="ex">{c.exchange}</span>
              <span class="dim">{fmtOffset(c.offset_hz)}</span>
              <span class="tag">{c.status}</span>
            </div>
          {/each}
          {#if !sim.worked && sim.callers.length === 0}
            <div class="dim">nobody yet</div>
          {/if}
        </div>
        <div class="truth-col">
          <div class="truth-head">Elsewhere in the passband</div>
          {#each sim.background as b}
            <div class="st">
              <span class="call">{b.call}</span>
              <span class="ex">{b.exchange}</span>
              <span class="dim">mark {b.mark_hz.toFixed(0)} Hz</span>
            </div>
          {/each}
          {#if sim.background.length === 0}
            <div class="dim">none</div>
          {/if}
        </div>
      </div>

      <div class="log" bind:this={logEl}>
        {#each simLog as l}
          <div class="line {l.who}">
            <span class="t">{fmtTime(l.t_ms)}</span>
            <span class="who">{l.who === "you" ? "YOU" : l.who === "dx" ? "DX" : l.who === "bg" ? "BG" : "SIM"}</span>
            <span class="txt">{l.text}</span>
          </div>
        {/each}
      </div>
    {:else}
      <div class="hint dim">
        Pileup: press your CQ key (F1). Stations answer; type one's call and send the exchange,
        then TU — they behave like a real pileup (repeats on AGN?, corrections on a wrong call,
        walking away if ignored). Playback: listen and log only — F-keys and Enter don't transmit.
        Your F-keys never key the radio while the simulator runs.
      </div>
    {/if}
  </section>

  <!-- ============ Audio input ============ -->
  <section class="block">
    <header>
      <h3>Audio input</h3>
      <span class="dim">
        decode any input device — e.g. an external simulator routed through a virtual cable
        (BlackHole on macOS, VB-CABLE on Windows)
      </span>
    </header>
    <div class="row">
      {#if scanned}
        <select bind:value={device} disabled={audioRunning}>
          {#each devices as d}
            <option value={d.name}>{d.name}{d.is_default ? " (default)" : ""}</option>
          {/each}
          {#if devices.length === 0}
            <option value="">no input devices found</option>
          {/if}
        </select>
        <button class="ghost" onclick={refreshDevices} disabled={audioRunning} title="Rescan devices">↻</button>
      {:else}
        <button class="ghost" onclick={refreshDevices} title="List input devices (macOS asks for microphone access the first time)">
          Scan input devices…
        </button>
      {/if}
      {#if audioRunning}
        <button class="stop" onclick={stopAudio}>Stop</button>
      {:else}
        <button class="primary" onclick={startAudio} disabled={scanned && devices.length === 0}>Start</button>
      {/if}
      {#if audioIn.kind === "running"}
        <div class="meter" title="input level">
          <div class="fill" class:hot={peakPct > 95} style="width: {peakPct}%"></div>
        </div>
        <span class="dim">
          {(audioIn.sample_rate / 1000).toFixed(1)} kHz · {audioIn.channels}ch
        </span>
      {:else if audioIn.kind === "error"}
        <span class="err">{audioIn.message}</span>
      {/if}
    </div>
    {#if audioError}
      <div class="err">{audioError}</div>
    {/if}
  </section>

  <!-- ============ WAV ============ -->
  <section class="block">
    <header>
      <h3>WAV file</h3>
      <span class="dim">play a recording through the decoder</span>
    </header>
    <div class="row">
      <button class="primary" onclick={pickAndPlay} disabled={wavPlaying}>Load WAV…</button>
      {#if wavPlaying}
        <button class="stop" onclick={stopWavFile}>Stop</button>
      {/if}
      {#if wav.kind === "playing"}
        <div class="info">
          <div class="file">{baseName(wav.path)}</div>
          <div class="meta">
            <span class="num">{wav.position_s.toFixed(1)}</span>
            <span class="dim">/</span>
            <span class="num">{wav.duration_s.toFixed(1)} s</span>
            <span class="dim">·</span>
            <span class="num">{(wav.sample_rate / 1000).toFixed(1)} kHz</span>
            <span class="dim">·</span>
            <span class="num">{wav.channels}ch</span>
          </div>
          <div class="progress">
            <div
              class="bar"
              style="width: {Math.min(100, (wav.position_s / Math.max(1, wav.duration_s)) * 100)}%"
            ></div>
          </div>
        </div>
      {:else if wav.kind === "done"}
        <div class="info">
          <div class="file">{baseName(wav.path)}</div>
          <div class="meta dim">finished</div>
        </div>
      {/if}
    </div>
    {#if wavError}
      <div class="err">{wavError}</div>
    {/if}
  </section>
</div>

<style>
  .test {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .block header {
    display: flex;
    align-items: baseline;
    gap: 10px;
    margin-bottom: 8px;
    flex-wrap: wrap;
  }

  h3 {
    margin: 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: #8a949d;
    font-weight: 600;
  }

  .dim {
    color: #6b7176;
    font-size: 11px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .num {
    color: #c5d1de;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }

  .grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px 24px;
  }
  @media (max-width: 900px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }

  .col {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .row.actions {
    margin-top: 8px;
  }

  .lbl {
    width: 64px;
    color: #8a949d;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  input[type="range"] {
    width: 140px;
  }

  select {
    background: #0f1214;
    color: #e6e6e6;
    border: 1px solid #2a3036;
    border-radius: 4px;
    padding: 4px 6px;
    font-size: 12px;
    max-width: 320px;
  }

  button {
    padding: 6px 14px;
    border-radius: 4px;
    border: 1px solid;
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
    color: #e6e6e6;
  }
  button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }
  button.primary {
    background: #2a3f5f;
    border-color: #3a5a8a;
  }
  button.primary:hover:not(:disabled) {
    background: #345080;
  }
  button.stop {
    background: #5a2a2a;
    border-color: #8a3a3a;
  }
  button.stop:hover {
    background: #703535;
  }
  button.ghost {
    background: transparent;
    border-color: #2a3036;
    padding: 4px 8px;
  }

  .seg {
    display: inline-flex;
    border: 1px solid #2a3036;
    border-radius: 4px;
    overflow: hidden;
  }
  .seg button {
    border: 0;
    border-radius: 0;
    background: #0f1214;
    color: #8a949d;
    padding: 4px 12px;
  }
  .seg button.on {
    background: #2a3f5f;
    color: #e6e6e6;
  }

  .phase {
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    color: #9fd3a0;
  }
  .phase.tx {
    color: #f0a0a0;
  }

  .truth {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px 24px;
    margin-top: 10px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }
  .truth-head {
    color: #8a949d;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.5px;
    margin-bottom: 4px;
  }
  .st {
    display: flex;
    gap: 10px;
    align-items: baseline;
    padding: 2px 0;
  }
  .st.worked .call {
    color: #9fd3a0;
  }
  .call {
    color: #e6e6e6;
    font-weight: 600;
    min-width: 72px;
  }
  .ex {
    color: #c5d1de;
  }
  .tag {
    color: #6b7176;
    font-size: 10px;
    text-transform: uppercase;
  }

  .log {
    margin-top: 10px;
    max-height: 160px;
    overflow-y: auto;
    background: #0f1214;
    border: 1px solid #262b30;
    border-radius: 4px;
    padding: 6px 8px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
  }
  .line {
    display: flex;
    gap: 8px;
    white-space: pre-wrap;
  }
  .line .t {
    color: #4d5459;
  }
  .line .who {
    width: 28px;
    color: #6b7176;
  }
  .line.you .who,
  .line.you .txt {
    color: #f0c674;
  }
  .line.dx .txt {
    color: #c5d1de;
  }
  .line.bg .txt {
    color: #7d8790;
  }
  .line.sim .txt {
    color: #9fd3a0;
  }

  .hint {
    margin-top: 8px;
    line-height: 1.5;
  }

  .meter {
    width: 120px;
    height: 8px;
    background: #1f2429;
    border-radius: 2px;
    overflow: hidden;
  }
  .meter .fill {
    height: 100%;
    background: #4a90e2;
    transition: width 80ms linear;
  }
  .meter .fill.hot {
    background: #e25c5c;
  }

  .info {
    flex: 1;
    min-width: 200px;
  }
  .file {
    color: #c5d1de;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 13px;
    margin-bottom: 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .meta {
    display: flex;
    align-items: baseline;
    gap: 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }
  .progress {
    height: 4px;
    background: #1f2429;
    border-radius: 2px;
    margin-top: 6px;
    overflow: hidden;
  }
  .bar {
    height: 100%;
    background: #4a90e2;
    transition: width 200ms linear;
  }

  .err {
    margin-top: 6px;
    color: #f87171;
    font-size: 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
</style>
