<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import {
    DEFAULT_TCI_URL,
    connect,
    disconnect,
    onState,
    onRig,
    status,
    type RigState,
    type TciState,
  } from "$lib/tci";
  import { fmtMhz, bandFromHz } from "$lib/bands";
  import { qsoLog } from "$lib/qsoLog.svelte";

  let { rig = $bindable() }: { rig: RigState } = $props();

  let url = $state(DEFAULT_TCI_URL);
  let tci = $state<TciState>({ kind: "disconnected" });
  let error = $state<string | null>(null);
  let now = $state(Date.now());
  let sessionStart = $state<number | null>(null);

  const unlisten: Array<() => void> = [];
  let tick: ReturnType<typeof setInterval>;

  onMount(async () => {
    try {
      const [s, r] = await status();
      tci = s;
      rig = r;
    } catch {}
    unlisten.push(await onState((s) => (tci = s)));
    unlisten.push(await onRig((r) => (rig = r)));
    tick = setInterval(() => (now = Date.now()), 1000);
  });

  onDestroy(() => {
    for (const u of unlisten) u();
    clearInterval(tick);
  });

  async function toggle() {
    error = null;
    try {
      if (tci.kind === "connected" || tci.kind === "connecting") {
        await disconnect();
      } else {
        if (!sessionStart) sessionStart = Date.now();
        await connect(url);
      }
    } catch (e: any) {
      error = String(e);
    }
  }

  let connected = $derived(tci.kind === "connected");
  let statusDot = $derived.by(() => {
    switch (tci.kind) {
      case "connected":
        return tci.ready ? "ok" : "warn";
      case "connecting":
        return "warn";
      case "error":
        return "err";
      default:
        return "off";
    }
  });

  let elapsed = $derived.by(() => {
    if (!sessionStart) return "00:00:00";
    const secs = Math.floor((now - sessionStart) / 1000);
    const h = Math.floor(secs / 3600);
    const m = Math.floor((secs % 3600) / 60);
    const s = secs % 60;
    return `${h.toString().padStart(2, "0")}:${m
      .toString()
      .padStart(2, "0")}:${s.toString().padStart(2, "0")}`;
  });
</script>

<header class="bar">
  <div class="brand">
    <span class="name">Diddle</span>
    <span class="tag">RTTY</span>
  </div>

  <div class="tci">
    <span class="dot {statusDot}" title={tci.kind}>●</span>
    <input
      type="text"
      bind:value={url}
      disabled={connected || tci.kind === "connecting"}
      spellcheck="false"
    />
    {#if connected || tci.kind === "connecting"}
      <button class="conn-btn disconnect" onclick={toggle}>Disconnect</button>
    {:else}
      <button class="conn-btn connect" onclick={toggle}>Connect</button>
    {/if}
  </div>

  <div class="rig">
    <span class="freq">{fmtMhz(rig.freq)}</span>
    <span class="band">{bandFromHz(rig.freq)}</span>
    <span class="mode">{(rig.mode || "—").toUpperCase()}</span>
    <span class="ptt" class:on={rig.ptt}>{rig.ptt ? "TX" : "RX"}</span>
  </div>

  <div class="score">
    <div class="stat">
      <span class="lbl">Qs</span>
      <span class="num">{qsoLog.qsos.length}</span>
    </div>
    <div class="stat">
      <span class="lbl">Rate</span>
      <span class="num">{qsoLog.ratePerHour}</span><span class="lbl">/hr</span>
    </div>
    <div class="stat">
      <span class="lbl">Time</span>
      <span class="num">{elapsed}</span>
    </div>
  </div>
</header>

{#if error}
  <div class="err-banner">{error}</div>
{/if}

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 8px 16px;
    background: #0e1113;
    border-bottom: 1px solid #262b30;
    font-size: 13px;
  }

  .brand {
    display: flex;
    align-items: baseline;
    gap: 6px;
  }

  .name {
    font-weight: 700;
    letter-spacing: 0.5px;
    font-size: 15px;
  }
  .tag {
    color: #8a949d;
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 1px;
  }

  .tci {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 0 1 320px;
  }

  .dot {
    font-size: 18px;
    line-height: 1;
    display: inline-block;
  }

  .dot.ok { color: #4ade80; }
  .dot.warn { color: #fbbf24; }
  .dot.err { color: #f87171; }
  .dot.off { color: #6b7176; }

  .tci input {
    flex: 1;
    background: #181c1f;
    border: 1px solid #262b30;
    color: #c5d1de;
    padding: 4px 8px;
    border-radius: 3px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
  }

  .tci input:disabled { opacity: 0.7; }

  .conn-btn {
    border: 1px solid;
    color: #e6e6e6;
    padding: 4px 12px;
    border-radius: 3px;
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
    white-space: nowrap;
  }
  .conn-btn.connect {
    background: #2a5a3f;
    border-color: #3a8a5f;
  }
  .conn-btn.connect:hover { background: #357050; }
  .conn-btn.disconnect {
    background: #3a3a4a;
    border-color: #4a4a5a;
    color: #c5d1de;
  }
  .conn-btn.disconnect:hover {
    background: #4a4a5a;
    border-color: #f87171;
    color: #f87171;
  }

  .rig {
    display: flex;
    align-items: baseline;
    gap: 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .rig .freq {
    font-size: 18px;
    font-weight: 600;
    letter-spacing: 0.5px;
  }
  .rig .band { color: #fbbf24; font-size: 12px; }
  .rig .mode { color: #8a949d; font-size: 12px; }
  .rig .ptt { color: #6b7176; font-weight: 600; }
  .rig .ptt.on { color: #f87171; }

  .score {
    margin-left: auto;
    display: flex;
    gap: 18px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .stat {
    display: flex;
    align-items: baseline;
    gap: 4px;
  }
  .stat .lbl { color: #6b7176; font-size: 10px; text-transform: uppercase; letter-spacing: 0.5px; }
  .stat .num { color: #e6e6e6; font-size: 14px; font-weight: 600; }

  .err-banner {
    background: #3f1d1d;
    color: #f87171;
    padding: 4px 16px;
    font-size: 12px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }
</style>
