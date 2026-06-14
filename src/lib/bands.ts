// Ham-band lookup from frequency in Hz.
// Covers HF through 23cm. Returns "—" for invalid / 0 freq, "?" for
// frequencies outside any amateur allocation.

export function bandFromHz(hz: number): string {
  if (!hz) return "—";
  if (hz < 2_000_000) return "160m";
  if (hz < 4_000_000) return "80m";
  if (hz < 7_300_000) return "40m";
  if (hz < 10_500_000) return "30m";
  if (hz < 14_500_000) return "20m";
  if (hz < 18_500_000) return "17m";
  if (hz < 22_000_000) return "15m";
  if (hz < 25_500_000) return "12m";
  if (hz < 30_000_000) return "10m";
  if (hz >= 50_000_000 && hz < 54_000_000) return "6m";
  if (hz >= 144_000_000 && hz < 148_000_000) return "2m";
  if (hz >= 222_000_000 && hz < 225_000_000) return "1.25m";
  if (hz >= 420_000_000 && hz < 450_000_000) return "70cm";
  if (hz >= 902_000_000 && hz < 928_000_000) return "33cm";
  if (hz >= 1_240_000_000 && hz < 1_300_000_000) return "23cm";
  return "?";
}

export function fmtMhz(hz: number): string {
  if (!hz) return "—";
  const h = Math.round(hz); // freqs may be fractional (sub-bin audio offsets)
  const mhz = Math.floor(h / 1_000_000);
  const khz = Math.floor((h % 1_000_000) / 1000);
  const rem = h % 1000;
  return `${mhz}.${khz.toString().padStart(3, "0")}.${rem
    .toString()
    .padStart(3, "0")}`;
}
