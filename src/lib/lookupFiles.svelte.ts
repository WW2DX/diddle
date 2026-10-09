// The lookup files Diddle loads into the backend: the SCP database (MASTER.SCP)
// and the N1MM-style call history file. Their state lives here rather than
// in the Settings panel so they're reloaded at startup even while Settings
// stays collapsed, and so switching a saved contest setup (from Settings or
// from New log) brings its history file with it.

import {
  scpAutoDownload,
  scpLoadFile,
  scpStatus,
  historyLoadFile,
  historyClear,
  historyStatus,
  type ScpStatus,
  type CallHistoryStatus,
} from "$lib/tci";
import { settings } from "$lib/settings.svelte";

class ScpFile {
  status = $state<ScpStatus>({ count: 0, source: "" });
  loading = $state(false);
  error = $state<string | null>(null);

  /// Startup: the saved file again, or on the very first launch MASTER.SCP
  /// from supercheckpartial.com, so there's a full callsign database
  /// without finding and downloading it by hand.
  async init() {
    try {
      this.status = await scpStatus();
    } catch (e) {
      console.error("scpStatus failed", e);
    }
    if (this.status.source !== "starter") return;
    if (settings.scpPath) await this.load(settings.scpPath);
    else await this.autoDownload();
  }

  async autoDownload() {
    this.loading = true;
    this.error = null;
    try {
      const result = await scpAutoDownload();
      this.status = result.status;
      settings.setScpPath(result.path);
    } catch (e) {
      this.error = `Auto-download failed (using starter list): ${e}`;
    } finally {
      this.loading = false;
    }
  }

  async load(path: string) {
    this.loading = true;
    this.error = null;
    try {
      this.status = await scpLoadFile(path);
      settings.setScpPath(path);
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }
}

class HistoryFile {
  status = $state<CallHistoryStatus>({ count: 0, path: "", fields: [] });
  loading = $state(false);
  error = $state<string | null>(null);

  async init() {
    try {
      this.status = await historyStatus();
    } catch (e) {
      console.error("historyStatus failed", e);
    }
    await this.sync();
  }

  async load(path: string) {
    this.loading = true;
    this.error = null;
    try {
      this.status = await historyLoadFile(path);
      settings.setHistoryPath(path);
    } catch (e) {
      this.error = String(e);
    } finally {
      this.loading = false;
    }
  }

  async clear() {
    this.error = null;
    try {
      this.status = await historyClear();
      settings.setHistoryPath("");
    } catch (e) {
      this.error = String(e);
    }
  }

  /// Make the loaded file match the one Settings names (after a saved
  /// setup was switched in, say).
  async sync() {
    const want = settings.historyPath;
    if (want && (want !== this.status.path || this.status.count === 0)) await this.load(want);
    else if (!want && this.status.count > 0) await this.clear();
  }
}

export const scpFile = new ScpFile();
export const historyFile = new HistoryFile();
