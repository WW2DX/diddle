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
  },
];

export function activeContest(): ContestProfile {
  return CONTESTS.find((c) => c.id === settings.activeContest) || CONTESTS[0];
}
