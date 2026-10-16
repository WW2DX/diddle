import { DEFAULT_TCI_URL } from "$lib/tci";
// Operator settings store, persisted to localStorage. Holds station-identity
// info that contests need (call, name, state, zone, grid) plus the active
// contest profile id.

const KEY = "diddle.settings";

interface Stored {
  myCall?: string;
  myName?: string;
  myState?: string; // state / province / country abbrev
  myZone?: string; // CQ zone
  myGrid?: string; // Maidenhead grid
  myAge?: string; // operator age (JARL RTTY exchange); "" = send 01
  activeContest?: string;
  scpPath?: string;
  clusterHost?: string;
  clusterPort?: number;
  esm?: boolean;
  spMode?: boolean; // false = Run, true = Search & Pounce
  decodeHistoryLines?: number;
  clusterLoginCommands?: string; // one per line, sent after login
  bandmapAllBands?: boolean; // false = current band only
  bandmapSide?: boolean; // true = docked column beside the operating panels
  historyPath?: string; // N1MM-style call history file
  tciUrl?: string; // last TCI server address that connected
  fonts?: Partial<Record<FontWin, WinFont>>;
  multBell?: boolean; // chime on a new multiplier
  bandmapWidth?: number;
  stackTuKey?: string; // macro ESM sends at the TU step with callers stacked
  squelch?: number; // RX-window decoder squelch, 0 (open) – 100
  historyField?: string; // history column that pre-fills Exch; "" = auto
  deepRtty?: boolean; // DeepRTTY neural decoder pane in the RX window
  waterfallHeight?: number; // px
  rxHeight?: number; // RX decoder text, px
  waterfallRf?: boolean; // waterfall RF-up left→right, like the radio's spectrum
  txLevel?: number; // TX audio, % of full scale
  udpWsjtx?: boolean; // broadcast QSOs in WSJT-X UDP format
  udpWsjtxAddrs?: string;
  udpN1mm?: boolean; // broadcast QSOs as N1MM contactinfo XML
  udpN1mmAddrs?: string;
}

/// Windows whose text font the operator can pick.
export type FontWin = "decoder" | "entry" | "bandmap" | "log";
export const FONT_WINS: { id: FontWin; label: string; defaultSize: number }[] = [
  { id: "decoder", label: "RX decoder", defaultSize: 14 },
  { id: "entry", label: "Entry", defaultSize: 18 },
  { id: "bandmap", label: "Bandmap", defaultSize: 12 },
  { id: "log", label: "Log", defaultSize: 12 },
];
/// family "" and size 0 mean the window's built-in default.
export interface WinFont {
  family: string;
  size: number;
  slashedZero: boolean;
}
const NO_FONT: WinFont = { family: "", size: 0, slashedZero: false };
export const BANDMAP_WIDTH_MIN = 300;
// Waterfall and RX text start compact (they used to be 260 / 220 px) so
// more of the operating panels fit on screen; both can be dragged taller.
export const WATERFALL_HEIGHT_DEFAULT = 140;
export const RX_HEIGHT_DEFAULT = 165;
function clampH(px: number): number {
  return Math.round(Math.min(900, Math.max(60, px)));
}
export const BANDMAP_WIDTH_MAX = 900;

// Bounds for how many decoded lines the RX window keeps before old lines
// scroll off for good.
export const HISTORY_MIN = 100;
export const HISTORY_MAX = 50_000;
export const HISTORY_DEFAULT = 1000;

class Settings {
  myCall = $state<string>("");
  myName = $state<string>("");
  myState = $state<string>("");
  myZone = $state<string>("");
  myGrid = $state<string>("");
  myAge = $state<string>("");
  activeContest = $state<string>("generic");
  scpPath = $state<string>("");
  clusterHost = $state<string>("dxc.k1ttt.net");
  clusterPort = $state<number>(7373);
  esm = $state<boolean>(true);
  spMode = $state<boolean>(false);
  decodeHistoryLines = $state<number>(HISTORY_DEFAULT);
  clusterLoginCommands = $state<string>("");
  bandmapAllBands = $state<boolean>(false);
  bandmapSide = $state<boolean>(true);
  historyPath = $state<string>("");
  tciUrl = $state<string>(DEFAULT_TCI_URL);
  fonts = $state<Record<FontWin, WinFont>>({
    decoder: { ...NO_FONT },
    entry: { ...NO_FONT },
    bandmap: { ...NO_FONT },
    log: { ...NO_FONT },
  });
  bandmapWidth = $state<number>(440);
  multBell = $state<boolean>(true);
  stackTuKey = $state<string>("F8");
  squelch = $state<number>(30);
  historyField = $state<string>("");
  deepRtty = $state<boolean>(false);
  waterfallHeight = $state<number>(WATERFALL_HEIGHT_DEFAULT);
  rxHeight = $state<number>(RX_HEIGHT_DEFAULT);
  waterfallRf = $state<boolean>(true);
  txLevel = $state<number>(60);
  udpWsjtx = $state<boolean>(false);
  udpWsjtxAddrs = $state<string>("127.0.0.1:2237");
  udpN1mm = $state<boolean>(false);
  udpN1mmAddrs = $state<string>("127.0.0.1:12060");
  loaded = $state(false);

  load() {
    try {
      const raw = localStorage.getItem(KEY);
      if (raw) {
        const obj: Stored = JSON.parse(raw);
        this.myCall = (obj.myCall || "").toUpperCase();
        this.myName = obj.myName || "";
        this.myState = (obj.myState || "").toUpperCase();
        this.myZone = (obj.myZone || "").toUpperCase();
        this.myGrid = (obj.myGrid || "").toUpperCase();
        this.myAge = obj.myAge || "";
        this.activeContest = obj.activeContest || "generic";
        this.scpPath = obj.scpPath || "";
        if (obj.clusterHost) this.clusterHost = obj.clusterHost;
        if (obj.clusterPort) this.clusterPort = obj.clusterPort;
        if (obj.esm !== undefined) this.esm = obj.esm;
        if (obj.spMode !== undefined) this.spMode = obj.spMode;
        if (obj.decodeHistoryLines !== undefined) {
          this.decodeHistoryLines = this.clampHistory(obj.decodeHistoryLines);
        }
        this.clusterLoginCommands = obj.clusterLoginCommands || "";
        if (obj.bandmapAllBands !== undefined) this.bandmapAllBands = obj.bandmapAllBands;
        if (obj.bandmapSide !== undefined) this.bandmapSide = obj.bandmapSide;
        this.historyPath = obj.historyPath || "";
        if (obj.tciUrl) this.tciUrl = obj.tciUrl;
        if (obj.fonts) {
          for (const w of FONT_WINS) {
            const f = obj.fonts[w.id];
            if (f) this.fonts[w.id] = { ...NO_FONT, ...f };
          }
        }
        if (obj.bandmapWidth) this.bandmapWidth = this.clampWidth(obj.bandmapWidth);
        if (obj.multBell !== undefined) this.multBell = obj.multBell;
        if (obj.stackTuKey) this.stackTuKey = obj.stackTuKey;
        this.historyField = obj.historyField || "";
        if (obj.deepRtty !== undefined) this.deepRtty = obj.deepRtty;
        if (obj.waterfallHeight) this.waterfallHeight = clampH(obj.waterfallHeight);
        if (obj.rxHeight) this.rxHeight = clampH(obj.rxHeight);
        if (obj.waterfallRf !== undefined) this.waterfallRf = obj.waterfallRf;
        if (obj.txLevel !== undefined) this.txLevel = Math.min(100, Math.max(1, obj.txLevel));
        if (obj.udpWsjtx !== undefined) this.udpWsjtx = obj.udpWsjtx;
        if (obj.udpWsjtxAddrs) this.udpWsjtxAddrs = obj.udpWsjtxAddrs;
        if (obj.udpN1mm !== undefined) this.udpN1mm = obj.udpN1mm;
        if (obj.udpN1mmAddrs) this.udpN1mmAddrs = obj.udpN1mmAddrs;
        if (obj.squelch !== undefined) this.squelch = Math.min(100, Math.max(0, obj.squelch));
      }
    } catch (e) {
      console.error("settings.load failed", e);
    }
    this.loaded = true;
  }

  private save() {
    try {
      localStorage.setItem(
        KEY,
        JSON.stringify({
          myCall: this.myCall,
          myName: this.myName,
          myState: this.myState,
          myZone: this.myZone,
          myGrid: this.myGrid,
          myAge: this.myAge,
          activeContest: this.activeContest,
          scpPath: this.scpPath,
          clusterHost: this.clusterHost,
          clusterPort: this.clusterPort,
          esm: this.esm,
          spMode: this.spMode,
          decodeHistoryLines: this.decodeHistoryLines,
          clusterLoginCommands: this.clusterLoginCommands,
          bandmapAllBands: this.bandmapAllBands,
          bandmapSide: this.bandmapSide,
          historyPath: this.historyPath,
          tciUrl: this.tciUrl,
          fonts: this.fonts,
          bandmapWidth: this.bandmapWidth,
          multBell: this.multBell,
          stackTuKey: this.stackTuKey,
          squelch: this.squelch,
          historyField: this.historyField,
          deepRtty: this.deepRtty,
          waterfallHeight: this.waterfallHeight,
          rxHeight: this.rxHeight,
          waterfallRf: this.waterfallRf,
          txLevel: this.txLevel,
          udpWsjtx: this.udpWsjtx,
          udpWsjtxAddrs: this.udpWsjtxAddrs,
          udpN1mm: this.udpN1mm,
          udpN1mmAddrs: this.udpN1mmAddrs,
        } satisfies Stored),
      );
    } catch (e) {
      console.error("settings.save failed", e);
    }
  }

  private normCall(s: string): string {
    return s
      .toUpperCase()
      .replace(/[^A-Z0-9/]/g, "")
      .slice(0, 12);
  }

  setMyCall(v: string) {
    this.myCall = this.normCall(v);
    this.save();
  }
  setMyName(v: string) {
    this.myName = v.toUpperCase().replace(/[^A-Z ]/g, "").slice(0, 12);
    this.save();
  }
  setMyState(v: string) {
    this.myState = v.toUpperCase().replace(/[^A-Z]/g, "").slice(0, 4);
    this.save();
  }
  setMyZone(v: string) {
    this.myZone = v.replace(/[^0-9]/g, "").slice(0, 2);
    this.save();
  }
  /// Two digits; "" sends 01, the substitute JARL accepts in place of an age.
  setMyAge(v: string) {
    this.myAge = v.replace(/\D/g, "").slice(0, 2);
    this.save();
  }
  /// The age as sent: two digits, 01 when not given.
  get sentAge(): string {
    return this.myAge ? this.myAge.padStart(2, "0") : "01";
  }
  setMyGrid(v: string) {
    this.myGrid = v.toUpperCase().replace(/[^A-Z0-9]/g, "").slice(0, 6);
    this.save();
  }
  setActiveContest(id: string) {
    this.activeContest = id;
    this.save();
  }
  setScpPath(v: string) {
    this.scpPath = v;
    this.save();
  }
  setClusterHost(v: string) {
    this.clusterHost = v.trim().toLowerCase();
    this.save();
  }
  setClusterPort(v: number) {
    if (Number.isFinite(v) && v > 0 && v < 65536) {
      this.clusterPort = Math.round(v);
      this.save();
    }
  }
  setEsm(v: boolean) {
    this.esm = v;
    this.save();
  }
  setSpMode(v: boolean) {
    this.spMode = v;
    this.save();
  }
  toggleSpMode() {
    this.setSpMode(!this.spMode);
  }

  private clampHistory(v: number): number {
    if (!Number.isFinite(v)) return HISTORY_DEFAULT;
    return Math.min(HISTORY_MAX, Math.max(HISTORY_MIN, Math.round(v)));
  }

  setDecodeHistoryLines(v: number) {
    this.decodeHistoryLines = this.clampHistory(v);
    this.save();
  }

  setClusterLoginCommands(v: string) {
    this.clusterLoginCommands = v;
    this.save();
  }
  /// Login commands as a list — one per line, blanks and #comments dropped.
  get clusterLoginCommandList(): string[] {
    return this.clusterLoginCommands
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter((l) => l.length > 0 && !l.startsWith("#"));
  }
  setBandmapAllBands(v: boolean) {
    this.bandmapAllBands = v;
    this.save();
  }
  toggleBandmapAllBands() {
    this.setBandmapAllBands(!this.bandmapAllBands);
  }
  toggleBandmapSide() {
    this.bandmapSide = !this.bandmapSide;
    this.save();
  }
  /// `persist` false while the slider is being dragged.
  setSquelch(v: number, persist = true) {
    this.squelch = Math.round(Math.min(100, Math.max(0, v)));
    if (persist) this.save();
  }
  /// Panel heights, from their resize handles.
  setWaterfallHeight(px: number) {
    const h = clampH(px);
    if (h === this.waterfallHeight) return;
    this.waterfallHeight = h;
    this.save();
  }
  setRxHeight(px: number) {
    const h = clampH(px);
    if (h === this.rxHeight) return;
    this.rxHeight = h;
    this.save();
  }
  /// `persist` false while the slider is being dragged.
  setTxLevel(v: number, persist = true) {
    this.txLevel = Math.round(Math.min(100, Math.max(1, v)));
    if (persist) this.save();
  }
  /// UDP QSO broadcast: which format, and the host:port list it goes to.
  setUdp(patch: { wsjtx?: boolean; wsjtxAddrs?: string; n1mm?: boolean; n1mmAddrs?: string }) {
    if (patch.wsjtx !== undefined) this.udpWsjtx = patch.wsjtx;
    if (patch.wsjtxAddrs !== undefined) this.udpWsjtxAddrs = patch.wsjtxAddrs.trim();
    if (patch.n1mm !== undefined) this.udpN1mm = patch.n1mm;
    if (patch.n1mmAddrs !== undefined) this.udpN1mmAddrs = patch.n1mmAddrs.trim();
    this.save();
  }
  setWaterfallRf(v: boolean) {
    this.waterfallRf = v;
    this.save();
  }
  setDeepRtty(v: boolean) {
    this.deepRtty = v;
    this.save();
  }
  setHistoryField(v: string) {
    this.historyField = v;
    this.save();
  }
  setStackTuKey(v: string) {
    this.stackTuKey = v;
    this.save();
  }
  setMultBell(v: boolean) {
    this.multBell = v;
    this.save();
  }

  setFont(win: FontWin, patch: Partial<WinFont>) {
    this.fonts[win] = { ...this.fonts[win], ...patch };
    this.save();
  }

  /// CSS custom properties for a window's text: --win-font, --win-size and
  /// --win-zero, consumed with var(…, built-in default) in its styles.
  fontStyle(win: FontWin): string {
    const f = this.fonts[win];
    const parts: string[] = [];
    if (f.family.trim()) {
      const fam = f.family.trim().replace(/["\\;{}]/g, "");
      parts.push(`--win-font: "${fam}", ui-monospace, Menlo, monospace`);
    }
    if (f.size > 0) parts.push(`--win-size: ${f.size}px`);
    if (f.slashedZero) parts.push("--win-zero: slashed-zero");
    return parts.join("; ");
  }

  private clampWidth(w: number): number {
    return Math.round(Math.min(BANDMAP_WIDTH_MAX, Math.max(BANDMAP_WIDTH_MIN, w)));
  }

  /// Docked bandmap column width; `persist` false while dragging.
  setBandmapWidth(w: number, persist = true) {
    this.bandmapWidth = this.clampWidth(w);
    if (persist) this.save();
  }

  /// Remember the TCI server address — called once it has connected, so a
  /// mistyped address never replaces a working one.
  setTciUrl(v: string) {
    const u = v.trim();
    if (!u || u === this.tciUrl) return;
    this.tciUrl = u;
    this.save();
  }
  setHistoryPath(v: string) {
    this.historyPath = v;
    this.save();
  }
}

export const settings = new Settings();
