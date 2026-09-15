<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { onScope, type ScopeFrame } from "$lib/tci";
  import { rttyConfig } from "$lib/rttyConfig.svelte";

  // Square canvas. Internal resolution; CSS scales to display size.
  const SIZE = 200;

  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D | null = null;
  let unlisten: (() => void) | null = null;

  // Auto-scale: track a slowly-decaying peak so the trace fills the scope
  // regardless of signal level.
  let peak = 0.0001;

  function drawCrosshair() {
    if (!ctx) return;
    ctx.strokeStyle = "rgba(120, 130, 140, 0.35)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    ctx.moveTo(SIZE / 2, 0);
    ctx.lineTo(SIZE / 2, SIZE);
    ctx.moveTo(0, SIZE / 2);
    ctx.lineTo(SIZE, SIZE / 2);
    ctx.stroke();
  }

  function onFrame(f: ScopeFrame) {
    if (!ctx || f.xs.length === 0) return;

    // Phosphor fade: dim the whole canvas a touch each frame.
    ctx.fillStyle = "rgba(8, 12, 14, 0.30)";
    ctx.fillRect(0, 0, SIZE, SIZE);

    // Update peak from this frame.
    let frameMax = 0;
    for (let i = 0; i < f.xs.length; i++) {
      const a = Math.abs(f.xs[i]);
      const b = Math.abs(f.ys[i]);
      if (a > frameMax) frameMax = a;
      if (b > frameMax) frameMax = b;
    }
    // Fast attack, slow decay so the display stays stable.
    if (frameMax > peak) peak = frameMax;
    else peak = peak * 0.95 + frameMax * 0.05;
    const scale = (SIZE / 2) * 0.9 / (peak + 1e-6);

    // Trace the XY path.
    ctx.strokeStyle = "rgba(74, 222, 128, 0.85)";
    ctx.lineWidth = 1;
    ctx.beginPath();
    for (let i = 0; i < f.xs.length; i++) {
      const px = SIZE / 2 + f.xs[i] * scale;
      const py = SIZE / 2 - f.ys[i] * scale;
      if (i === 0) ctx.moveTo(px, py);
      else ctx.lineTo(px, py);
    }
    ctx.stroke();

    drawCrosshair();
  }

  onMount(async () => {
    const c = canvas.getContext("2d", { alpha: false });
    if (!c) return;
    ctx = c;
    ctx.fillStyle = "#080c0e";
    ctx.fillRect(0, 0, SIZE, SIZE);
    drawCrosshair();
    unlisten = await onScope(onFrame);
  });

  // Nudge arrows: fine-tune the mark a few Hz at a time while watching the
  // crossed-bananas display — the manual counterpart to AFC. A deliberate
  // tuning action, so RX and TX move together (like a waterfall click).
  // Click = one step; hold = repeat.
  const NUDGE_HZ = 5;
  const REPEAT_DELAY_MS = 350;
  const REPEAT_EVERY_MS = 80;
  let repeatTimer: ReturnType<typeof setTimeout> | null = null;

  function nudge(dir: -1 | 1) {
    rttyConfig.setMark(rttyConfig.markHz + dir * NUDGE_HZ);
  }

  function startNudge(dir: -1 | 1) {
    stopNudge();
    nudge(dir);
    repeatTimer = setTimeout(function tick() {
      nudge(dir);
      repeatTimer = setTimeout(tick, REPEAT_EVERY_MS);
    }, REPEAT_DELAY_MS);
  }

  function stopNudge() {
    if (repeatTimer) clearTimeout(repeatTimer);
    repeatTimer = null;
  }

  onDestroy(() => {
    unlisten?.();
    stopNudge();
  });
</script>

<div class="scope">
  <canvas bind:this={canvas} width={SIZE} height={SIZE}></canvas>
  <div class="caption">tuning</div>
  <div class="nudge">
    <button
      type="button"
      title="Nudge mark down {NUDGE_HZ} Hz (hold to repeat)"
      onpointerdown={() => startNudge(-1)}
      onpointerup={stopNudge}
      onpointerleave={stopNudge}
      onpointercancel={stopNudge}
    >◀</button>
    <span class="hz">{rttyConfig.markHz.toFixed(0)}</span>
    <button
      type="button"
      title="Nudge mark up {NUDGE_HZ} Hz (hold to repeat)"
      onpointerdown={() => startNudge(1)}
      onpointerup={stopNudge}
      onpointerleave={stopNudge}
      onpointercancel={stopNudge}
    >▶</button>
  </div>
</div>

<style>
  .scope {
    position: relative;
    flex: 0 0 auto;
  }
  canvas {
    display: block;
    width: 150px;
    height: 150px;
    background: #080c0e;
    border: 1px solid #262b30;
    border-radius: 4px;
  }
  .caption {
    position: absolute;
    top: 4px;
    left: 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 9px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: #5a636c;
    pointer-events: none;
  }
  .nudge {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 4px;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 10px;
    color: #8a949d;
  }
  .nudge .hz { min-width: 34px; text-align: center; color: #c5d1de; }
  .nudge button {
    background: rgba(24, 28, 31, 0.85);
    border: 1px solid #3a4452;
    color: #8a949d;
    border-radius: 3px;
    padding: 1px 7px;
    font-size: 10px;
    line-height: 1.4;
    cursor: pointer;
    user-select: none;
    touch-action: none;
  }
  .nudge button:hover { color: #c5d1de; border-color: #5a6573; }
  .nudge button:active { background: #2a3f5f; border-color: #4a90e2; color: #e6e6e6; }
</style>
