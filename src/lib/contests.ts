// Contest profile registry. A profile specifies:
//   - human name + Cabrillo CONTEST: field
//   - how to format what we send (per QSO) given operator settings + serial
//   - hint text for the received exchange field

import { settings } from "./settings.svelte";

export interface ContestProfile {
  id: string;
  name: string;
  cabrilloName: string; // "CQ-WW-RTTY", "" if not Cabrillo-defined
  exchangeFormat: string; // shown in UI as "RST + CQ Zone"
  rcvdPlaceholder: string; // placeholder for the entry exch input
  buildSent: (serial: number) => string;
  // When false, the received exchange is optional — used by the General QSO
  // (ragchew) profile so you can log a contact with just a callsign. Treated
  // as true when omitted.
  requiresExchange?: boolean;
  // Build the expected received exchange from an N1MM-style call history
  // record (field → value, e.g. Name/State/CQZone). Return "" when the
  // record doesn't carry what this contest needs. Omitted for serial-number
  // contests, where history can't predict the exchange.
  historyExchange?: (rec: Record<string, string>) => string;
}

// Case-insensitive field fetch from a history record.
function hf(rec: Record<string, string>, ...names: string[]): string {
  for (const n of names) {
    for (const k of Object.keys(rec)) {
      if (k.toLowerCase() === n.toLowerCase() && rec[k]) return rec[k];
    }
  }
  return "";
}

export const CONTESTS: ContestProfile[] = [
  {
    id: "qso",
    name: "General QSO (ragchew)",
    cabrilloName: "",
    exchangeFormat: "RST / name / QTH (optional)",
    rcvdPlaceholder: "RST NAME QTH",
    requiresExchange: false,
    buildSent: () => `599${settings.myName ? " " + settings.myName : ""}`,
    historyExchange: (r) => [hf(r, "Name"), hf(r, "State", "Loc1")].filter(Boolean).join(" "),
  },
  {
    id: "generic",
    name: "Generic RTTY (RST + Serial)",
    cabrilloName: "",
    exchangeFormat: "RST + Serial",
    rcvdPlaceholder: "001",
    buildSent: (serial) => `599 ${String(serial).padStart(3, "0")}`,
  },
  {
    id: "cqww-rtty",
    name: "CQ WW RTTY DX",
    cabrilloName: "CQ-WW-RTTY",
    exchangeFormat: "RST + CQ Zone (US/VE add State)",
    rcvdPlaceholder: "5 MA",
    buildSent: () => {
      const z = settings.myZone || "?";
      const s = settings.myState ? ` ${settings.myState}` : "";
      return `599 ${z}${s}`;
    },
    historyExchange: (r) =>
      [hf(r, "CQZone", "Zone", "Exch1"), hf(r, "State", "Loc1")].filter(Boolean).join(" "),
  },
  {
    id: "wpx-rtty",
    name: "CQ WPX RTTY",
    cabrilloName: "CQ-WPX-RTTY",
    exchangeFormat: "RST + Serial",
    rcvdPlaceholder: "001",
    buildSent: (serial) => `599 ${String(serial).padStart(3, "0")}`,
  },
  {
    id: "rtty-roundup",
    name: "ARRL RTTY Roundup",
    cabrilloName: "ARRL-RTTY",
    exchangeFormat: "RST + State/Prov (DX: Serial)",
    rcvdPlaceholder: "MA",
    buildSent: () => `599 ${settings.myState || "?"}`,
    historyExchange: (r) => hf(r, "State", "Loc1", "Sect", "Exch1"),
  },
  {
    id: "naqp-rtty",
    name: "NAQP RTTY",
    cabrilloName: "NAQP-RTTY",
    exchangeFormat: "Name + State/Prov/Country",
    rcvdPlaceholder: "JOHN MA",
    buildSent: () => {
      const n = settings.myName || "?";
      const s = settings.myState || "?";
      return `${n} ${s}`;
    },
    historyExchange: (r) => [hf(r, "Name"), hf(r, "State", "Loc1")].filter(Boolean).join(" "),
  },
  {
    id: "makrothen-rtty",
    name: "Makrothen RTTY",
    cabrilloName: "MAKROTHEN-RTTY",
    exchangeFormat: "Grid square (4 characters)",
    rcvdPlaceholder: "FN31",
    buildSent: () => (settings.myGrid.slice(0, 4) || "?").toUpperCase(),
    historyExchange: (r) => hf(r, "Grid", "GridSquare", "Loc1", "Exch1").slice(0, 4),
  },
];

/// The received exchange a call-history record predicts. The column picked
/// in Settings wins; otherwise the contest's own rule; otherwise Exch1 —
/// N1MM's catch-all exchange column, which is how history files for
/// contests Diddle has no profile for (URC DX, say) carry theirs.
export function historyExchange(contest: ContestProfile, rec: Record<string, string>): string {
  if (settings.historyField) return hf(rec, settings.historyField);
  if (contest.historyExchange) return contest.historyExchange(rec);
  return hf(rec, "Exch1");
}

export function activeContest(): ContestProfile {
  return CONTESTS.find((c) => c.id === settings.activeContest) || CONTESTS[0];
}
