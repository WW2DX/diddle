<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { onScope, type ScopeFrame } from "$lib/tci";

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

  onDestroy(() => unlisten?.());
</script>

<div class="scope">
  <canvas bind:this={canvas} width={SIZE} height={SIZE}></canvas>
  <div class="caption">tuning</div>
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
</style>
