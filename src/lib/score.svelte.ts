// Live contest score and multiplier tracking for the active contest, from
// the QSO log and the country file. Recomputed when either changes.

import { qsoLog } from "./qsoLog.svelte";
import { settings } from "./settings.svelte";
import { cty } from "./ctyStore.svelte";
import { entryBus } from "./entry.svelte";
import { MultTracker, neededMults, qsoRate, scoreLog, type NeededMults, type Rate, type Score } from "./scoring";

class ScoreStore {
  private lookup = (c: string) => cty.lookup(c);

  /// My own country hit (for QSO points), from the call in Settings.
  my = $derived.by(() => (cty.db ? cty.lookup(settings.myCall) : null));

  score = $derived.by<Score>(() => {
    cty.db; // recompute when the country file changes
    return scoreLog(settings.activeContest, qsoLog.qsos, this.lookup, this.my, {
      myGrid: settings.myGrid,
    });
  });

  /// What's still needed on the band the entry form is on.
  needed = $derived.by<NeededMults[]>(() => {
    cty.db;
    const band = entryBus.currentBand;
    if (!band) return [];
    return neededMults(settings.activeContest, qsoLog.qsos, this.lookup, this.my, band, {
      myGrid: settings.myGrid,
    });
  });

  // Ticks so the rate falls off as QSOs age out of the window.
  now = $state(Date.now());
  constructor() {
    setInterval(() => (this.now = Date.now()), 15_000);
  }

  rate = $derived.by<Rate>(() => qsoRate(qsoLog.qsos.map((q) => q.ts), this.now));

  // Everything logged so far, for "would this be a new mult?" questions.
  private tracker = $derived.by(() => {
    cty.db;
    const t = new MultTracker(settings.activeContest, this.lookup, this.my, {
      myGrid: settings.myGrid,
    });
    for (const q of qsoLog.qsos) t.add(q);
    return t;
  });

  /// Multipliers a QSO with `call` on `band` would add right now (empty if
  /// none, a dupe, or no country file). `exch` refines it (state, zone).
  newMults(call: string, band: string, exch = ""): string[] {
    if (!call || !band || !cty.db) return [];
    const q = { call, band, exchRcvd: exch };
    const t = this.tracker;
    if (t.isDupe(q)) return [];
    return t.newMults(q);
  }

  isNewMult(call: string, band: string): boolean {
    return this.newMults(call, band).length > 0;
  }
}

export const scoreStore = new ScoreStore();
