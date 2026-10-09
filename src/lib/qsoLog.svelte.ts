// Reactive QSO log + counters. Singleton, imported by EntryWindow,
// Logbook, Header (score). Diddle keeps a named log per contest on disk
// (see log_storage.rs); this holds the open one, and the list of all of
// them for the Logbook's picker.

import { invoke } from "@tauri-apps/api/core";
import type { Qso } from "./types";

export interface LogMeta {
  id: string;
  name: string;
  contestId: string; // "" = not recorded
  setupId: string; // saved contest setup used with it, "" = none
  created: number;
}

export interface LogInfo extends LogMeta {
  count: number;
  firstTs: number | null;
  lastTs: number | null;
}

interface OpenLog {
  meta: LogMeta;
  qsos: Qso[];
}

function nextSerialOf(qsos: Qso[]): number {
  return qsos.length > 0 ? Math.max(...qsos.map((q) => q.serialSent)) + 1 : 1;
}

class QsoLog {
  qsos = $state<Qso[]>([]);
  nextSerial = $state(1);
  loaded = $state(false);
  /// The open log.
  meta = $state<LogMeta | null>(null);
  /// Every log, for the picker (refreshed on open/create/rename/delete).
  logs = $state<LogInfo[]>([]);
  lastError = $state<string | null>(null);
  // Saves go out one after another, so an older write can't land last.
  private saveChain: Promise<void> = Promise.resolve();

  /// Open the log that was open last time.
  async load() {
    try {
      this.apply(await invoke<OpenLog>("log_open", { id: null }));
    } catch (e) {
      console.error("log_open failed", e);
      this.lastError = String(e);
    }
    this.loaded = true;
    await this.refreshLogs();
  }

  async refreshLogs() {
    try {
      const l = await invoke<{ active: string; logs: LogInfo[] }>("log_list");
      this.logs = l.logs;
    } catch (e) {
      console.error("log_list failed", e);
    }
  }

  /// Switch to another log. Pending saves of the current one finish first.
  async open(id: string): Promise<LogMeta | null> {
    if (id === this.meta?.id) return this.meta;
    await this.saveChain;
    try {
      this.apply(await invoke<OpenLog>("log_open", { id }));
      await this.refreshLogs();
      return this.meta;
    } catch (e) {
      this.lastError = String(e);
      return null;
    }
  }

  /// Start a new log (empty, or holding imported QSOs) and open it.
  async create(name: string, contestId: string, setupId = "", qsos: Qso[] = []): Promise<boolean> {
    await this.saveChain;
    try {
      this.apply(await invoke<OpenLog>("log_create", { name, contestId, setupId, qsos }));
      await this.refreshLogs();
      return true;
    } catch (e) {
      this.lastError = String(e);
      return false;
    }
  }

  async rename(id: string, name: string) {
    await this.updateMeta(id, { name });
  }

  /// Record which contest the open log is for (follows the contest picker).
  async setContest(contestId: string) {
    if (!this.meta || this.meta.contestId === contestId) return;
    this.meta = { ...this.meta, contestId };
    await this.updateMeta(this.meta.id, { contestId });
  }

  /// Record which saved setup the open log is used with.
  async setSetup(setupId: string) {
    if (!this.meta || this.meta.setupId === setupId) return;
    this.meta = { ...this.meta, setupId };
    await this.updateMeta(this.meta.id, { setupId });
  }

  private async updateMeta(id: string, patch: { name?: string; contestId?: string; setupId?: string }) {
    try {
      await invoke("log_update_meta", {
        id,
        name: patch.name ?? null,
        contestId: patch.contestId ?? null,
        setupId: patch.setupId ?? null,
      });
      if (this.meta?.id === id && patch.name) this.meta = { ...this.meta, name: patch.name.trim() };
      await this.refreshLogs();
    } catch (e) {
      this.lastError = String(e);
    }
  }

  /// Delete a log other than the open one.
  async removeLog(id: string) {
    try {
      await invoke("log_delete", { id });
      await this.refreshLogs();
    } catch (e) {
      this.lastError = String(e);
    }
  }

  private apply(o: OpenLog) {
    this.meta = o.meta;
    this.qsos = o.qsos;
    this.nextSerial = nextSerialOf(o.qsos);
    this.lastError = null;
  }

  private save() {
    // Fire-and-forget; the UI shouldn't block on disk.
    if (!this.loaded || !this.meta) return; // don't clobber a log before it's read
    const id = this.meta.id;
    const qsos = this.qsos;
    this.saveChain = this.saveChain
      .then(() => invoke<void>("log_save", { id, qsos }))
      .catch((e) => {
        console.error("log_save failed", e);
        this.lastError = String(e);
      });
  }

  add(qso: Qso) {
    this.qsos = [...this.qsos, qso];
    this.nextSerial = this.nextSerial + 1;
    this.save();
  }

  remove(id: string) {
    this.qsos = this.qsos.filter((q) => q.id !== id);
    // Recompute so deleting the most recent QSO (Ctrl-D) frees its serial
    // for the next contact instead of leaving a hole in the sequence.
    this.nextSerial = nextSerialOf(this.qsos);
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
