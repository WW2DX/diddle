// Screen layout: which operating panels are folded away, how tall the
// resizable ones are, and a few presets to start from. Persisted to
// localStorage. (The waterfall and RX-window heights live in Settings, next
// to the rest of their display options; presets set them too.)

import { settings, WATERFALL_HEIGHT_DEFAULT, RX_HEIGHT_DEFAULT } from "$lib/settings.svelte";

export type PanelId = "waterfall" | "decoder" | "entry" | "fkeys" | "bandmap" | "score" | "log";

export const PANEL_TITLES: Record<PanelId, string> = {
  waterfall: "Waterfall",
  decoder: "RX decoder",
  entry: "Entry",
  fkeys: "F-keys",
  bandmap: "Bandmap",
  score: "Score",
  log: "Log",
};

const LOG_HEIGHT_DEFAULT = 280;
const BANDMAP_HEIGHT_DEFAULT = 320;

interface Preset {
  label: string;
  hint: string;
  folded: PanelId[];
  waterfall: number;
  rx: number;
  log: number;
  bandmap: number;
}

export const PRESETS: Record<string, Preset> = {
  standard: {
    label: "Standard",
    hint: "Everything open, default sizes",
    folded: [],
    waterfall: WATERFALL_HEIGHT_DEFAULT,
    rx: RX_HEIGHT_DEFAULT,
    log: LOG_HEIGHT_DEFAULT,
    bandmap: BANDMAP_HEIGHT_DEFAULT,
  },
  compact: {
    label: "Compact",
    hint: "Short waterfall, RX window and log; Score folded — the most on one screen",
    folded: ["score"],
    waterfall: 90,
    rx: 120,
    log: 150,
    bandmap: 220,
  },
  decode: {
    label: "Big RX window",
    hint: "A tall RX window for reading copy; Score and Log folded",
    folded: ["score", "log"],
    waterfall: 110,
    rx: 340,
    log: LOG_HEIGHT_DEFAULT,
    bandmap: BANDMAP_HEIGHT_DEFAULT,
  },
};

const KEY = "diddle.layout";

function clampH(px: number): number {
  return Math.round(Math.min(900, Math.max(60, px)));
}

class Layout {
  folded = $state<Record<PanelId, boolean>>({
    waterfall: false,
    decoder: false,
    entry: false,
    fkeys: false,
    bandmap: false,
    score: false,
    log: false,
  });
  logHeight = $state(LOG_HEIGHT_DEFAULT);
  bandmapHeight = $state(BANDMAP_HEIGHT_DEFAULT);
  private loaded = false;

  load() {
    try {
      const raw = localStorage.getItem(KEY);
      if (raw) {
        const o = JSON.parse(raw);
        for (const id of Object.keys(this.folded) as PanelId[]) {
          if (typeof o.folded?.[id] === "boolean") this.folded[id] = o.folded[id];
        }
        if (o.logHeight) this.logHeight = clampH(o.logHeight);
        if (o.bandmapHeight) this.bandmapHeight = clampH(o.bandmapHeight);
      }
    } catch (e) {
      console.error("layout.load failed", e);
    }
    this.loaded = true;
  }

  private save() {
    if (!this.loaded) return;
    try {
      localStorage.setItem(
        KEY,
        JSON.stringify({ folded: this.folded, logHeight: this.logHeight, bandmapHeight: this.bandmapHeight }),
      );
    } catch (e) {
      console.error("layout.save failed", e);
    }
  }

  toggle(id: PanelId) {
    this.folded[id] = !this.folded[id];
    this.save();
  }

  setLogHeight(px: number) {
    const h = clampH(px);
    if (h === this.logHeight) return;
    this.logHeight = h;
    this.save();
  }

  setBandmapHeight(px: number) {
    const h = clampH(px);
    if (h === this.bandmapHeight) return;
    this.bandmapHeight = h;
    this.save();
  }

  apply(id: string) {
    const p = PRESETS[id];
    if (!p) return;
    for (const k of Object.keys(this.folded) as PanelId[]) this.folded[k] = p.folded.includes(k);
    this.logHeight = p.log;
    this.bandmapHeight = p.bandmap;
    settings.setWaterfallHeight(p.waterfall);
    settings.setRxHeight(p.rx);
    this.save();
  }
}

export const layout = new Layout();

/// Remember an element's height when the operator drags its resize corner.
/// The resize handle writes the new height into the inline style in CSS px
/// (unaffected by the UI zoom, unlike measured sizes). Returns a cleanup.
export function watchResize(el: HTMLElement, onHeight: (px: number) => void): () => void {
  let t: ReturnType<typeof setTimeout> | null = null;
  const ro = new ResizeObserver(() => {
    if (t) clearTimeout(t);
    t = setTimeout(() => {
      const h = parseFloat(el.style.height);
      if (Number.isFinite(h)) onHeight(h);
    }, 200);
  });
  ro.observe(el);
  return () => {
    ro.disconnect();
    if (t) clearTimeout(t);
  };
}
