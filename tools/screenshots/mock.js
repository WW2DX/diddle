// Stand-in Tauri backend for README screenshots: answers Diddle's IPC calls
// with a plausible contest state and lets the scene emit backend events.
// Injected before the app loads (CDP addScriptToEvaluateOnNewDocument).
(() => {
  const callbacks = new Map();
  const listeners = {};
  let next = 1;
  const now = Date.now();
  const RIG = {
    freq: 14_087_000, mode: "digl", ptt: false,
    filter_lo: -3000, filter_hi: -200, agc_mode: "normal", agc_gain: 62, drive: 60,
  };

  // ~45 CQ WW RTTY QSOs over the last 1¼ hours (placeholder-style calls).
  const RAW = [
    ["DL1ABC", "20m", "599", "14"], ["JA1XYZ", "20m", "599", "25"], ["K1ABC", "20m", "599", "05 MA"],
    ["UA3XYZ", "20m", "599", "16"], ["EA5ABC", "20m", "599", "14"], ["VE3XYZ", "20m", "599", "04 ON"],
    ["G4ABC", "20m", "599", "14"], ["I2XYZ", "20m", "599", "15"], ["PY2ABC", "20m", "599", "11"],
    ["W6XYZ", "20m", "599", "03 CA"], ["OH2ABC", "20m", "599", "15"], ["SP5XYZ", "20m", "599", "15"],
    ["LU1ABC", "20m", "599", "13"], ["ZL2XYZ", "20m", "599", "32"], ["VK3ABC", "20m", "599", "30"],
    ["OK1XYZ", "20m", "599", "15"], ["S51ABC", "20m", "599", "15"], ["HA5XYZ", "20m", "599", "15"],
    ["N8ABC", "20m", "599", "04 OH"], ["K0XYZ", "20m", "599", "04 CO"], ["YO3ABC", "20m", "599", "20"],
    ["LZ1XYZ", "20m", "599", "20"], ["UR5ABC", "20m", "599", "16"], ["EI7XYZ", "20m", "599", "14"],
    ["CT1ABC", "20m", "599", "14"], ["9A2XYZ", "20m", "599", "15"],
    ["DL5XYZ", "40m", "599", "14"], ["W4ABC", "40m", "599", "05 GA"], ["F6XYZ", "40m", "599", "14"],
    ["K9ABC", "40m", "599", "04 IL"], ["VE2XYZ", "40m", "599", "02 QC"], ["ON4ABC", "40m", "599", "14"],
    ["PA3XYZ", "40m", "599", "14"], ["HB9ABC", "40m", "599", "14"],
    ["JH1XYZ", "15m", "599", "25"], ["BV2ABC", "15m", "599", "24"], ["ZS6XYZ", "15m", "599", "38"],
    ["KH6ABC", "15m", "599", "31"], ["HK3XYZ", "15m", "599", "09"], ["CE3ABC", "15m", "599", "12"],
    ["XE2XYZ", "15m", "599", "06"], ["4X4ABC", "15m", "599", "20"], ["A61XYZ", "15m", "599", "21"],
    ["VU2ABC", "15m", "599", "22"],
  ];
  const BASE = { "20m": 14_070_000, "40m": 7_082_000, "15m": 21_084_000 };
  const order = ["40m", "20m", "15m"];
  const sorted = [...RAW].sort((a, b) => order.indexOf(a[1]) - order.indexOf(b[1]));
  const QSOS = sorted.map(([call, band, rst, ex], i) => ({
    id: `q${i}`,
    ts: now - (sorted.length - i) * 95_000,
    call, band, freqHz: BASE[band] + ((i * 397) % 6000), mode: "DIGL",
    rstSent: "599", rstRcvd: rst, exchSent: "05", exchRcvd: ex, serialSent: i + 1,
  }));
  const META = { id: "log-1", name: "CQ WW RTTY 2026", contestId: "cqww-rtty", setupId: "", created: now - 86_400_000 };
  const LOGS = [
    { ...META, count: QSOS.length, firstTs: QSOS[0].ts, lastTs: QSOS[QSOS.length - 1].ts },
    { id: "log-0", name: "JARL World Wide RTTY 2026-10-17", contestId: "jarl-rtty", setupId: "", created: now - 9 * 86_400_000, count: 212, firstTs: now - 9 * 86_400_000, lastTs: now - 8 * 86_400_000 },
    { id: "log-00", name: "Makrothen RTTY 2026-10-10", contestId: "makrothen-rtty", setupId: "", created: now - 16 * 86_400_000, count: 63, firstTs: now - 16 * 86_400_000, lastTs: now - 15 * 86_400_000 },
  ];
  const SCP = ["K4XYZ", "K4XYB", "K4XYP", "KH6ABC", "DL1ABC", "JA1XYZ", "F5ABC", "VE3XYZ", "OH2ABC"];

  function answer(cmd, args) {
    switch (cmd) {
      case "plugin:app|version": return "0.1.35";
      case "tci_status": return [{ kind: "connected", url: "ws://127.0.0.1:40001", ready: true }, RIG];
      case "get_rtty_config": return { mark_hz: 2125, space_hz: 2295, baud: 45.45, tx_mark_hz: 2125, tx_space_hz: 2295, squelch: 30 };
      case "log_open": return { meta: META, qsos: QSOS };
      case "log_list": return { active: META.id, logs: LOGS };
      case "cty_cached": return null;
      case "scp_status": return { count: 41_237, source: "file:MASTER.SCP" };
      case "scp_auto_download": return { status: { count: 41_237, source: "file:MASTER.SCP" }, path: "MASTER.SCP" };
      case "scp_search": return SCP.filter((c) => c.includes(String(args.query).toUpperCase())).slice(0, args.max ?? 8);
      case "scp_contains_any": return true;
      case "history_status": return { count: 12_873, path: "CQWW-RTTY.txt", fields: ["Name", "State", "CQZone"] };
      case "history_lookup": return null;
      case "cluster_status": return { kind: "connected", host: "dxc.k1ttt.net", port: 7373 };
      case "deep_rtty_status": return { state: "on", message: "listening" };
      case "sim_status": return { running: false, qso_count: 0, callers: 0 };
      case "wav_status": return { kind: "idle" };
      case "audio_input_status": return { kind: "idle" };
      case "audio_devices": return [];
      case "list_fonts": return [{ name: "Menlo", mono: true }, { name: "Courier New", mono: true }];
      default: return null;
    }
  }

  window.__TAURI_EVENT_PLUGIN_INTERNALS__ = { unregisterListener() {} };
  window.__TAURI_INTERNALS__ = {
    metadata: { currentWindow: { label: "main" }, currentWebview: { label: "main", windowLabel: "main" } },
    transformCallback(cb) { const id = next++; callbacks.set(id, cb); return id; },
    unregisterCallback(id) { callbacks.delete(id); },
    convertFileSrc: (p) => p,
    async invoke(cmd, args = {}) {
      if (cmd === "plugin:event|listen") {
        (listeners[args.event] ||= []).push(args.handler);
        return args.handler;
      }
      if (cmd === "plugin:event|unlisten") return null;
      return answer(cmd, args);
    },
  };
  window.__emit = (event, payload) =>
    (listeners[event] || []).forEach((id) => callbacks.get(id)?.({ event, id: 0, payload }));
  window.__RIG = RIG;
  window.__NOW = now;

  // Settings the scene runs with (first load only).
  try {
    if (!localStorage.getItem("diddle.settings")) {
      localStorage.setItem("diddle.settings", JSON.stringify({
        myCall: "WW2DX", myZone: "05", activeContest: "cqww-rtty", esm: true, spMode: false,
        bandmapSide: true, deepRtty: true, squelch: 30, txLevel: 45, rxHeight: 215,
      }));
    }
  } catch {}
})();
