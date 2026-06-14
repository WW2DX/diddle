// DX cluster connection state + spot stream. Owns the live list of cluster
// spots (TTL-pruned) and the raw line log for the cluster console.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type ClusterStateMsg =
  | { kind: "disconnected" }
  | { kind: "connecting"; host: string; port: number }
  | { kind: "connected"; host: string; port: number }
  | { kind: "error"; message: string };

export interface ClusterSpot {
  source: string;
  dx_call: string;
  freq_hz: number;
  band: string;
  comment: string;
  time_utc: string;
  timestamp_ms: number;
}

export interface ClusterLine {
  dir: "rx" | "tx";
  text: string;
}

const SPOT_TTL_MS = 30 * 60 * 1000; // 30 minutes — cluster convention
const MAX_LINES = 500;

class ClusterStore {
  state = $state<ClusterStateMsg>({ kind: "disconnected" });
  spots = $state<ClusterSpot[]>([]);
  lines = $state<ClusterLine[]>([]);
  private unlisten: UnlistenFn[] = [];
  private pruneTimer: ReturnType<typeof setInterval> | null = null;

  async init() {
    try {
      this.state = await invoke<ClusterStateMsg>("cluster_status");
    } catch (e) {
      console.error("cluster_status failed", e);
    }
    this.unlisten.push(
      await listen<ClusterStateMsg>("cluster:state", (e) => (this.state = e.payload)),
    );
    this.unlisten.push(
      await listen<ClusterSpot>("cluster:spot", (e) => this.addSpot(e.payload)),
    );
    this.unlisten.push(
      await listen<ClusterLine>("cluster:line", (e) => this.addLine(e.payload)),
    );
    this.pruneTimer = setInterval(() => this.prune(), 30_000);
  }

  destroy() {
    for (const u of this.unlisten) u();
    if (this.pruneTimer) clearInterval(this.pruneTimer);
  }

  async connect(host: string, port: number, login: string) {
    await invoke("cluster_connect", { host, port, login });
  }

  async disconnect() {
    await invoke("cluster_disconnect");
  }

  async send(line: string) {
    await invoke("cluster_send", { line });
  }

  private addSpot(spot: ClusterSpot) {
    const i = this.spots.findIndex((s) => s.dx_call === spot.dx_call);
    if (i >= 0) {
      this.spots = this.spots.map((s, j) => (j === i ? spot : s));
    } else {
      this.spots = [...this.spots, spot];
    }
  }

  private addLine(line: ClusterLine) {
    this.lines = [...this.lines, line].slice(-MAX_LINES);
  }

  prune() {
    const now = Date.now();
    const fresh = this.spots.filter((s) => now - s.timestamp_ms < SPOT_TTL_MS);
    if (fresh.length !== this.spots.length) this.spots = fresh;
  }

  clearLines() {
    this.lines = [];
  }
}

export const cluster = new ClusterStore();
