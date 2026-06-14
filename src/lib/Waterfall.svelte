<script lang="ts">
  import { onMount, onDestroy } from "svelte";
  import { onRig, onSpectrum, setFreq, type RigState } from "$lib/tci";
  import { rttyConfig } from "$lib/rttyConfig.svelte";
  import { spots } from "$lib/spots.svelte";
  import { cluster } from "$lib/cluster.svelte";
  import { bandFromHz } from "$lib/bands";

  // Canvas dimensions. Width matches the half-spectrum (fft_size/2).
  const WIDTH = 1024;
  const HEIGHT = 260;

  // dB range — user-adjustable via sliders. Wide defaults so any audio scale
  // (normalized [-1,1] or raw int values stored in f32) produces a visible image.
  let dbMin = $state(-90);
  let dbMax = $state(0);

  // Visible audio span (Hz). RHR's SSB filter delivers ~6 kHz of useful
  // signal but RTTY operating typically wants a closer view — default to
  // 3 kHz so individual mark/space pairs are immediately visible.
  let viewSpanHz = $state(3000);
  const SPAN_PRESETS = [3000, 6000, 12000, 24000];

  // Snap-to-peak: when clicking, find the strongest bin within ±SNAP_HZ
  // and set mark there. Makes click-to-tune pixel-forgiving.
  let snapToPeak = $state(true);
  const SNAP_HZ = 150;
  // Cache the most-recent spectrum frame for snap lookups.
  let lastMags: number[] = [];
  let lastFftSize = 0;

  // Smoothed spectrum + peak finder (for visual peak ticks and Auto-tune).
  // The smoothing averages out the per-frame scintillation so peak picking
  // is stable.
  let smoothedMags: Float32Array | null = null;
  const SMOOTH_ALPHA = 0.12;
  const PEAK_WINDOW = 6; // ±bins for local-max check
  const PEAK_MIN_DB_ABOVE_MEAN = 7;
  const MAX_PEAKS = 8;
  let peakHzs = $state<number[]>([]);
  let peakSearchCounter = 0;

  const SHIFT_CANDIDATES = [170, 200, 425, 850];
  const AUTO_TUNE_TOLERANCE_HZ = 25;

  // AFC tracking: when on, every TRACK_INTERVAL_MS we re-center the mark on
  // the strongest peak within ±TRACK_WINDOW_HZ of the current mark. Used to
  // follow slow drift on real-world signals.
  let tracking = $state(false);
  const TRACK_INTERVAL_MS = 1500;
  const TRACK_WINDOW_HZ = 40;
  const TRACK_MIN_DELTA_HZ = 2;
  let trackTimer: ReturnType<typeof setInterval> | null = null;

  let canvas: HTMLCanvasElement;
  let ctx: CanvasRenderingContext2D | null = null;
  let fps = $state(0);
  let lastT = 0;
  let lastFrameAt = $state(0);
  let sampleRate = $state(0);
  let rig = $state<RigState>({ freq: 0, mode: "", ptt: false });

  // "Live" = we've seen a spectrum frame within the last LIVE_WINDOW_MS.
  // The audio stream auto-starts when TCI is ready, so the operator never
  // needs to drive it manually; this just reports whether it's currently
  // flowing.
  const LIVE_WINDOW_MS = 1500;
  let nowMs = $state(0);
  let live = $derived(lastFrameAt > 0 && nowMs - lastFrameAt < LIVE_WINDOW_MS);

  // Stats for the auto-calibrate feature.
  let lastFrameMin = $state(0);
  let lastFrameMax = $state(0);
  let lastFrameMean = $state(0);

  const unlisten: Array<() => void> = [];

  // Viridis-style colormap LUT (256 entries × RGBA). Perceptually uniform.
  const LUT = makeViridisLut();

  function makeViridisLut(): Uint8ClampedArray {
    // Anchor points sampled from matplotlib's viridis at t = 0, 0.25, 0.5, 0.75, 1.
    const stops: Array<[number, [number, number, number]]> = [
      [0.0, [68, 1, 84]],
      [0.25, [59, 82, 139]],
      [0.5, [33, 144, 141]],
      [0.75, [94, 201, 98]],
      [1.0, [253, 231, 37]],
    ];
    const a = new Uint8ClampedArray(256 * 4);
    for (let i = 0; i < 256; i++) {
      const t = i / 255;
      let lo = stops[0],
        hi = stops[stops.length - 1];
      for (let j = 0; j < stops.length - 1; j++) {
        if (t >= stops[j][0] && t <= stops[j + 1][0]) {
          lo = stops[j];
          hi = stops[j + 1];
          break;
        }
      }
      const f = (t - lo[0]) / (hi[0] - lo[0] || 1);
      a[i * 4] = Math.round(lo[1][0] + f * (hi[1][0] - lo[1][0]));
      a[i * 4 + 1] = Math.round(lo[1][1] + f * (hi[1][1] - lo[1][1]));
      a[i * 4 + 2] = Math.round(lo[1][2] + f * (hi[1][2] - lo[1][2]));
      a[i * 4 + 3] = 255;
    }
    return a;
  }

  function renderRow(mags: number[]) {
    if (!ctx) return;
    ctx.drawImage(canvas, 0, 1, WIDTH, HEIGHT - 1, 0, 0, WIDTH, HEIGHT - 1);
    const row = ctx.createImageData(WIDTH, 1);
    const px = row.data;
    const range = dbMax - dbMin || 1;
    const totalBins = mags.length;

    // How many FFT bins cover the user-selected view span.
    const binHz = sampleRate > 0 ? sampleRate / (2 * totalBins) : 0;
    let visBins = totalBins;
    if (binHz > 0) {
      visBins = Math.min(totalBins, Math.max(1, Math.ceil(viewSpanHz / binHz)));
    }

    // Stats over the visible range only — that's what the user sees.
    let fmin = Infinity,
      fmax = -Infinity,
      fsum = 0;
    for (let i = 0; i < visBins; i++) {
      const db = mags[i];
      if (db < fmin) fmin = db;
      if (db > fmax) fmax = db;
      fsum += db;
    }

    // Map each canvas column → a visible bin (nearest-neighbour, no smoothing).
    for (let x = 0; x < WIDTH; x++) {
      const bin = Math.min(visBins - 1, (x * visBins / WIDTH) | 0);
      const db = mags[bin];
      let t = (db - dbMin) / range;
      if (t < 0) t = 0;
      else if (t > 1) t = 1;
      const idx = (t * 255) | 0;
      const o = idx << 2;
      const p = x << 2;
      px[p] = LUT[o];
      px[p + 1] = LUT[o + 1];
      px[p + 2] = LUT[o + 2];
      px[p + 3] = 255;
    }
    ctx.putImageData(row, 0, HEIGHT - 1);
    lastFrameMin = fmin;
    lastFrameMax = fmax;
    lastFrameMean = fsum / visBins;
  }

  function autoCalibrate() {
    // 5 dB below observed min → 5 dB above observed max gives a comfortable
    // viewing range. Float values are already in dB so no log needed.
    if (!isFinite(lastFrameMin) || !isFinite(lastFrameMax)) return;
    dbMin = Math.round(lastFrameMin - 5);
    dbMax = Math.round(lastFrameMax + 5);
  }

  function clearCanvas() {
    if (!ctx) return;
    ctx.fillStyle = "#000";
    ctx.fillRect(0, 0, WIDTH, HEIGHT);
  }

  onMount(async () => {
    const c = canvas.getContext("2d", { alpha: false });
    if (!c) return;
    ctx = c;
    clearCanvas();

    unlisten.push(
      await onSpectrum((f) => {
        sampleRate = f.sample_rate;
        lastMags = f.mags_db;
        lastFftSize = f.fft_size;
        const now = performance.now();
        if (lastT > 0) {
          const dt = (now - lastT) / 1000;
          if (dt > 0) fps = 0.9 * fps + 0.1 * (1 / dt);
        }
        lastT = now;
        lastFrameAt = Date.now();
        renderRow(f.mags_db);
        updateSmoothed(f.mags_db);
        // Recompute peaks ~5 Hz to keep UI cheap.
        peakSearchCounter++;
        if (peakSearchCounter >= 10) {
          peakSearchCounter = 0;
          peakHzs = findPeaks();
        }
      }),
    );

    // Tick the clock so the "live" indicator can drop when frames stop.
    const tick = setInterval(() => (nowMs = Date.now()), 500);
    unlisten.push(() => clearInterval(tick));

    unlisten.push(
      await onRig((r) => {
        // Drop stale labels when the VFO jumps — their audio_hz no longer
        // reflects the signal's actual position.
        if (rig.freq && Math.abs(r.freq - rig.freq) > 500) {
          spots.clear();
        }
        rig = r;
      }),
    );
  });

  // QSY to a clicked spot: retune the radio so the signal lands at the
  // user's preferred mark tone. In USB: vfo_new = signal_abs_hz - markHz.
  async function qsyToAbs(abs_hz: number) {
    if (!abs_hz) return;
    const newVfo = Math.round(abs_hz - rttyConfig.markHz);
    try {
      await setFreq(newVfo);
    } catch (e) {
      console.error("set_freq failed", e);
    }
  }

  // Unified overlay: decoder + cluster spots, deduped by call (cluster wins
  // because it carries an accurate absolute frequency and a comment).
  type Overlay = {
    call: string;
    audio_hz: number;
    abs_hz: number;
    source: "decoder" | "cluster";
    timestamp_ms: number;
    comment?: string;
  };

  let overlays = $derived.by<Overlay[]>(() => {
    if (!rig.freq) return [];
    const band = bandFromHz(rig.freq);
    const map = new Map<string, Overlay>();

    for (const s of spots.spots) {
      const key = s.call.toUpperCase();
      map.set(key, {
        call: key,
        audio_hz: s.audio_hz,
        abs_hz: rig.freq + s.audio_hz,
        source: "decoder",
        timestamp_ms: s.timestamp_ms,
      });
    }

    for (const s of cluster.spots) {
      if (s.band !== band) continue;
      const audio_hz = s.freq_hz - rig.freq;
      if (audio_hz < 0 || audio_hz > viewSpanHz) continue;
      const key = s.dx_call.toUpperCase();
      map.set(key, {
        call: key,
        audio_hz,
        abs_hz: s.freq_hz,
        source: "cluster",
        timestamp_ms: s.timestamp_ms,
        comment: s.comment,
      });
    }

    return [...map.values()];
  });

  onDestroy(() => {
    for (const u of unlisten) u();
  });

  let topFreqHz = $derived(rig.freq + viewSpanHz);

  // Map audio Hz → percent of the canvas width (for the marker overlay).
  function pctForAudioHz(hz: number): number {
    return (hz / viewSpanHz) * 100;
  }

  let markPct = $derived(pctForAudioHz(rttyConfig.markHz));
  let spacePct = $derived(pctForAudioHz(rttyConfig.spaceHz));
  let markInView = $derived(
    rttyConfig.markHz >= 0 && rttyConfig.markHz <= viewSpanHz,
  );
  let spaceInView = $derived(
    rttyConfig.spaceHz >= 0 && rttyConfig.spaceHz <= viewSpanHz,
  );

  // EMA-smooth incoming spectrum frames so peak picking is stable.
  function updateSmoothed(mags: number[]) {
    if (!smoothedMags || smoothedMags.length !== mags.length) {
      smoothedMags = new Float32Array(mags);
      return;
    }
    const a = SMOOTH_ALPHA;
    for (let i = 0; i < mags.length; i++) {
      smoothedMags[i] = (1 - a) * smoothedMags[i] + a * mags[i];
    }
  }

  // Find local maxima at least PEAK_MIN_DB_ABOVE_MEAN dB above the band mean.
  // Returns frequencies (Hz) with parabolic sub-bin interpolation, sorted by
  // magnitude descending.
  function findPeaks(): number[] {
    if (!smoothedMags || !sampleRate || !lastFftSize) return [];
    const n = smoothedMags.length;
    const binHz = sampleRate / lastFftSize;
    let sum = 0;
    for (let i = 0; i < n; i++) sum += smoothedMags[i];
    const mean = sum / n;
    const threshold = mean + PEAK_MIN_DB_ABOVE_MEAN;
    const win = PEAK_WINDOW;
    const cands: Array<{ bin: number; db: number }> = [];
    for (let i = win; i < n - win; i++) {
      const v = smoothedMags[i];
      if (v < threshold) continue;
      let isPeak = true;
      for (let j = i - win; j <= i + win; j++) {
        if (j !== i && smoothedMags[j] >= v) {
          isPeak = false;
          break;
        }
      }
      if (isPeak) cands.push({ bin: i, db: v });
    }
    cands.sort((a, b) => b.db - a.db);
    return cands.slice(0, MAX_PEAKS).map((c) => {
      // Parabolic sub-bin interpolation around (k-1, k, k+1).
      const k = c.bin;
      const a = smoothedMags![k - 1];
      const b = c.db;
      const cc = smoothedMags![k + 1];
      const denom = a - 2 * b + cc;
      let offset = 0;
      if (Math.abs(denom) > 1e-9) {
        offset = (0.5 * (a - cc)) / denom;
        if (offset < -1) offset = -1;
        else if (offset > 1) offset = 1;
      }
      return (k + offset) * binHz;
    });
  }

  // Look up the smoothed dB magnitude at a given audio frequency (nearest bin).
  function dbAt(hz: number): number {
    if (!smoothedMags || !sampleRate || !lastFftSize) return -Infinity;
    const binHz = sampleRate / lastFftSize;
    const k = Math.round(hz / binHz);
    if (k < 0 || k >= smoothedMags.length) return -Infinity;
    return smoothedMags[k];
  }

  // AFC tracker: every TRACK_INTERVAL_MS, recenter the mark tone on the
  // strongest bin within ±TRACK_WINDOW_HZ of current. Effective for slow
  // drift; ignored if there's no peak in the window.
  function trackingTick() {
    if (!smoothedMags || !sampleRate || !lastFftSize) return;
    const binHz = sampleRate / lastFftSize;
    const center = rttyConfig.markHz;
    const winBins = Math.max(1, Math.round(TRACK_WINDOW_HZ / binHz));
    const centerBin = Math.round(center / binHz);
    const lo = Math.max(1, centerBin - winBins);
    const hi = Math.min(smoothedMags.length - 1, centerBin + winBins);
    let bestBin = centerBin;
    let bestDb = -Infinity;
    for (let b = lo; b < hi; b++) {
      if (smoothedMags[b] > bestDb) {
        bestDb = smoothedMags[b];
        bestBin = b;
      }
    }
    // Parabolic refine.
    let offset = 0;
    if (bestBin > 0 && bestBin < smoothedMags.length - 1) {
      const a = smoothedMags[bestBin - 1];
      const c = smoothedMags[bestBin + 1];
      const d = a - 2 * bestDb + c;
      if (Math.abs(d) > 1e-9) {
        offset = (0.5 * (a - c)) / d;
        if (offset < -1) offset = -1;
        else if (offset > 1) offset = 1;
      }
    }
    const newMark = (bestBin + offset) * binHz;
    if (
      Math.abs(newMark - center) > TRACK_MIN_DELTA_HZ &&
      Math.abs(newMark - center) < TRACK_WINDOW_HZ
    ) {
      rttyConfig.setMark(Math.round(newMark));
    }
  }

  $effect(() => {
    if (tracking) {
      trackTimer = setInterval(trackingTick, TRACK_INTERVAL_MS);
    } else if (trackTimer) {
      clearInterval(trackTimer);
      trackTimer = null;
    }
  });

  // Auto-tune: pick the best two peaks whose separation matches a standard
  // RTTY shift. Scoring rewards:
  //   - strong peaks (peakHzs sorted by magnitude)
  //   - shift close to a standard
  //   - 170 Hz bias (most common amateur RTTY shift)
  //   - similar magnitudes between mark and space (real RTTY mark/space
  //     have ~equal energy; a strong loner paired with a weak distant peak
  //     is probably not RTTY)
  function autoTune() {
    if (peakHzs.length < 2) return;
    let best: { mark: number; shift: number; score: number } | null = null;
    for (const shift of SHIFT_CANDIDATES) {
      for (let i = 0; i < peakHzs.length; i++) {
        for (let j = 0; j < peakHzs.length; j++) {
          if (i === j) continue;
          const a = peakHzs[i];
          const b = peakHzs[j];
          if (b <= a) continue;
          const actual = b - a;
          const err = Math.abs(actual - shift);
          if (err > AUTO_TUNE_TOLERANCE_HZ) continue;
          const rankScore = (MAX_PEAKS - i) + (MAX_PEAKS - j);
          const stdBonus = shift === 170 ? 5 : 0;
          const magDiff = Math.abs(dbAt(a) - dbAt(b));
          const magBonus = Math.max(0, 10 - magDiff); // up to 10 if ≤ 0 dB apart
          const score = rankScore * 10 + stdBonus + magBonus - err;
          if (!best || score > best.score) {
            best = { mark: a, shift, score };
          }
        }
      }
    }
    if (best) {
      rttyConfig.setShift(best.shift);
      rttyConfig.setMark(Math.round(best.mark));
    }
  }

  // Click on the waterfall → set mark tone at that audio frequency.
  // If snap-to-peak is on, find the loudest bin within ±SNAP_HZ.
  function handleClick(e: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const fraction = Math.max(0, Math.min(1, x / rect.width));
    let audioHz = fraction * viewSpanHz;

    if (snapToPeak && lastMags.length > 0 && sampleRate > 0 && lastFftSize > 0) {
      const binHz = sampleRate / lastFftSize;
      const centerBin = Math.round(audioHz / binHz);
      const windowBins = Math.max(1, Math.round(SNAP_HZ / binHz));
      const lo = Math.max(0, centerBin - windowBins);
      const hi = Math.min(lastMags.length, centerBin + windowBins + 1);
      let bestBin = centerBin;
      let bestDb = -Infinity;
      for (let b = lo; b < hi; b++) {
        if (lastMags[b] > bestDb) {
          bestDb = lastMags[b];
          bestBin = b;
        }
      }
      audioHz = bestBin * binHz;
    }
    rttyConfig.setMark(Math.round(audioHz));
  }

  function fmtMhz(hz: number): string {
    if (!hz) return "—";
    return (hz / 1_000_000).toFixed(3);
  }

  function spanLabel(hz: number): string {
    return hz >= 1000 ? `${(hz / 1000).toFixed(0)} kHz` : `${hz} Hz`;
  }
</script>

<section class="panel">
  <header>
    <h2>Waterfall</h2>
    <div class="meta">
      {#if live}
        <span class="ok">● live</span>
      {:else}
        <span class="off">○ idle</span>
      {/if}
      <span>{fps.toFixed(1)} fps</span>
      <span>view {spanLabel(viewSpanHz)}</span>
      <span class="stats">
        last: <span class="num">{lastFrameMin.toFixed(0)}</span>…<span
          class="num">{lastFrameMax.toFixed(0)}</span
        >
        <span class="dim">(mean {lastFrameMean.toFixed(0)})</span>
        dB
      </span>
    </div>
    <div class="controls">
      <button class="ghost" onclick={autoCalibrate}>Auto dB</button>
      <button
        class="ghost"
        onclick={autoTune}
        disabled={peakHzs.length < 2}
        title="Find the best RTTY pair among detected peaks"
      >
        Auto-tune
      </button>
      <label class="track-label" title="Continuously re-center mark on the strongest nearby peak">
        <input type="checkbox" bind:checked={tracking} />
        AFC
      </label>
      <button class="ghost" onclick={clearCanvas}>Clear</button>
    </div>
  </header>

  <div class="db-controls">
    <label>
      min <span class="num">{dbMin}</span>
      <input
        type="range"
        min="-150"
        max="50"
        step="1"
        bind:value={dbMin}
      />
    </label>
    <label>
      max <span class="num">{dbMax}</span>
      <input
        type="range"
        min="-150"
        max="50"
        step="1"
        bind:value={dbMax}
      />
    </label>
    <div class="span-buttons">
      <span class="dim">span:</span>
      {#each SPAN_PRESETS as s}
        <button
          class="ghost span"
          class:active={viewSpanHz === s}
          onclick={() => (viewSpanHz = s)}
        >
          {spanLabel(s)}
        </button>
      {/each}
      <label class="snap-label">
        <input type="checkbox" bind:checked={snapToPeak} />
        snap to peak
      </label>
    </div>
  </div>

  <div class="freq-axis">
    <span>{fmtMhz(rig.freq)}</span>
    <span class="dim">click → set mark · USB → ↑{spanLabel(viewSpanHz)}</span>
    <span>{fmtMhz(topFreqHz)}</span>
  </div>

  <div class="canvas-wrap">
    <canvas
      bind:this={canvas}
      width={WIDTH}
      height={HEIGHT}
      onclick={handleClick}
    ></canvas>
    {#each peakHzs as hz, i}
      {@const pct = pctForAudioHz(hz)}
      {#if pct >= 0 && pct <= 100}
        <div
          class="peak-tick"
          class:strong={i < 2}
          style="left: {pct}%"
          title="peak {i + 1}: {hz.toFixed(0)} Hz"
        ></div>
      {/if}
    {/each}

    {#each overlays as o (o.source + ":" + o.call)}
      {@const pct = pctForAudioHz(o.audio_hz)}
      {#if pct >= 0 && pct <= 100}
        <button
          class="spot-label {o.source}"
          style="left: {pct}%"
          onclick={() => qsyToAbs(o.abs_hz)}
          title={`${o.source === "cluster" ? "cluster" : "decoded"} · ${o.call} · ${o.audio_hz.toFixed(0)} Hz · ${new Date(o.timestamp_ms).toLocaleTimeString()}${o.comment ? " · " + o.comment : ""}`}
        >
          {o.call}
        </button>
      {/if}
    {/each}
    {#if markInView}
      <div class="marker mark" style="left: {markPct}%">
        <span class="label">M {rttyConfig.markHz.toFixed(0)}</span>
      </div>
    {/if}
    {#if spaceInView}
      <div class="marker space" style="left: {spacePct}%">
        <span class="label">S {rttyConfig.spaceHz.toFixed(0)}</span>
      </div>
    {/if}
  </div>
</section>

<style>
  .panel {
    background: #181c1f;
    border: 1px solid #262b30;
    border-radius: 8px;
    padding: 16px 18px;
    margin-bottom: 16px;
  }

  header {
    display: flex;
    align-items: center;
    gap: 16px;
    margin-bottom: 10px;
    flex-wrap: wrap;
  }

  h2 {
    margin: 0;
    font-size: 13px;
    text-transform: uppercase;
    letter-spacing: 1px;
    color: #8a949d;
    font-weight: 600;
    flex-shrink: 0;
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 14px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 12px;
    color: #8a949d;
    flex: 1;
    flex-wrap: wrap;
  }

  .num {
    color: #c5d1de;
  }

  .ok {
    color: #4ade80;
  }
  .off {
    color: #6b7176;
  }
  .dim {
    color: #5a636c;
  }

  .controls {
    display: flex;
    gap: 6px;
  }

  button {
    background: #2a3f5f;
    border: 1px solid #3a5a8a;
    color: #e6e6e6;
    border-radius: 4px;
    padding: 5px 12px;
    cursor: pointer;
    font-size: 12px;
    font-weight: 500;
  }

  button.ghost {
    background: transparent;
    border-color: #3a4452;
    color: #8a949d;
  }

  button:hover {
    background: #345080;
  }
  button.ghost:hover {
    border-color: #5a6573;
    color: #c5d1de;
  }

  .db-controls {
    display: flex;
    gap: 24px;
    margin-bottom: 8px;
    font-size: 11px;
    color: #8a949d;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
  }

  .db-controls label {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
  }

  .db-controls input[type="range"] {
    flex: 1;
    accent-color: #4a90e2;
  }

  .span-buttons {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  button.span {
    padding: 3px 8px;
    font-size: 11px;
  }

  button.span.active {
    background: #2a3f5f;
    border-color: #4a90e2;
    color: #e6e6e6;
  }

  .snap-label,
  .track-label {
    display: flex;
    align-items: center;
    gap: 4px;
    color: #8a949d;
    font-size: 11px;
    cursor: pointer;
    margin-left: 8px;
  }

  .track-label {
    margin-left: 0;
  }

  .freq-axis {
    display: flex;
    justify-content: space-between;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    color: #8a949d;
    padding: 0 2px 4px;
  }

  .canvas-wrap {
    position: relative;
    width: 100%;
    height: 260px;
  }

  canvas {
    display: block;
    width: 100%;
    height: 100%;
    image-rendering: pixelated;
    background: #000;
    border: 1px solid #262b30;
    border-radius: 4px;
    cursor: crosshair;
  }

  .marker {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 1px;
    pointer-events: none;
    transform: translateX(-0.5px);
  }
  .marker.mark { background: #4ade80; box-shadow: 0 0 4px #4ade80; }
  .marker.space { background: #fbbf24; box-shadow: 0 0 4px #fbbf24; }

  .marker .label {
    position: absolute;
    top: 4px;
    left: 4px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 10px;
    font-weight: 600;
    color: #0a0c0d;
    background: inherit;
    padding: 1px 4px;
    border-radius: 2px;
    white-space: nowrap;
    box-shadow: none;
  }
  .marker.mark .label { background: #4ade80; }
  .marker.space .label { background: #fbbf24; top: 18px; }

  .peak-tick {
    position: absolute;
    top: -1px;
    width: 0;
    height: 0;
    pointer-events: none;
    border-left: 4px solid transparent;
    border-right: 4px solid transparent;
    border-top: 6px solid rgba(200, 200, 200, 0.5);
    transform: translateX(-4px);
  }
  .peak-tick.strong {
    border-top-color: rgba(255, 255, 255, 0.9);
  }
  .peak-tick:hover {
    border-top-color: #4a90e2;
  }

  .spot-label {
    position: absolute;
    transform: translateX(-50%);
    border-radius: 3px;
    padding: 1px 6px;
    font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.5px;
    cursor: pointer;
    box-shadow: 0 2px 4px rgba(0, 0, 0, 0.5);
    z-index: 2;
    pointer-events: auto;
  }
  .spot-label.decoder {
    top: 8px;
    background: rgba(74, 222, 128, 0.92);
    color: #0a1a10;
    border: 1px solid #2a8a4a;
  }
  .spot-label.cluster {
    top: 28px;
    background: rgba(74, 144, 226, 0.92);
    color: #06121f;
    border: 1px solid #2a5a8a;
  }
  .spot-label:hover {
    background: #fbbf24;
    border-color: #fff;
    color: #1a0f00;
  }
</style>
