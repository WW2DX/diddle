// The radio's receive side, set over TCI: a narrow RX filter for heavy QRM
// and the AGC. Narrow filters are centred on the decoder's mark/space pair
// and follow it as you retune; "Wide" puts back the filter the radio had
// before we narrowed it.
//
// The width is per session: a narrow filter hides the rest of the band from
// the waterfall and the bandmap decoder, which shouldn't come as a surprise
// on the next launch.

import { sendRaw, type RigState } from "$lib/tci";
import { isLowerSideband } from "$lib/freq";

export const RX_WIDTHS = [0, 500, 250] as const; // 0 = wide (radio's own)
export type RxWidth = (typeof RX_WIDTHS)[number];

// Restored when nothing wider was ever seen: 200–3000 Hz audio.
const FALLBACK_WIDE_HZ = [200, 3000];
// A reported filter at least this wide is the operator's own "wide" one.
const WIDE_MIN_HZ = 600;

class RadioRx {
  width = $state<RxWidth>(0);
  lastError = $state<string | null>(null);
  // Edges of the last wide filter the radio reported (VFO-relative).
  private wide: [number, number] | null = null;
  private sent: [number, number] | null = null;

  /// Note what the radio reports: a wide filter is remembered for "Wide".
  observe(rig: RigState) {
    const lo = rig.filter_lo,
      hi = rig.filter_hi;
    if (lo == null || hi == null) return;
    if (hi - lo >= WIDE_MIN_HZ) this.wide = [lo, hi];
  }

  setWidth(w: RxWidth, rig: RigState, markHz: number, spaceHz: number) {
    this.width = w;
    this.sent = null;
    if (w === 0) {
      const [lo, hi] = this.wide ?? this.edgesFor(rig.mode, FALLBACK_WIDE_HZ[0], FALLBACK_WIDE_HZ[1]);
      void this.send(lo, hi);
    } else {
      this.follow(rig, markHz, spaceHz);
    }
  }

  /// Keep a narrow filter centred on the tone pair (call on retune / mode
  /// change). Small moves (AFC) under 10 Hz are left alone.
  follow(rig: RigState, markHz: number, spaceHz: number) {
    if (this.width === 0) return;
    const centre = (markHz + spaceHz) / 2;
    const [lo, hi] = this.edgesFor(rig.mode, centre - this.width / 2, centre + this.width / 2);
    if (this.sent && Math.abs(this.sent[0] - lo) < 10 && Math.abs(this.sent[1] - hi) < 10) return;
    void this.send(lo, hi);
  }

  setAgc(mode: "normal" | "fast" | "off") {
    void this.raw(`agc_mode:0,${mode};`);
  }

  setAgcGain(db: number) {
    const g = Math.round(Math.min(120, Math.max(-20, db)));
    void this.raw(`agc_gain:0,${g};`);
  }

  // Audio passband [a, b] Hz → VFO-relative edges. In DIGL/LSB audio tone f
  // is f Hz *below* the VFO.
  private edgesFor(mode: string, a: number, b: number): [number, number] {
    a = Math.round(a);
    b = Math.round(b);
    return isLowerSideband(mode) ? [-b, -a] : [a, b];
  }

  private async send(lo: number, hi: number) {
    this.sent = [lo, hi];
    await this.raw(`rx_filter_band:0,${lo},${hi};`);
  }

  private async raw(cmd: string) {
    try {
      await sendRaw(cmd);
      this.lastError = null;
    } catch (e) {
      this.lastError = String(e);
    }
  }
}

export const radioRx = new RadioRx();
