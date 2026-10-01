// The loaded country file. Prefers the operator's replacement (downloaded
// or loaded from disk, kept by the backend); otherwise the copy of cty.dat
// bundled with Diddle.

import { invoke } from "@tauri-apps/api/core";
import { CtyDb, type CtyHit } from "./cty";

class CtyStore {
  db = $state.raw<CtyDb | null>(null);
  source = $state<"bundled" | "custom" | "">("");
  error = $state<string | null>(null);
  busy = $state(false);
  private memo = new Map<string, CtyHit | null>();

  async load() {
    this.error = null;
    try {
      const custom = await invoke<string | null>("cty_cached");
      if (custom) return this.use(custom, "custom");
    } catch {
      // Not running under Tauri (or no custom file) — fall through.
    }
    try {
      const r = await fetch("/cty.dat");
      if (!r.ok) throw new Error(`HTTP ${r.status}`);
      this.use(await r.text(), "bundled");
    } catch (e) {
      this.error = `country file: ${e}`;
    }
  }

  async download() {
    await this.run(async () => this.use(await invoke<string>("cty_download"), "custom"));
  }

  async loadFile(path: string) {
    await this.run(async () => this.use(await invoke<string>("cty_load_file", { path }), "custom"));
  }

  async reset() {
    await this.run(async () => {
      await invoke("cty_reset");
      await this.load();
    });
  }

  private async run(f: () => Promise<void>) {
    this.busy = true;
    this.error = null;
    try {
      await f();
    } catch (e) {
      this.error = String(e);
    } finally {
      this.busy = false;
    }
  }

  private use(text: string, source: "bundled" | "custom") {
    this.memo.clear();
    this.db = CtyDb.parse(text);
    this.source = source;
  }

  /// Country, zones and continent for a call (memoised; null if unknown or
  /// no file loaded).
  lookup(call: string): CtyHit | null {
    const db = this.db;
    if (!db) return null;
    const c = call.trim().toUpperCase();
    if (this.memo.has(c)) return this.memo.get(c)!;
    const h = db.lookup(c);
    if (this.memo.size > 20_000) this.memo.clear();
    this.memo.set(c, h);
    return h;
  }
}

export const cty = new CtyStore();
