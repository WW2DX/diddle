// Sideband-aware frequency conversions.
//
// Diddle runs AFSK RTTY in DIGL (LSB), so an audio tone at `audioHz` sits at
// RF = dial − audioHz. Every place that converts between audio offsets and
// RF (waterfall labels, bandmap spots, QSY-to-spot, logged QSO frequency)
// must agree on this, or spots land kHz off and the log shows the dial
// instead of where the mark tone actually is.

export function isLowerSideband(mode: string | undefined): boolean {
  const m = (mode || "").toLowerCase();
  // Diddle forces DIGL, so an unknown/empty mode is treated as LSB too.
  if (m === "" ) return true;
  return m === "digl" || m === "lsb";
}

/** RF frequency of an audio tone given the dial frequency. */
export function rfFromAudio(dialHz: number, audioHz: number, mode?: string): number {
  return isLowerSideband(mode) ? dialHz - audioHz : dialHz + audioHz;
}

/** Audio offset of an RF frequency given the dial frequency. */
export function audioFromRf(dialHz: number, rfHz: number, mode?: string): number {
  return isLowerSideband(mode) ? dialHz - rfHz : rfHz - dialHz;
}

/** Dial frequency that puts `rfHz` at audio tone `audioHz`. */
export function dialForRf(rfHz: number, audioHz: number, mode?: string): number {
  return Math.round(isLowerSideband(mode) ? rfHz + audioHz : rfHz - audioHz);
}

/**
 * Accept the common ways an operator types a frequency:
 *   "14.080.500" → MHz.kHz.Hz (matches fmtMhz output, round-trips)
 *   "14080.5"    → kHz
 *   "14.0805"    → MHz (any decimal w/ a single dot)
 *   "14080500"   → raw Hz
 * Returns Hz, or null if it doesn't parse.
 */
export function parseFreqInput(raw: string): number | null {
  const s = raw.trim();
  if (!s) return null;
  if (/^\d+\.\d{1,3}\.\d{1,3}$/.test(s)) {
    const [mhz, khz, hz] = s.split(".").map(Number);
    return mhz * 1_000_000 + khz * 1000 + hz;
  }
  if (!/^\d+(\.\d+)?$/.test(s)) return null;
  const n = parseFloat(s);
  if (!isFinite(n) || n <= 0) return null;
  if (s.includes(".")) {
    // "14080.5" is kHz; "14.0805" is MHz. Anything ≥ 1000 with a dot is kHz.
    return Math.round(n >= 1000 ? n * 1000 : n * 1_000_000);
  }
  if (n < 100_000) return Math.round(n * 1000); // kHz integer
  return Math.round(n); // raw Hz
}
