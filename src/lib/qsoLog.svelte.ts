// Reactive QSO log + counters. Singleton, imported by EntryWindow,
// Logbook, Header (score). Persists to JSON on disk via the backend.

import { invoke } from "@tauri-apps/api/core";
import type { Qso } from "./types";

class QsoLog {
  qsos = $state<Qso[]>([]);
  nextSerial = $state(1);
  loaded = $state(false);

  async load() {
    try {
      const stored = await invoke<Qso[]>("load_log");
      this.qsos = stored;
      this.nextSerial =
        stored.length > 0
          ? Math.max(...stored.map((q) => q.serialSent)) + 1
          : 1;
    } catch (e) {
      console.error("load_log failed", e);
    }
    this.loaded = true;
  }

  private save() {
    // Fire-and-forget; the UI shouldn't block on disk.
    if (!this.loaded) return; // don't clobber existing file before initial load
    invoke("save_log", { qsos: this.qsos }).catch((e) =>
      console.error("save_log failed", e),
    );
  }

  add(qso: Qso) {
    this.qsos = [...this.qsos, qso];
    this.nextSerial = this.nextSerial + 1;
    this.save();
  }

  remove(id: string) {
    this.qsos = this.qsos.filter((q) => q.id !== id);
    this.save();
  }

  update(id: string, patch: Partial<Qso>) {
    let changed = false;
    this.qsos = this.qsos.map((q) => {
      if (q.id !== id) return q;
      changed = true;
      return { ...q, ...patch };
    });
    if (changed) this.save();
  }

  clear() {
    this.qsos = [];
    this.nextSerial = 1;
    this.save();
  }

  isDupe(call: string, band: string): boolean {
    if (!call || !band || band === "—") return false;
    const c = call.toUpperCase();
    return this.qsos.some((q) => q.call === c && q.band === band);
  }

  // QSOs per hour, based on QSOs in the last 10 minutes × 6.
  get ratePerHour(): number {
    const cutoff = Date.now() - 10 * 60 * 1000;
    return this.qsos.filter((q) => q.ts >= cutoff).length * 6;
  }
}

export const qsoLog = new QsoLog();
