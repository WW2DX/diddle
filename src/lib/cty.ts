// Country file (cty.dat / WL_CTY.DAT, by AD1C — country-files.com): maps a
// callsign to its DXCC/WAE entity, CQ and ITU zones and continent. This is
// what N1MM+ and WriteLog use to predict zones and count country mults.
//
// Format: one record per entity, terminated by ';'
//   Name:  CQ:  ITU:  Cont:  Lat:  Lon:  UTC:  Prefix:
//       pfx,pfx(cq)[itu],=EXACTCALL(cq)[itu]{cont},…;
// Overrides on an entry: (CQ zone) [ITU zone] <lat/lon> {continent} ~utc~.
// '=' marks an exact callsign. A primary prefix starting with '*' is a WAE
// (CQ-only) entity. WL_CTY.DAT adds '#' template lines, which we skip.
//
// Pure module (no runtime imports) so it can be unit-tested under Node.

export interface CtyEntity {
  name: string;
  cq: number;
  itu: number;
  cont: string;
  /// Primary prefix without the WAE '*', e.g. "K", "UA9", "GM/s".
  prefix: string;
  /// CQ-only (WAE) entity, not on the ARRL DXCC list.
  wae: boolean;
}

export interface CtyHit {
  entity: CtyEntity;
  cq: number;
  itu: number;
  cont: string;
}

interface Entry {
  entity: CtyEntity;
  cq?: number;
  itu?: number;
  cont?: string;
}

// Portable / operating suffixes that don't change the location.
const IGNORE_SUFFIXES = new Set(["P", "M", "QRP", "QRPP", "A", "B", "LH", "J", "R", "T"]);
// Suffixes that put the station in no DXCC entity at all.
const NO_ENTITY_SUFFIXES = new Set(["MM", "AM"]);

// CQ zone by call-area digit. cty.dat lists the US and Canada as one zone
// (5) apart from individual exact calls; WL_CTY.DAT only templates some
// prefixes. This is the rule loggers apply on top. Portable operators send
// their real zone, which the operator can always overtype.
const US_AREA_ZONE: Record<string, number> = {
  "0": 4, "1": 5, "2": 5, "3": 5, "4": 5, "5": 4, "6": 3, "7": 3, "8": 4, "9": 4,
};
const VE_AREA_ZONE: Record<string, number> = {
  "1": 5, "2": 5, "3": 4, "4": 4, "5": 4, "6": 4, "7": 3, "8": 1, "9": 5,
};

export class CtyDb {
  private exact = new Map<string, Entry>();
  private prefixes = new Map<string, Entry>();
  private maxPrefixLen = 0;
  readonly entities: CtyEntity[] = [];

  static parse(text: string): CtyDb {
    const db = new CtyDb();
    const body = text
      .split(/\r?\n/)
      .filter((l) => !l.trimStart().startsWith("#"))
      .join("\n");
    for (const rec of body.split(";")) {
      const t = rec.trim();
      if (!t) continue;
      const nl = t.indexOf("\n");
      const head = (nl < 0 ? t : t.slice(0, nl)).split(":").map((x) => x.trim());
      if (head.length < 8) continue;
      const [name, cq, itu, cont, , , , pfx] = head;
      const wae = pfx.startsWith("*");
      const entity: CtyEntity = {
        name,
        cq: parseInt(cq, 10),
        itu: parseInt(itu, 10),
        cont: cont.toUpperCase(),
        prefix: wae ? pfx.slice(1) : pfx,
        wae,
      };
      if (!Number.isFinite(entity.cq)) continue;
      db.entities.push(entity);
      const list = nl < 0 ? "" : t.slice(nl + 1);
      for (const raw of list.split(",")) {
        const tok = raw.trim();
        if (tok) db.addToken(tok, entity);
      }
    }
    return db;
  }

  private addToken(tok: string, entity: CtyEntity) {
    const isExact = tok.startsWith("=");
    let s = isExact ? tok.slice(1) : tok;
    const e: Entry = { entity };
    s = s.replace(/\((\d+)\)/, (_, z) => ((e.cq = parseInt(z, 10)), ""));
    s = s.replace(/\[(\d+)\]/, (_, z) => ((e.itu = parseInt(z, 10)), ""));
    s = s.replace(/\{([A-Za-z]{2})\}/, (_, c) => ((e.cont = c.toUpperCase()), ""));
    s = s.replace(/<[^>]*>/, "").replace(/~[^~]*~/, "");
    const key = s.trim().toUpperCase();
    if (!key) return;
    if (isExact) {
      this.exact.set(key, e);
    } else {
      // Later records win on a clash, matching how loggers read the file.
      this.prefixes.set(key, e);
      if (key.length > this.maxPrefixLen) this.maxPrefixLen = key.length;
    }
  }

  get size(): number {
    return this.entities.length;
  }

  /// Country, zones and continent for a callsign, or null if unknown.
  lookup(rawCall: string): CtyHit | null {
    const call = rawCall.trim().toUpperCase();
    if (!call) return null;
    const ex = this.exact.get(call);
    if (ex) return hit(ex);

    const parts = call.split("/").filter(Boolean);
    if (parts.some((p) => NO_ENTITY_SUFFIXES.has(p))) return null;
    const kept = parts.filter((p) => !IGNORE_SUFFIXES.has(p));
    if (kept.length === 0) return null;

    let target: string;
    let areaFromCall = true;
    if (kept.length === 1) {
      target = kept[0];
      const ex1 = this.exact.get(target);
      if (ex1) return hit(ex1);
    } else {
      const digitPart = kept.find((p) => /^\d$/.test(p));
      const base = kept.filter((p) => !/^\d$/.test(p)).sort((a, b) => b.length - a.length)[0];
      if (digitPart && base) {
        // K1ABC/4: same country, call area 4.
        target = base.replace(/\d/, digitPart);
      } else {
        // VP2E/K1ABC or K1ABC/VP9: the shorter part names the location.
        const byLen = [...kept].sort((a, b) => a.length - b.length);
        target = byLen[0];
        areaFromCall = false;
      }
    }

    for (let n = Math.min(target.length, this.maxPrefixLen); n > 0; n--) {
      const e = this.prefixes.get(target.slice(0, n));
      if (!e) continue;
      const h = hit(e);
      // US / Canada: zone by call area unless the file says otherwise.
      if (e.cq === undefined && areaFromCall) {
        const digit = target.match(/\d/)?.[0];
        const map =
          e.entity.prefix === "K" ? US_AREA_ZONE : e.entity.prefix === "VE" ? VE_AREA_ZONE : null;
        if (map && digit && map[digit]) h.cq = map[digit];
      }
      return h;
    }
    return null;
  }
}

function hit(e: Entry): CtyHit {
  return {
    entity: e.entity,
    cq: e.cq ?? e.entity.cq,
    itu: e.itu ?? e.entity.itu,
    cont: e.cont ?? e.entity.cont,
  };
}
