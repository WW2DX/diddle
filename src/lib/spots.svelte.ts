// Detected callsign spots from the multi-decoder. Each spot has a TTL;
// stale ones are pruned by a background interval.

import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export interface SignalSpot {
  audio_hz: number;
  mark_hz: number;
  space_hz: number;
  call: string;
  timestamp_ms: number;
}

const TTL_MS = 90_000;
const BUCKET_HZ = 50;

class SpotsStore {
  spots = $state<SignalSpot[]>([]);
  slotsActive = $state(0);
  private unlisten: UnlistenFn[] = [];
  private pruneTimer: ReturnType<typeof setInterval> | null = null;

  async init() {
    this.unlisten.push(
      await listen<SignalSpot>("signal:spot", (e) => this.add(e.payload)),
    );
    this.unlisten.push(
      await listen<number>("multi:slots", (e) => (this.slotsActive = e.payload)),
    );
    this.pruneTimer = setInterval(() => this.prune(), 5000);
  }

  destroy() {
    for (const u of this.unlisten) u();
    if (this.pruneTimer) clearInterval(this.pruneTimer);
  }

  /// Add or update a spot. Spots are deduplicated by call + freq bucket so a
  /// signal that keeps re-emitting refreshes its TTL rather than stacking.
  add(spot: SignalSpot) {
    const bucket = Math.round(spot.audio_hz / BUCKET_HZ) * BUCKET_HZ;
    const idx = this.spots.findIndex(
      (s) =>
        s.call === spot.call &&
        Math.round(s.audio_hz / BUCKET_HZ) * BUCKET_HZ === bucket,
    );
    if (idx >= 0) {
      this.spots = this.spots.map((s, i) => (i === idx ? spot : s));
    } else {
      this.spots = [...this.spots, spot];
    }
  }

  prune() {
    const now = Date.now();
    const fresh = this.spots.filter((s) => now - s.timestamp_ms < TTL_MS);
    if (fresh.length !== this.spots.length) {
      this.spots = fresh;
    }
  }

  /// Drop everything (call when VFO changes significantly so labels don't
  /// hang at wrong audio positions).
  clear() {
    this.spots = [];
  }
}

export const spots = new SpotsStore();
