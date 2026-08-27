// Saved contest setups — "call up a previous contest and have the messages
// fill in". A setup snapshots the contest profile, the F-key macros, and the
// call-history file so switching between, say, NAQP and CQ WW is one pick.
//
// While a setup is active, macro/contest/history edits are written back to
// it automatically, so the next time it's loaded it reflects what you
// actually used.

import { settings } from "$lib/settings.svelte";
import { macroState, type Macro } from "$lib/macros.svelte";

export interface ContestSetup {
  id: string;
  name: string;
  contestId: string;
  macros: Macro[];
  historyPath: string;
  updated: number; // unix ms
}

const KEY = "diddle.contestSetups";
const ACTIVE_KEY = "diddle.contestSetups.active";

class ContestSetups {
  setups = $state<ContestSetup[]>([]);
  activeId = $state<string>("");
  loaded = $state(false);

  load() {
    try {
      const raw = localStorage.getItem(KEY);
      if (raw) this.setups = JSON.parse(raw) as ContestSetup[];
      this.activeId = localStorage.getItem(ACTIVE_KEY) || "";
      if (!this.setups.some((s) => s.id === this.activeId)) this.activeId = "";
    } catch (e) {
      console.error("contestSetups.load failed", e);
    }
    this.loaded = true;
  }

  private save() {
    if (!this.loaded) return;
    try {
      localStorage.setItem(KEY, JSON.stringify(this.setups));
      localStorage.setItem(ACTIVE_KEY, this.activeId);
    } catch (e) {
      console.error("contestSetups.save failed", e);
    }
  }

  get active(): ContestSetup | undefined {
    return this.setups.find((s) => s.id === this.activeId);
  }

  private snapshot(): Omit<ContestSetup, "id" | "name"> {
    return {
      contestId: settings.activeContest,
      macros: macroState.macros.map((m) => ({ ...m })),
      historyPath: settings.historyPath,
      updated: Date.now(),
    };
  }

  /// Save the current contest + macros + history file under `name`. If a
  /// setup with that name exists it is overwritten. Becomes the active setup.
  saveAs(name: string): ContestSetup | null {
    const n = name.trim().slice(0, 40);
    if (!n) return null;
    const existing = this.setups.find((s) => s.name.toLowerCase() === n.toLowerCase());
    const setup: ContestSetup = {
      id: existing?.id || crypto.randomUUID(),
      name: existing?.name || n,
      ...this.snapshot(),
    };
    this.setups = existing
      ? this.setups.map((s) => (s.id === setup.id ? setup : s))
      : [...this.setups, setup].sort((a, b) => a.name.localeCompare(b.name));
    this.activeId = setup.id;
    this.save();
    return setup;
  }

  /// Write the live state back into the active setup (called after macro /
  /// contest / history edits).
  syncActive() {
    const a = this.active;
    if (!a) return;
    const snap = this.snapshot();
    const same =
      a.contestId === snap.contestId &&
      a.historyPath === snap.historyPath &&
      JSON.stringify(a.macros) === JSON.stringify(snap.macros);
    if (same) return;
    this.setups = this.setups.map((s) => (s.id === a.id ? { ...s, ...snap } : s));
    this.save();
  }

  /// Apply a saved setup: contest, macros, history path. Returns it so the
  /// caller can (re)load the history file.
  activate(id: string): ContestSetup | null {
    const s = this.setups.find((x) => x.id === id);
    if (!s) return null;
    this.activeId = s.id;
    settings.setActiveContest(s.contestId);
    macroState.replaceAll(s.macros);
    settings.setHistoryPath(s.historyPath || "");
    this.save();
    return s;
  }

  deactivate() {
    this.activeId = "";
    this.save();
  }

  remove(id: string) {
    this.setups = this.setups.filter((s) => s.id !== id);
    if (this.activeId === id) this.activeId = "";
    this.save();
  }
}

export const contestSetups = new ContestSetups();
