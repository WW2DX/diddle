// Contest scoring: exchange parsing, QSO points, multipliers, band summary.
//
// Pure module — only type imports — so it runs under Node for unit tests.
// The country lookup is passed in (see cty.ts).
//
// The rules here follow each sponsor's published scoring in outline. Edge
// cases (DC, KH6/KL7 in NAQP, WPX portable prefixes) are simplified; check
// the current year's rules before relying on a claimed score.

import type { CtyHit } from "./cty";

export type Lookup = (call: string) => CtyHit | null;

/// What a logged (or prospective) QSO needs for scoring.
export interface ScoreQso {
  call: string;
  band: string;
  exchRcvd: string;
}

// US states (50) + DC, and Canadian provinces/territories (with common
// alternates loggers accept).
export const US_STATES = new Set([
  "AL","AK","AZ","AR","CA","CO","CT","DE","FL","GA","HI","ID","IL","IN","IA",
  "KS","KY","LA","ME","MD","MA","MI","MN","MS","MO","MT","NE","NV","NH","NJ",
  "NM","NY","NC","ND","OH","OK","OR","PA","RI","SC","SD","TN","TX","UT","VT",
  "VA","WA","WV","WI","WY","DC",
]);
export const VE_PROVS = new Set([
  "NB","NS","PE","PEI","NL","NF","LB","QC","PQ","ON","MB","SK","AB","BC","NT","NWT","NU","YT","YK",
]);
// Fold alternates onto one multiplier.
const PROV_CANON: Record<string, string> = {
  PEI: "PE", NF: "NL", LB: "NL", PQ: "QC", NWT: "NT", YK: "YT",
};
function canonRegion(r: string): string {
  return PROV_CANON[r] ?? r;
}

// WAE entities → their DXCC parent, for ARRL contests (DXCC only).
const WAE_PARENT: Record<string, string> = {
  "4U1V": "OE", "GM/s": "GM", "IG9": "I", "IT9": "I", "JW/b": "JW", "TA1": "TA",
};

const LOW_BANDS = new Set(["160m", "80m", "40m"]);
export const BAND_ORDER = ["160m", "80m", "40m", "20m", "15m", "10m"];

/// Received exchange, split into the parts a contest cares about.
export interface ParsedExch {
  zone?: number;
  region?: string; // US state / VE province (canonical)
  serial?: number;
  name?: string;
  dx?: boolean; // sent "DX" or is outside W/VE
  grid?: string; // 4-character Maidenhead locator
}

const RST = /^(5[1-9][1-9]|5NN|59|5N)$/;

export function parseExchange(contestId: string, exch: string, hit: CtyHit | null): ParsedExch {
  const toks = exch.toUpperCase().split(/\s+/).filter((t) => t && !RST.test(t));
  const isWve = hit ? hit.entity.prefix === "K" || hit.entity.prefix === "VE" : false;
  const out: ParsedExch = {};
  const region = toks.find((t) => US_STATES.has(t) || VE_PROVS.has(t));
  if (region) out.region = canonRegion(region);
  const num = toks.find((t) => /^\d+$/.test(t));
  switch (contestId) {
    case "cqww-rtty": {
      if (num) {
        const z = parseInt(num, 10);
        if (z >= 1 && z <= 40) out.zone = z;
      }
      out.dx = toks.includes("DX") || !isWve;
      if (out.dx) delete out.region;
      break;
    }
    case "rtty-roundup":
      if (num) out.serial = parseInt(num, 10);
      out.dx = !isWve;
      break;
    case "naqp-rtty": {
      const alpha = toks.filter((t) => /^[A-Z]+$/.test(t));
      if (alpha.length) out.name = alpha[0];
      // The location is the last token (a state, province or country).
      if (alpha.length >= 2) {
        const loc = alpha[alpha.length - 1];
        out.region = US_STATES.has(loc) || VE_PROVS.has(loc) ? canonRegion(loc) : undefined;
      }
      break;
    }
    case "makrothen-rtty": {
      const g = toks.find((t) => /^[A-R]{2}\d{2}([A-X]{2})?$/.test(t));
      if (g) out.grid = g.slice(0, 4);
      break;
    }
    default:
      if (num) out.serial = parseInt(num, 10);
  }
  return out;
}

// ---- Grid distance (Makrothen) ----

/// Centre of a 4-character Maidenhead square (6-character locators are
/// cut to their square), as [lat, lon] in degrees; null if malformed.
export function gridCentre(grid: string): [number, number] | null {
  const g = grid.trim().toUpperCase().slice(0, 4);
  if (!/^[A-R]{2}\d{2}$/.test(g)) return null;
  const lon = (g.charCodeAt(0) - 65) * 20 - 180 + Number(g[2]) * 2 + 1;
  const lat = (g.charCodeAt(1) - 65) * 10 - 90 + Number(g[3]) + 0.5;
  return [lat, lon];
}

/// Great-circle distance between two grid squares' centres in km, with
/// the Makrothen rules' formula and earth radius (6378.16 km).
export function gridDistanceKm(a: string, b: string): number | null {
  const p = gridCentre(a);
  const q = gridCentre(b);
  if (!p || !q) return null;
  const r = Math.PI / 180;
  const [a1, b1, a2, b2] = [p[0] * r, p[1] * r, q[0] * r, q[1] * r];
  const c =
    Math.cos(a1) * Math.cos(b1) * Math.cos(a2) * Math.cos(b2) +
    Math.cos(a1) * Math.sin(b1) * Math.cos(a2) * Math.sin(b2) +
    Math.sin(a1) * Math.sin(a2);
  // Rounding can push the same point a hair past 1.
  return Math.acos(Math.min(1, Math.max(-1, c))) * 6378.16;
}

const MAKROTHEN_BAND_FACTOR: Record<string, number> = {
  "80m": 2, "40m": 1.5, "20m": 1, "15m": 1, "10m": 1,
};

/// Makrothen QSO points: whole km between the squares, times the band
/// factor, rounded down; 100 flat in the same square; 0 without both
/// grids or off the contest bands.
export function makrothenPoints(myGrid: string, hisGrid: string, band: string): number {
  const factor = MAKROTHEN_BAND_FACTOR[band];
  const d = gridDistanceKm(myGrid, hisGrid);
  if (!factor || d === null) return 0;
  if (myGrid.trim().toUpperCase().slice(0, 4) === hisGrid.trim().toUpperCase().slice(0, 4)) return 100;
  return Math.floor(Math.floor(d) * factor);
}

/// WPX prefix: letters and digits up to and including the last digit of
/// the call's prefix part. Portable forms: K1ABC/4 → K4, VP2E/K1ABC → VP2E
/// (+0 if it has no digit), K1ABC/P → K1. Calls with no digit get a 0.
export function wpxPrefix(rawCall: string): string {
  const ignore = new Set(["P", "M", "MM", "AM", "QRP", "A", "B"]);
  const parts = rawCall.toUpperCase().split("/").filter((p) => p && !ignore.has(p));
  const digit = parts.find((p) => /^\d$/.test(p));
  const named = parts.filter((p) => !/^\d$/.test(p)).sort((a, b) => a.length - b.length);
  if (named.length === 0) return "";
  if (named.length > 1) {
    // VP2E/K1ABC, W1AW/KH6: the shorter part is the operating prefix.
    const loc = named[0];
    return /\d/.test(loc) ? loc : loc + "0";
  }
  const base = named[0];
  // Everything up to the last digit before the suffix letters.
  const m = base.match(/^(.*\d)[A-Z]+$/);
  let pfx = m ? m[1] : /\d/.test(base) ? base : base.slice(0, 2) + "0";
  if (digit) pfx = pfx.replace(/\d+$/, digit);
  return pfx;
}

/// One kind of multiplier and whether it counts once per band or once.
export interface MultKind {
  key: string; // "zone" | "country" | "region" | "prefix"
  label: string; // column heading
  perBand: boolean;
}

/// What scoring needs from Settings beyond the country file.
export interface ScoreCtx {
  myGrid?: string;
}

export interface ContestRules {
  kinds: MultKind[];
  points(my: CtyHit | null, his: CtyHit | null, band: string, p: ParsedExch, ctx: ScoreCtx): number;
  /// The multiplier values this QSO carries, by kind ("" / undefined = none).
  mults(q: ScoreQso, his: CtyHit | null, p: ParsedExch): Record<string, string | undefined>;
}

function dxccOf(h: CtyHit | null): string | undefined {
  if (!h) return undefined;
  return h.entity.wae ? WAE_PARENT[h.entity.prefix] ?? h.entity.prefix : h.entity.prefix;
}

const RULES: Record<string, ContestRules> = {
  "cqww-rtty": {
    kinds: [
      { key: "zone", label: "Zones", perBand: true },
      { key: "country", label: "Ctys", perBand: true },
      { key: "region", label: "St/Pr", perBand: true },
    ],
    points(my, his) {
      if (!my || !his) return 1;
      if (my.entity.prefix === his.entity.prefix) return 1;
      return my.cont === his.cont ? 2 : 3;
    },
    mults(_q, his, p) {
      const zone = p.zone ?? his?.cq;
      // CQ WW RTTY: US states (KH6/KL7 are separate countries, so they
      // never reach here as W) and VE areas; DC counts as MD.
      const region = p.dx ? undefined : p.region === "DC" ? "MD" : p.region;
      return {
        zone: zone ? String(zone) : undefined,
        country: his?.entity.prefix,
        region: his && (his.entity.prefix === "K" || his.entity.prefix === "VE") ? region : undefined,
      };
    },
  },
  "rtty-roundup": {
    kinds: [
      { key: "region", label: "St/Pr", perBand: false },
      { key: "country", label: "DXCC", perBand: false },
    ],
    points: () => 1,
    mults(_q, his, p) {
      const wve = his && (his.entity.prefix === "K" || his.entity.prefix === "VE");
      return {
        region: wve ? p.region : undefined,
        country: !wve ? dxccOf(his) : undefined,
      };
    },
  },
  "naqp-rtty": {
    kinds: [
      { key: "region", label: "St/Pr", perBand: true },
      { key: "country", label: "NA Ctys", perBand: true },
    ],
    points: () => 1,
    mults(_q, his, p) {
      const wve = his && (his.entity.prefix === "K" || his.entity.prefix === "VE");
      return {
        region: wve ? p.region : undefined,
        country: !wve && his?.cont === "NA" ? dxccOf(his) : undefined,
      };
    },
  },
  "wpx-rtty": {
    kinds: [{ key: "prefix", label: "Prefixes", perBand: false }],
    points(my, his, band) {
      const low = LOW_BANDS.has(band);
      if (!my || !his) return 1;
      if (my.cont !== his.cont) return low ? 6 : 3;
      return low ? 2 : 1;
    },
    mults(q) {
      return { prefix: wpxPrefix(q.call) };
    },
  },
  // Points by distance between grid squares; no multipliers.
  "makrothen-rtty": {
    kinds: [],
    points: (_my, _his, band, p, ctx) => makrothenPoints(ctx.myGrid ?? "", p.grid ?? "", band),
    mults: () => ({}),
  },
};

const NO_RULES: ContestRules = { kinds: [], points: () => 1, mults: () => ({}) };

export function rulesFor(contestId: string): ContestRules {
  return RULES[contestId] ?? NO_RULES;
}

export interface BandRow {
  band: string;
  qsos: number;
  points: number;
  mults: Record<string, number>; // per-band counts (per-band kinds only)
}

export interface Score {
  kinds: MultKind[];
  qsos: number;
  dupes: number;
  points: number;
  mults: Record<string, number>; // totals by kind
  totalMults: number;
  score: number;
  bands: BandRow[];
}

/// Tracks worked multipliers; used both for the score and to tell whether
/// a prospective QSO would be a new one.
export class MultTracker {
  private seen = new Set<string>();
  private worked = new Set<string>();
  readonly rules: ContestRules;
  readonly contestId: string;
  private lookup: Lookup;
  private my: CtyHit | null;
  private ctx: ScoreCtx;
  constructor(contestId: string, lookup: Lookup, my: CtyHit | null, ctx: ScoreCtx = {}) {
    this.contestId = contestId;
    this.lookup = lookup;
    this.my = my;
    this.ctx = ctx;
    this.rules = rulesFor(contestId);
  }

  private keyFor(kind: MultKind, band: string, value: string) {
    return kind.perBand ? `${kind.key}|${band}|${value}` : `${kind.key}||${value}`;
  }

  /// Multipliers this QSO would add, as "Zone 14", "Ukraine", "NY"…
  newMults(q: ScoreQso): string[] {
    const his = this.lookup(q.call);
    const p = parseExchange(this.contestId, q.exchRcvd, his);
    const vals = this.rules.mults(q, his, p);
    const out: string[] = [];
    for (const k of this.rules.kinds) {
      const v = vals[k.key];
      if (v && !this.seen.has(this.keyFor(k, q.band, v))) out.push(describe(k.key, v, his));
    }
    return out;
  }

  isDupe(q: ScoreQso): boolean {
    return this.worked.has(`${q.call.toUpperCase()}|${q.band}`);
  }

  /// Record a QSO. Returns its points and the multipliers it added (empty
  /// for a dupe, which scores nothing).
  add(q: ScoreQso): { points: number; added: { kind: string; value: string }[] } {
    const wk = `${q.call.toUpperCase()}|${q.band}`;
    if (this.worked.has(wk)) return { points: 0, added: [] };
    this.worked.add(wk);
    const his = this.lookup(q.call);
    const p = parseExchange(this.contestId, q.exchRcvd, his);
    const vals = this.rules.mults(q, his, p);
    const added: { kind: string; value: string }[] = [];
    for (const k of this.rules.kinds) {
      const v = vals[k.key];
      if (!v) continue;
      const key = this.keyFor(k, q.band, v);
      if (!this.seen.has(key)) {
        this.seen.add(key);
        added.push({ kind: k.key, value: v });
      }
    }
    return { points: this.rules.points(this.my, his, q.band, p, this.ctx), added };
  }
}

function describe(kind: string, value: string, his: CtyHit | null): string {
  switch (kind) {
    case "zone":
      return `Zone ${value}`;
    case "country":
      return his?.entity.name ?? value;
    case "prefix":
      return `Prefix ${value}`;
    default:
      return value;
  }
}

/// Full score for a log.
export function scoreLog(
  contestId: string,
  qsos: ScoreQso[],
  lookup: Lookup,
  my: CtyHit | null,
  ctx: ScoreCtx = {},
): Score {
  const t = new MultTracker(contestId, lookup, my, ctx);
  const kinds = t.rules.kinds;
  const bands = new Map<string, BandRow>();
  const totals: Record<string, number> = Object.fromEntries(kinds.map((k) => [k.key, 0]));
  let points = 0;
  let dupes = 0;
  let count = 0;
  for (const q of qsos) {
    const row =
      bands.get(q.band) ??
      (bands.set(q.band, { band: q.band, qsos: 0, points: 0, mults: Object.fromEntries(kinds.map((k) => [k.key, 0])) }),
      bands.get(q.band)!);
    if (t.isDupe(q)) {
      dupes++;
      continue;
    }
    const r = t.add(q);
    count++;
    row.qsos++;
    row.points += r.points;
    points += r.points;
    for (const a of r.added) {
      totals[a.kind]++;
      row.mults[a.kind]++;
    }
  }
  const totalMults = kinds.length ? Object.values(totals).reduce((a, b) => a + b, 0) : 0;
  const ordered = [...bands.values()].sort(
    (a, b) => idx(a.band) - idx(b.band),
  );
  return {
    kinds,
    qsos: count,
    dupes,
    points,
    mults: totals,
    totalMults,
    score: kinds.length ? points * totalMults : points,
    bands: ordered,
  };
}

function idx(band: string): number {
  const i = BAND_ORDER.indexOf(band);
  return i < 0 ? 99 : i;
}
