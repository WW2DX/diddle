// Shared macro store + send helper. Both the F-keys panel and the ESM
// Enter-handler in EntryWindow read macros from here and call `fire()`.
//
// Macros are user-editable from the Settings panel and persisted to
// localStorage. Slot keys (F1..F8) are fixed; label and text are editable.

import { transmit, txAbort } from "$lib/tci";
import { settings } from "$lib/settings.svelte";
import { qsoLog } from "$lib/qsoLog.svelte";
import { entryBus } from "$lib/entry.svelte";

export interface Macro {
  key: string; // "F1" etc. — slot identifier, not user-editable
  label: string;
  text: string;
}

const DEFAULT_MACROS: Macro[] = [
  { key: "F1", label: "CQ",   text: "CQ CQ CQ DE <MYCALL> <MYCALL> <MYCALL> CQ K" },
  { key: "F2", label: "Excg", text: "<CALL> 599 <SERIAL> 599 <SERIAL>" },
  { key: "F3", label: "TU",   text: "TU 73 DE <MYCALL> CQ" },
  { key: "F4", label: "Call", text: "DE <MYCALL>" },
  { key: "F5", label: "Rpt",  text: "599 <SERIAL> 599 <SERIAL>" },
  { key: "F6", label: "?",    text: "PSE AGN ?" },
  { key: "F7", label: "BRK",  text: "BRK BRK <MYCALL>" },
  { key: "F8", label: "73",   text: "73 DE <MYCALL>" },
];

const STORE_KEY = "diddle.macros";

function clone(ms: Macro[]): Macro[] {
  return ms.map((m) => ({ ...m }));
}

class MacroState {
  macros = $state<Macro[]>(clone(DEFAULT_MACROS));
  txing = $state(false);
  lastSent = $state<string | null>(null);
  lastError = $state<string | null>(null);
  loaded = $state(false);

  load() {
    try {
      const raw = localStorage.getItem(STORE_KEY);
      if (raw) {
        const stored = JSON.parse(raw) as Partial<Macro>[];
        // Merge by slot key so a future release that ships more slots
        // picks up its defaults instead of dropping the new ones.
        this.macros = DEFAULT_MACROS.map((d) => {
          const found = stored.find((s) => s.key === d.key);
          return found
            ? { key: d.key, label: found.label ?? d.label, text: found.text ?? d.text }
            : { ...d };
        });
      }
    } catch (e) {
      console.error("macros.load failed", e);
    }
    this.loaded = true;
  }

  private save() {
    if (!this.loaded) return;
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify(this.macros));
    } catch (e) {
      console.error("macros.save failed", e);
    }
  }

  setLabel(i: number, v: string) {
    if (i < 0 || i >= this.macros.length) return;
    const label = v.slice(0, 10);
    this.macros = this.macros.map((m, j) => (j === i ? { ...m, label } : m));
    this.save();
  }

  setText(i: number, v: string) {
    if (i < 0 || i >= this.macros.length) return;
    this.macros = this.macros.map((m, j) => (j === i ? { ...m, text: v } : m));
    this.save();
  }

  resetOne(i: number) {
    if (i < 0 || i >= DEFAULT_MACROS.length) return;
    const def = { ...DEFAULT_MACROS[i] };
    this.macros = this.macros.map((m, j) => (j === i ? def : m));
    this.save();
  }

  resetAll() {
    this.macros = clone(DEFAULT_MACROS);
    this.save();
  }

  expand(template: string, ctx: { call?: string } = {}): string {
    // Fall back to the entry window's live Call field so macros fired from the
    // F-keys (ESM off, no per-QSO context) still resolve <CALL>.
    const call = ctx.call || entryBus.currentCall || "";
    return template
      .replaceAll("<MYCALL>", settings.myCall || "MYCALL")
      .replaceAll("<CALL>", call)
      .replaceAll("<SERIAL>", String(qsoLog.nextSerial).padStart(3, "0"));
  }

  /// Fire a macro by F-key (`F1`..) or label (`CQ`, `Excg`, ...). Prefer
  /// F-keys so renaming labels doesn't break ESM/Enter behavior.
  async fire(key: string, ctx: { call?: string } = {}): Promise<void> {
    if (this.txing) return;
    const m = this.macros.find((x) => x.key === key || x.label === key);
    if (!m) return;
    const text = this.expand(m.text, ctx);
    if (text.trim().length === 0) {
      this.lastError = `macro ${m.key} (${m.label}) is empty — nothing to send`;
      return;
    }
    this.lastError = null;
    this.lastSent = text;
    this.txing = true;
    try {
      await transmit(text);
    } catch (e: any) {
      this.lastError = String(e);
      console.error("macro fire failed", e);
    } finally {
      this.txing = false;
    }
  }

  /// Abort an in-flight transmission. Safe to call when not TXing.
  async abort(): Promise<void> {
    try {
      await txAbort();
    } catch (e: any) {
      this.lastError = String(e);
      console.error("tx abort failed", e);
    }
  }
}

export const macroState = new MacroState();
