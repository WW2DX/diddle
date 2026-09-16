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
  } from "$lib/tci";

  // ------------------------------------------------------------------
  // Audio-device input
  // ------------------------------------------------------------------

  const DEV_KEY = "diddle.audioInputDevice";
  let devices = $state<AudioDevice[]>([]);
  let device = $state<string>("");
  let audioIn = $state<AudioInStatus>({ kind: "idle" });
  let audioError = $state<string | null>(null);
  let audioRunning = $derived(audioIn.kind === "running");
  let peakPct = $derived(
    audioIn.kind === "running" ? Math.min(100, Math.round(audioIn.peak * 100)) : 0,
  );

  async function refreshDevices() {
    try {
      devices = await audioDevices();
      if (!device || !devices.some((d) => d.name === device)) {
        device = devices.find((d) => d.is_default)?.name ?? devices[0]?.name ?? "";
      }
    } catch (e: any) {
      audioError = String(e);
    }
  }

  async function startAudio() {
    audioError = null;
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
    try {
      device = localStorage.getItem(DEV_KEY) || "";
    } catch {}
    try {
      audioIn = await audioInputStatus();
    } catch {}
    try {
      wav = await wavStatus();
    } catch {}
    unlisten.push(await onAudioInStatus((s) => (audioIn = s)));
    unlisten.push(await onWavStatus((s) => (wav = s)));
    refreshDevices();
  });

  onDestroy(() => {
    for (const u of unlisten) u();
  });
</script>

<div class="test">
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
      <select bind:value={device} disabled={audioRunning}>
        {#each devices as d}
          <option value={d.name}>{d.name}{d.is_default ? " (default)" : ""}</option>
        {/each}
        {#if devices.length === 0}
          <option value="">no input devices found</option>
        {/if}
      </select>
      <button class="ghost" onclick={refreshDevices} disabled={audioRunning} title="Rescan devices">↻</button>
      {#if audioRunning}
        <button class="stop" onclick={stopAudio}>Stop</button>
      {:else}
        <button class="primary" onclick={startAudio} disabled={devices.length === 0}>Start</button>
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
