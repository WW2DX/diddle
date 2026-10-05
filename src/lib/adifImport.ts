// ADIF → Diddle QSOs, for importing a log into a new named log — Diddle's
// own exports (round trip, including edits made to the file afterwards)
// and other loggers' (N1MM+, WriteLog…).
//
// Pure module (no runtime imports — the band lookup is passed in) so it can
// be unit-tested under Node.

import type { Qso } from "./types";

export interface AdifImport {
  qsos: Qso[];
  /// CONTEST_ID of the first record that has one (e.g. "CQ-WW-RTTY").
  contestId: string;
  /// Records skipped for having no call or no usable date/time.
  skipped: number;
}

/// Split ADIF text into records of field → value (names upper-cased). The
/// header (anything before <EOH>) is dropped.
export function parseAdifRecords(text: string): Record<string, string>[] {
  const eoh = text.search(/<eoh>/i);
  let i = eoh >= 0 ? eoh + 5 : 0;
  const records: Record<string, string>[] = [];
  let cur: Record<string, string> = {};
  const tag = /<([A-Za-z0-9_]+)(?::(\d+))?(?::[A-Za-z])?>/g;
  tag.lastIndex = i;
  let m: RegExpExecArray | null;
  while ((m = tag.exec(text))) {
    const name = m[1].toUpperCase();
    if (name === "EOR") {
      if (Object.keys(cur).length) records.push(cur);
      cur = {};
      continue;
    }
    if (m[2] === undefined) continue;
    const len = parseInt(m[2], 10);
    const start = m.index + m[0].length;
    cur[name] = text.slice(start, start + len);
    tag.lastIndex = start + len;
    i = tag.lastIndex;
  }
  if (Object.keys(cur).length) records.push(cur);
  return records;
}

function adifTs(date: string, time: string): number | null {
  const d = date.trim();
  const t = (time || "0000").trim().padEnd(6, "0");
  if (!/^\d{8}$/.test(d) || !/^\d{6}$/.test(t)) return null;
  const ts = Date.UTC(
    +d.slice(0, 4),
    +d.slice(4, 6) - 1,
    +d.slice(6, 8),
    +t.slice(0, 2),
    +t.slice(2, 4),
    +t.slice(4, 6),
  );
  return Number.isNaN(ts) ? null : ts;
}

export function importAdif(text: string, bandFromHz: (hz: number) => string): AdifImport {
  const out: Qso[] = [];
  let skipped = 0;
  let contestId = "";
  for (const r of parseAdifRecords(text)) {
    const call = (r.CALL || "").trim().toUpperCase();
    const ts = adifTs(r.QSO_DATE || "", r.TIME_ON || "");
    if (!call || ts === null) {
      skipped++;
      continue;
    }
    if (!contestId && r.CONTEST_ID) contestId = r.CONTEST_ID.trim();
    const mhz = parseFloat(r.FREQ || "");
    const freqHz = Number.isFinite(mhz) ? Math.round(mhz * 1_000_000) : 0;
    const band = freqHz ? bandFromHz(freqHz) : (r.BAND || "").trim().toLowerCase() || "—";
    // Diddle writes the whole exchange to STX_STRING / SRX_STRING; other
    // loggers often only have the serial in STX / SRX.
    const exchSent = (r.STX_STRING || r.STX || "").trim();
    const exchRcvd = (r.SRX_STRING || r.SRX || "").trim();
    const serial = parseInt(r.APP_DIDDLE_SERIAL || r.STX || "", 10);
    out.push({
      id: crypto.randomUUID(),
      ts,
      call,
      freqHz,
      band,
      mode: (r.MODE || "RTTY").trim().toUpperCase(),
      rstSent: (r.RST_SENT || "599").trim(),
      rstRcvd: (r.RST_RCVD || "599").trim(),
      exchSent,
      exchRcvd,
      serialSent: Number.isFinite(serial) && serial > 0 ? serial : 0,
      ...(r.COMMENT?.trim() ? { note: r.COMMENT.trim() } : {}),
    });
  }
  out.sort((a, b) => a.ts - b.ts);
  // Number any QSO the file gave no serial, in time order after the rest.
  let next = Math.max(0, ...out.map((q) => q.serialSent)) + 1;
  for (const q of out) if (!q.serialSent) q.serialSent = next++;
  return { qsos: out, contestId, skipped };
}
