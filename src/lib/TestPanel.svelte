<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { open } from "@tauri-apps/plugin-dialog";
  import {
    playWav,
    stopWav,
    wavStatus,
    onWavStatus,
    type WavStatus,
  } from "$lib/tci";

  let status = $state<WavStatus>({ kind: "idle" });
  let error = $state<string | null>(null);
  let unlisten: (() => void) | null = null;

  onMount(async () => {
    try {
      status = await wavStatus();
    } catch {}
    unlisten = await onWavStatus((s) => (status = s));
  });

  onDestroy(() => unlisten?.());

  async function pickAndPlay() {
    error = null;
    const path = await open({
      title: "Load WAV with RTTY signal",
      multiple: false,
      filters: [{ name: "WAV audio", extensions: ["wav"] }],
    });
    if (!path || typeof path !== "string") return;
    try {
      await playWav(path);
    } catch (e: any) {
      error = String(e);
    }
  }

  async function stop() {
    try {
      await stopWav();
    } catch (e: any) {
      error = String(e);
    }
  }

  function baseName(p: string): string {
    const i = Math.max(p.lastIndexOf("/"), p.lastIndexOf("\\"));
    return i >= 0 ? p.slice(i + 1) : p;
  }

  let playing = $derived(status.kind === "playing");
</script>

<section class="panel">
  <header>
    <h2>Test input</h2>
    <span class="dim">play a WAV through the decoder</span>
  </header>

  <div class="row">
    <button class="primary" onclick={pickAndPlay} disabled={playing}>
      Load WAV…
    </button>
    {#if playing}
      <button class="stop" onclick={stop}>Stop</button>
    {/if}

    {#if status.kind === "playing"}
      <div class="info">
        <div class="file">{baseName(status.path)}</div>
        <div class="meta">
          <span class="num">{status.position_s.toFixed(1)}</span>
          <span class="dim">/</span>
          <span class="num">{status.duration_s.toFixed(1)} s</span>
          <span class="dim">·</span>
          <span class="num">{(status.sample_rate / 1000).toFixed(1)} kHz</span>
          <span class="dim">·</span>
          <span class="num">{status.channels}ch</span>
        </div>
        <div class="progress">
          <div
            class="bar"
            style="width: {Math.min(
              100,
              (status.position_s / Math.max(1, status.duration_s)) * 100,
            )}%"
          ></div>
        </div>
      </div>
    {:else if status.kind === "done"}
      <div class="info">
        <div class="file">{baseName(status.path)}</div>
        <div class="meta dim">finished</div>
      </div>
    {:else}
      <div class="info">
        <div class="meta dim">
          no file loaded — output goes to the waterfall + RX decoder
        </div>
      </div>
    {/if}
  </div>

  {#if error}
    <div class="err">{error}</div>
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
    gap: 10px;
    margin-bottom: 10px;
  }

  h2 {
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

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
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
  button.primary {
    background: #2a3f5f;
    border-color: #3a5a8a;
  }
  button.primary:hover:not(:disabled) { background: #345080; }
  button.primary:disabled { opacity: 0.4; cursor: not-allowed; }
  button.stop {
    background: #5a2a2a;
    border-color: #8a3a3a;
  }
  button.stop:hover { background: #703535; }

  .info {
    flex: 1;
    min-width: 0;
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
  .num { color: #c5d1de; }

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
    margin-top: 8px;
    color: #f87171;
    font-size: 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
</style>
