// Reactive RTTY decoder tuning. Shared across Waterfall (click sets mark)
// and DecoderView (shift selector, reverse toggle, current-tones display).
// Writes propagate to the Rust backend via set_rtty_tones.
//
// RX and TX marks are tracked separately: deliberate tuning (waterfall
// click, auto-tune, QSY) moves both, while AFC moves only the decoder —
// so following a drifting station never drags our transmitted signal
// around the band (MMTTY-style AFC/NET separation).

import { invoke } from "@tauri-apps/api/core";

class RttyConfigStore {
  markHz = $state(2125);
  txMarkHz = $state(2125);
  shiftHz = $state(170);
  baud = $state(45.45);
  reverse = $state(false);
  syncing = $state(false);

  /// Derived: space tone, accounting for shift direction.
  get spaceHz(): number {
    return this.reverse ? this.markHz - this.shiftHz : this.markHz + this.shiftHz;
  }

  get txSpaceHz(): number {
    return this.reverse ? this.txMarkHz - this.shiftHz : this.txMarkHz + this.shiftHz;
  }

  async load() {
    try {
      const cfg = await invoke<{
        mark_hz: number;
        space_hz: number;
        baud: number;
        tx_mark_hz?: number;
      }>("get_rtty_config");
      this.markHz = cfg.mark_hz;
      this.txMarkHz = cfg.tx_mark_hz || cfg.mark_hz;
      const diff = cfg.space_hz - cfg.mark_hz;
      this.reverse = diff < 0;
      this.shiftHz = Math.abs(diff);
      this.baud = cfg.baud;
    } catch (e) {
      console.error("get_rtty_config failed", e);
    }
  }

  /// Deliberate tuning: move RX and TX together.
  async setMark(hz: number) {
    this.markHz = Math.max(50, Math.min(20000, hz));
    this.txMarkHz = this.markHz;
    await this.sync();
  }

  /// AFC: move only the decoder; TX stays put.
  async setRxMark(hz: number) {
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
        txMarkHz: this.txMarkHz,
        txSpaceHz: this.txSpaceHz,
      });
    } catch (e) {
      console.error("set_rtty_config failed", e);
    } finally {
      this.syncing = false;
    }
  }
}

export const rttyConfig = new RttyConfigStore();
