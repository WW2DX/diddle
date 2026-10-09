// ADIF + Cabrillo export formatters.

import type { Qso } from "./types";
import { settings } from "./settings.svelte";
import { CONTESTS } from "./contests";
import { cty } from "./ctyStore.svelte";
import { scoreStore } from "./score.svelte";
import { parseExchange, wpxPrefix, type ParsedExch } from "./scoring";

// Received/sent exchange split into contest fields, with the country file
// filling what the exchange doesn't carry (CQ zone).
function parsedRcvd(q: Qso): ParsedExch & { hit: ReturnType<typeof cty.lookup> } {
  const hit = cty.lookup(q.call);
  const p = parseExchange(settings.activeContest, q.exchRcvd, hit);
  if (settings.activeContest === "cqww-rtty" && p.zone === undefined && hit) p.zone = hit.cq;
  return { ...p, hit };
}

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
    // Each exchange part in its own field, the way N1MM+ / WriteLog import.
    const contest = CONTESTS.find((c) => c.id === settings.activeContest);
    if (contest?.cabrilloName) parts.push(adifField("CONTEST_ID", contest.cabrilloName));
    const p = parsedRcvd(q);
    if (p.hit) {
      parts.push(adifField("COUNTRY", p.hit.entity.name));
      parts.push(adifField("CONT", p.hit.cont));
      parts.push(adifField("ITUZ", String(p.hit.itu)));
    }
    if (p.zone !== undefined) parts.push(adifField("CQZ", String(p.zone)));
    else if (p.hit) parts.push(adifField("CQZ", String(p.hit.cq)));
    if (p.region) {
      parts.push(adifField(p.hit?.entity.prefix === "VE" ? "VE_PROV" : "STATE", p.region));
    }
    if (p.serial !== undefined) parts.push(adifField("SRX", String(p.serial)));
    if (p.name) parts.push(adifField("NAME", p.name));
    if (p.grid) parts.push(adifField("GRIDSQUARE", p.grid));
    if (settings.myGrid) parts.push(adifField("MY_GRIDSQUARE", settings.myGrid.toUpperCase()));
    if (settings.activeContest === "wpx-rtty") parts.push(adifField("PFX", wpxPrefix(q.call)));
    if (/^(generic|wpx-rtty|rtty-roundup)$/.test(settings.activeContest)) {
      parts.push(adifField("STX", String(q.serialSent)));
    }
    parts.push(adifField("APP_DIDDLE_SERIAL", String(q.serialSent)));
    if (q.note) parts.push(adifField("COMMENT", q.note));
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

// One side of a Cabrillo QSO line (call + exchange) in the active contest's
// column layout.
function cabSide(call: string, rst: string, exch: string, serial: number, isMine: boolean): string {
  const id = settings.activeContest;
  const hit = cty.lookup(call);
  const p = parseExchange(id, exch, hit);
  const c = call.padEnd(13, " ");
  const r = (rst || "599").padEnd(3, " ");
  const wve = hit ? hit.entity.prefix === "K" || hit.entity.prefix === "VE" : false;
  switch (id) {
    case "cqww-rtty": {
      const zone = p.zone ?? (isMine ? parseInt(settings.myZone, 10) : hit?.cq);
      const qth = wve ? p.region || (isMine ? settings.myState : "") || "" : "DX";
      return `${c} ${r} ${String(zone ?? "").padStart(2, "0")} ${qth.padEnd(2, " ")}`;
    }
    case "rtty-roundup": {
      const x = wve ? p.region || (isMine ? settings.myState : "") : String(isMine ? serial : p.serial ?? "");
      return `${c} ${r} ${x.padEnd(6, " ")}`;
    }
    case "naqp-rtty": {
      const toks = exch.trim().split(/\s+/);
      const name = (isMine ? settings.myName : toks[0]) || "";
      const loc = (isMine ? settings.myState : toks[toks.length - 1]) || "";
      return `${c} ${name.padEnd(10, " ")} ${loc.padEnd(3, " ")}`;
    }
    case "makrothen-rtty": {
      const grid = isMine ? settings.myGrid.slice(0, 4).toUpperCase() : p.grid || exch.trim().slice(0, 4);
      return `${c} ${grid.padEnd(4, " ")}`;
    }
        case "wpx-rtty":
    case "generic": {
      const n = isMine ? serial : p.serial;
      return `${c} ${r} ${String(n ?? "").padStart(4, " ")}`;
    }
    default:
      return `${c} ${r} ${exch.padEnd(8, " ")}`;
  }
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
  lines.push(`CLAIMED-SCORE: ${scoreStore.score.score}`);
  lines.push(`NAME: ${settings.myName || ""}`);
  lines.push(`GRID-LOCATOR: ${settings.myGrid || ""}`);
  lines.push("CREATED-BY: Diddle");
  for (const q of qsos) {
    const mine = cabSide(settings.myCall || "", q.rstSent, q.exchSent, q.serialSent, true);
    const his = cabSide(q.call, q.rstRcvd, q.exchRcvd, 0, false);
    lines.push(
      `QSO: ${cabrilloFreqKhz(q.freqHz)} RY ${cabrilloDate(q.ts)} ${cabrilloTime(q.ts)} ${mine} ${his}`.trimEnd(),
    );
  }
  lines.push("END-OF-LOG:");
  return lines.join("\n") + "\n";
}
