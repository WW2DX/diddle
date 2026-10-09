// Shared macro store + send helper. Both the F-keys panel and the ESM
// Enter-handler in EntryWindow read macros from here and call `fire()`.
//
// Macros are user-editable from the Settings panel and persisted to
// localStorage. Slot keys are fixed — F1..F8 plus Shift+F1..F8 ("SF1"..
// "SF8") — while label and text are editable.

import { transmit, txAbort, txLiveStart, txLivePush, txLiveFinish } from "$lib/tci";
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
  // Stack TU: ESM sends it at the TU step when callers are stacked.
  { key: "F8", label: "TU+Nxt", text: "TU <CALL><LOGIT> NOW<CRLF><POPSTACK><CALL> 599 <SERIAL> 599 <SERIAL>" },
  // Shift+F1..F8.
  { key: "SF1", label: "QRZ",   text: "QRZ? DE <MYCALL>" },
  { key: "SF2", label: "AGN",   text: "AGN AGN" },
  { key: "SF3", label: "Call?", text: "CALL? CALL?" },
  { key: "SF4", label: "Nr?",   text: "NR? NR?" },
  { key: "SF5", label: "Exch?", text: "EXCH? EXCH?" },
  { key: "SF6", label: "QSL",   text: "QSL TU" },
  { key: "SF7", label: "QRL?",  text: "QRL? DE <MYCALL>" },
  { key: "SF8", label: "Test",  text: "<MYCALL> TEST" },
];

/// How a slot key is shown: "SF3" → "⇧F3".
export function keyLabel(key: string): string {
  return key.startsWith("SF") ? `⇧${key.slice(1)}` : key;
}

const STORE_KEY = "diddle.macros";

function clone(ms: Macro[]): Macro[] {
  return ms.map((m) => ({ ...m }));
}

class MacroState {
  macros = $state<Macro[]>(clone(DEFAULT_MACROS));
  // Macros handed to the radio and not yet finished. Several can be in
  // flight: F-keys pressed while one is playing chain onto it.
  private pending = $state(0);
  get txing(): boolean {
    return this.pending > 0;
  }
  /// A live keyboard send (ad-hoc window) is on the air.
  live = $state(false);
  // Pushes and the finish must reach the backend in order.
  private liveChain: Promise<void> = Promise.resolve();

  /// Anything of ours on the air — a macro or a live keyboard send.
  get onAir(): boolean {
    return this.txing || this.live;
  }
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

  /// Write the macros out. Edits already call this as you type; the
  /// F-key Save button calls it too, to confirm. True once written.
  save(): boolean {
    if (!this.loaded) return false;
    try {
      localStorage.setItem(STORE_KEY, JSON.stringify(this.macros));
      return true;
    } catch (e) {
      console.error("macros.save failed", e);
      return false;
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

  /// Replace every slot from a saved set (contest setups). Unknown slots are
  /// ignored; missing ones keep their defaults.
  replaceAll(ms: Macro[]) {
    this.macros = DEFAULT_MACROS.map((d) => {
      const found = ms.find((m) => m.key === d.key);
      return found ? { key: d.key, label: found.label ?? d.label, text: found.text ?? d.text } : { ...d };
    });
    this.save();
  }

  /// Substitute macro tokens. Token names are case-insensitive — `<serial>`
  /// works the same as `<SERIAL>` — and anything unrecognized is left as
  /// typed rather than silently swallowed.
  ///
  /// Tokens are read left to right, so the action tokens change what
  /// follows them:
  ///   <CRLF>      a new line on the other station's screen
  ///   <LOGIT>     log the QSO in the entry form (a later <SERIAL> is the
  ///               next number)
  ///   <POPSTACK>  load the next stacked caller; a later <CALL> is him
  /// Actions only run with `act` — when the text really goes out — so
  /// checking a macro for emptiness or previewing it has no side effects.
  expand(template: string, ctx: { call?: string } = {}, act = false): string {
    // Fall back to the entry window's live Call field so macros fired from the
    // F-keys (ESM off, no per-QSO context) still resolve <CALL>.
    let call = ctx.call || entryBus.currentCall || "";
    return template.replace(/<([A-Za-z]+)>/g, (tok, name: string) => {
      switch (name.toUpperCase()) {
        case "MYCALL":
          return settings.myCall || "MYCALL";
        // The rest of Settings → operator + contest.
        case "NAME":
          return settings.myName;
        case "STATE":
          return settings.myState;
        case "CQZONE":
        case "ZONE":
          return settings.myZone;
        case "GRID":
          // Four characters, the way contests exchange it.
          return settings.myGrid.slice(0, 4);
        case "CALL":
          return call;
        case "SERIAL":
          return String(qsoLog.nextSerial).padStart(3, "0");
        case "CRLF":
          return "\n";
        case "LOGIT":
          if (act) entryBus.actions?.logIt();
          return "";
        case "POPSTACK":
          if (act) call = entryBus.actions?.popStack() ?? "";
          else call = entryBus.nextQueue[0] ?? "";
          return "";
        default:
          return tok;
      }
    });
  }

  /// Send arbitrary text (ad-hoc keyboard send). Runs the same token
  /// expansion as macros and drives the shared txing/lastSent/lastError
  /// state so the TX indicator and ESC-abort behave identically.
  /// Resolves true once the text went out (or joined a live send).
  ///
  /// Never refused for being busy: text sent while we're on the air chains
  /// onto that transmission (the backend queues it). Tokens are expanded
  /// now, synchronously, so callers can change the entry form right after.
  async send(text: string, ctx: { call?: string } = {}): Promise<boolean> {
    const expanded = this.expand(text, ctx, true);
    if (expanded.trim().length === 0) return false;
    this.lastError = null;
    this.lastSent = expanded;
    this.pending++;
    try {
      await transmit(expanded);
      return true;
    } catch (e: any) {
      this.lastError = String(e);
      console.error("send failed", e);
      return false;
    } finally {
      this.pending--;
    }
  }

  /// Fire a macro by F-key (`F1`..) or label (`CQ`, `Excg`, ...). Prefer
  /// F-keys so renaming labels doesn't break ESM/Enter behavior.
  /// Resolves true once the macro went out.
  async fire(key: string, ctx: { call?: string } = {}): Promise<boolean> {
    const m = this.macros.find((x) => x.key === key || x.label === key);
    if (!m) return false;
    if (this.expand(m.text, ctx).trim().length === 0) {
      this.lastError = `macro ${m.key} (${m.label}) is empty — nothing to send`;
      return false;
    }
    return this.send(m.text, ctx);
  }

  /// Key up now and diddle, starting with `text` (may be empty). Further
  /// text goes through `pushLive`; `finishLive` sends what's queued and
  /// unkeys. Ignored while a macro is transmitting.
  startLive(text = "") {
    if (this.live || this.txing) return;
    this.live = true;
    this.lastError = null;
    this.liveChain = Promise.resolve();
    txLiveStart(text)
      .catch((e) => {
        this.lastError = String(e);
        console.error("live tx failed", e);
      })
      .finally(() => {
        this.live = false;
      });
  }

  pushLive(text: string) {
    if (!text) return;
    this.liveChain = this.liveChain.then(() => this.retryWhileLive(() => txLivePush(text)));
  }

  finishLive() {
    this.liveChain = this.liveChain.then(() => this.retryWhileLive(() => txLiveFinish()));
  }

  // The start command may not have published the stream yet (a few ms after
  // the first keystroke) — retry briefly rather than drop text or leave the
  // rig diddling until the idle timeout.
  private async retryWhileLive(op: () => Promise<void>) {
    for (let i = 0; i < 25 && this.live; i++) {
      try {
        await op();
        return;
      } catch {
        await new Promise((r) => setTimeout(r, 20));
      }
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
