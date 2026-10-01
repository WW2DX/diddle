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
