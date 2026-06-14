// Reactive RTTY decoder tuning. Shared across Waterfall (click sets mark)
// and DecoderView (shift selector, reverse toggle, current-tones display).
// Writes propagate to the Rust backend via set_rtty_tones.

import { invoke } from "@tauri-apps/api/core";

class RttyConfigStore {
  markHz = $state(2125);
  shiftHz = $state(170);
  baud = $state(45.45);
  reverse = $state(false);
  syncing = $state(false);

  /// Derived: space tone, accounting for shift direction.
  get spaceHz(): number {
    return this.reverse ? this.markHz - this.shiftHz : this.markHz + this.shiftHz;
  }

  async load() {
    try {
      const cfg = await invoke<{
        mark_hz: number;
        space_hz: number;
        baud: number;
      }>("get_rtty_config");
      this.markHz = cfg.mark_hz;
      const diff = cfg.space_hz - cfg.mark_hz;
      this.reverse = diff < 0;
      this.shiftHz = Math.abs(diff);
      this.baud = cfg.baud;
    } catch (e) {
      console.error("get_rtty_config failed", e);
    }
  }

  async setMark(hz: number) {
    this.markHz = Math.max(50, Math.min(20000, hz));
    await this.sync();
  }

  async setShift(hz: number) {
    this.shiftHz = hz;
    await this.sync();
  }

  async setBaud(baud: number) {
    this.baud = baud;
    await this.sync();
  }

  async setReverse(r: boolean) {
    this.reverse = r;
    await this.sync();
  }

  private async sync() {
    this.syncing = true;
    try {
      await invoke("set_rtty_config", {
        markHz: this.markHz,
        spaceHz: this.spaceHz,
        baud: this.baud,
      });
    } catch (e) {
      console.error("set_rtty_config failed", e);
    } finally {
      this.syncing = false;
    }
  }
}

export const rttyConfig = new RttyConfigStore();
