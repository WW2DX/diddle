// Plays the screenshot scene into the running app through the stand-in
// backend: waterfall, decoder tags, cluster spots, RX/Deep text (real
// decoder output from rx.json), then the operator's right-clicks.
async (RX) => {
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const E = window.__emit;
  const RIG = window.__RIG;
  const now = Date.now();
  E("tci:state", { kind: "connected", url: "ws://127.0.0.1:40001", ready: true });
  E("tci:rig", RIG);
  E("cluster:state", { kind: "connected", host: "dxc.k1ttt.net", port: 7373 });
  E("rtty_deep:status", { state: "on", message: "listening" });
  await sleep(300);

  // --- Waterfall: seven RTTY stations in the 3 kHz view (audio Hz). ---
  const SIGS = [
    { call: "SM5XYZ", mark: 515, lvl: -57, from: 0.0, to: 1.0 },
    { call: "UA9XYZ", mark: 880, lvl: -47, from: 0.0, to: 1.0 },
    { call: "LU5ABC", mark: 1290, lvl: -66, from: 0.25, to: 1.0 },
    { call: "VE3XYZ", mark: 1625, lvl: -52, from: 0.0, to: 0.8 },
    { call: "K4XYZ", mark: 2138, lvl: -45, from: 0.45, to: 1.0 },
    { call: "PY5ABC", mark: 2555, lvl: -54, from: 0.0, to: 1.0 },
    { call: "JA1XYZ", mark: 2850, lvl: -62, from: 0.1, to: 0.55 },
  ];
  const FFT = 4096, SR = 48000, BINS = FFT / 2, binHz = SR / FFT;
  let seed = 7;
  const rnd = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
  const gauss = () => Math.sqrt(-2 * Math.log(rnd() + 1e-9)) * Math.cos(2 * Math.PI * rnd());
  const N = 300;
  const frame = (f) => {
    const t = Math.min(1, f / N);
    const lin = new Float64Array(BINS);
    for (let b = 0; b < BINS; b++) lin[b] = Math.pow(10, (-88 + 3 * gauss()) / 10);
    for (const s of SIGS) {
      if (t < s.from || t > s.to) continue;
      const fade = 4 * Math.sin(2 * Math.PI * (t * 3 + s.mark / 700));
      const mark = rnd() < 0.55;
      for (const [hz, rel] of [[mark ? s.mark : s.mark + 170, 0], [mark ? s.mark + 170 : s.mark, -14]]) {
        const c = hz / binHz;
        for (let b = Math.floor(c) - 3; b <= Math.ceil(c) + 3; b++) {
          const d = b - c;
          lin[b] += Math.pow(10, (s.lvl + fade + rel - 9 * d * d + 2 * gauss()) / 10);
        }
      }
    }
    const mags_db = Array.from(lin, (v) => 10 * Math.log10(v));
    E("spectrum", { mags_db, fft_size: FFT, sample_rate: SR, seq: f });
  };
  // At the real frame rate (48 kHz / 1024-sample stride ≈ 47 fps), and on
  // through the capture so the waterfall stays live.
  for (let f = 0; f < N; f++) {
    frame(f);
    await sleep(21);
  }
  let more = N;
  setInterval(() => frame(more++), 21);

  // --- Decoder tags and cluster spots. ---
  for (const s of SIGS) {
    if (s.to < 0.9) continue;
    E("signal:spot", { audio_hz: s.mark + 85, mark_hz: s.mark, space_hz: s.mark + 170, call: s.call, timestamp_ms: now - 4000 });
  }
  const spot = (call, khz, comment, ago) =>
    E("cluster:spot", {
      source: "DXC", dx_call: call, freq_hz: Math.round(khz * 1000), band: "20m",
      comment, time_utc: "", timestamp_ms: now - ago,
    });
  spot("VE3XYZ", 14085.3, "RTTY CQ", 120_000);
  spot("ZL1XYZ", 14080.4, "RTTY up 1", 300_000);
  spot("VP8ABC", 14088.9, "RTTY loud", 200_000);
  spot("D4ABC", 14091.2, "RTTY", 420_000);
  spot("3B8XYZ", 14093.6, "RTTY CQ TEST", 90_000);
  spot("TF3ABC", 14095.1, "RTTY", 600_000);
  spot("EA8XYZ", 14079.2, "RTTY", 520_000);
  spot("ZD8ABC", 14097.8, "RTTY CQ", 60_000);
  E("multi:slots", 7);

  // --- Tuning scope: mark / space traces of the station being worked. ---
  for (let k = 0; k < 12; k++) {
    const xs = [], ys = [];
    for (let i = 0; i < 240; i++) {
      const ph = (i / 240) * 2 * Math.PI * 9;
      const markBit = Math.floor(i / 30) % 2 === 0;
      xs.push((markBit ? 1 : 0.08) * Math.cos(ph) + 0.03 * gauss());
      ys.push((markBit ? 0.08 : 1) * Math.sin(ph) + 0.03 * gauss());
    }
    E("scope", { xs, ys });
    await sleep(20);
  }

  // --- RX window: our TX echo + the callers, as the real decoders copied them. ---
  const tx = (s) => E("tx:echo", s);
  const rx = (i) => {
    E("rtty", RX[i].classic + "\n");
    E("rtty_deep", RX[i].deep + "\n");
  };
  tx("CQ TEST WW2DX WW2DX CQ\n"); await sleep(60);
  rx(0); await sleep(60); rx(1); await sleep(60);
  tx("DL1ABC 599 05 05 DL1ABC\n"); await sleep(60);
  rx(2); await sleep(60);
  tx("TU DL1ABC NOW\nJA1XYZ 599 05 05\n"); await sleep(60);
  rx(3); await sleep(60);
  tx("TU WW2DX TEST\n"); await sleep(60);
  rx(4); await sleep(60);
  tx("F5ABC 599 05 05\n"); await sleep(60);
  rx(5); await sleep(60);
  tx("TU WW2DX TEST\n"); await sleep(60);
  rx(6); await sleep(400);

  // --- The operator: K4XYZ into Call from the RX window, two callers onto
  // the stack from the waterfall, and a dupe the stack refuses. ---
  const ctx = (el) => {
    const r = el.getBoundingClientRect();
    el.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, cancelable: true, clientX: r.left + r.width / 2, clientY: r.top + r.height / 2 }));
  };
  const chip = [...document.querySelectorAll(".rx-text .call-chip")].reverse().find((b) => b.textContent.trim() === "K4XYZ");
  if (chip) ctx(chip);
  await sleep(300);
  for (const c of ["UA9XYZ", "PY5ABC", "VE3XYZ"]) {
    const tag = [...document.querySelectorAll(".spot-label")].find((b) => b.textContent.trim() === c);
    if (tag) ctx(tag);
    await sleep(250);
  }
  // Bandmap centred on our frequency, as the app keeps it while tuning.
  const here = document.querySelector(".bandmap-col .here");
  const rows = here?.closest(".rows");
  if (here && rows) rows.scrollTop = here.offsetTop - rows.clientHeight / 2;
  await sleep(200);
  return [...document.querySelectorAll(".next-chip")].map((b) => b.textContent.trim()).join(",");
}
