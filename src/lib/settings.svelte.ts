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
  decodeHistoryLines?: number;
}

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
  decodeHistoryLines = $state<number>(HISTORY_DEFAULT);
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
        if (obj.decodeHistoryLines !== undefined) {
          this.decodeHistoryLines = this.clampHistory(obj.decodeHistoryLines);
        }
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
          decodeHistoryLines: this.decodeHistoryLines,
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

  private clampHistory(v: number): number {
    if (!Number.isFinite(v)) return HISTORY_DEFAULT;
    return Math.min(HISTORY_MAX, Math.max(HISTORY_MIN, Math.round(v)));
  }

  setDecodeHistoryLines(v: number) {
    this.decodeHistoryLines = this.clampHistory(v);
    this.save();
  }
}

export const settings = new Settings();
