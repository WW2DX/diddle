// ADIF + Cabrillo export formatters.

import type { Qso } from "./types";
import { settings } from "./settings.svelte";
import { CONTESTS } from "./contests";

function pad(n: number, w: number): string {
  return n.toString().padStart(w, "0");
}

function utcDate(ts: number): string {
  const d = new Date(ts);
  return `${d.getUTCFullYear()}${pad(d.getUTCMonth() + 1, 2)}${pad(d.getUTCDate(), 2)}`;
}

function utcTime(ts: number): string {
  const d = new Date(ts);
  return `${pad(d.getUTCHours(), 2)}${pad(d.getUTCMinutes(), 2)}${pad(d.getUTCSeconds(), 2)}`;
}

function bandToAdif(band: string): string {
  // Normalize "20m" → "20M" (ADIF wants no lowercase m).
  return band.toUpperCase();
}

function adifField(name: string, value: string): string {
  return `<${name}:${value.length}>${value}`;
}

export function toAdif(qsos: Qso[]): string {
  const lines: string[] = [];
  lines.push("ADIF export from Diddle");
  lines.push("<ADIF_VER:5>3.1.4");
  lines.push("<PROGRAMID:6>Diddle");
  lines.push(`<CREATED_TIMESTAMP:15>${utcDate(Date.now())} ${utcTime(Date.now())}`);
  lines.push("<EOH>");
  lines.push("");
  for (const q of qsos) {
    const parts: string[] = [];
    parts.push(adifField("CALL", q.call));
    parts.push(adifField("QSO_DATE", utcDate(q.ts)));
    parts.push(adifField("TIME_ON", utcTime(q.ts).slice(0, 4)));
    parts.push(adifField("BAND", bandToAdif(q.band)));
    parts.push(adifField("FREQ", (q.freqHz / 1_000_000).toFixed(6)));
    parts.push(adifField("MODE", "RTTY"));
    if (q.rstSent) parts.push(adifField("RST_SENT", q.rstSent));
    if (q.rstRcvd) parts.push(adifField("RST_RCVD", q.rstRcvd));
    if (q.exchSent) parts.push(adifField("STX_STRING", q.exchSent));
    if (q.exchRcvd) parts.push(adifField("SRX_STRING", q.exchRcvd));
    parts.push(adifField("APP_DIDDLE_SERIAL", String(q.serialSent)));
    parts.push("<EOR>");
    lines.push(parts.join(" "));
  }
  return lines.join("\n") + "\n";
}

function cabrilloFreqKhz(hz: number): string {
  // Cabrillo wants kHz integer.
  return Math.round(hz / 1000).toString().padStart(5, " ");
}

function cabrilloDate(ts: number): string {
  const d = new Date(ts);
  return `${d.getUTCFullYear()}-${pad(d.getUTCMonth() + 1, 2)}-${pad(d.getUTCDate(), 2)}`;
}

function cabrilloTime(ts: number): string {
  const d = new Date(ts);
  return `${pad(d.getUTCHours(), 2)}${pad(d.getUTCMinutes(), 2)}`;
}

export function toCabrillo(qsos: Qso[]): string {
  const contest = CONTESTS.find((c) => c.id === settings.activeContest);
  const cabName = contest?.cabrilloName || "RTTY";
  const lines: string[] = [];
  lines.push("START-OF-LOG: 3.0");
  lines.push(`CALLSIGN: ${settings.myCall || ""}`);
  lines.push(`CONTEST: ${cabName}`);
  lines.push("CATEGORY-OPERATOR: SINGLE-OP");
  lines.push("CATEGORY-BAND: ALL");
  lines.push("CATEGORY-MODE: RTTY");
  lines.push("CATEGORY-POWER: HIGH");
  lines.push("CATEGORY-STATION: FIXED");
  lines.push("CLAIMED-SCORE: 0");
  lines.push(`NAME: ${settings.myName || ""}`);
  lines.push(`GRID-LOCATOR: ${settings.myGrid || ""}`);
  lines.push("CREATED-BY: Diddle");
  for (const q of qsos) {
    // Cabrillo QSO: freq mode date time call_sent exch_sent call_rcvd exch_rcvd
    const callSent = (settings.myCall || "").padEnd(13, " ");
    const callRcvd = q.call.padEnd(13, " ");
    const exchSent = q.exchSent.padEnd(8, " ");
    const exchRcvd = q.exchRcvd.padEnd(8, " ");
    lines.push(
      `QSO: ${cabrilloFreqKhz(q.freqHz)} RY ${cabrilloDate(q.ts)} ${cabrilloTime(
        q.ts,
      )} ${callSent} ${q.rstSent} ${exchSent} ${callRcvd} ${q.rstRcvd} ${exchRcvd}`,
    );
  }
  lines.push("END-OF-LOG:");
  return lines.join("\n") + "\n";
}
