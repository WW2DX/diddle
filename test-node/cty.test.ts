// Run: node --test test-node/
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { CtyDb } from "../src/lib/cty.ts";

const db = CtyDb.parse(readFileSync(new URL("../static/cty.dat", import.meta.url), "utf8"));
const look = (c: string) => {
  const h = db.lookup(c);
  return h && { pfx: h.entity.prefix, cq: h.cq, cont: h.cont };
};

test("parses the whole file", () => {
  assert.ok(db.size > 330, `only ${db.size} entities`);
});

test("ordinary calls", () => {
  assert.deepEqual(look("UR5ZZ"), { pfx: "UR", cq: 16, cont: "EU" });
  assert.deepEqual(look("JA1ABC"), { pfx: "JA", cq: 25, cont: "AS" });
  assert.equal(look("UA9AA")?.pfx, "UA9");
  assert.equal(look("UA0TA")?.cq, 18); // prefix override UA0T(18)
  assert.equal(look("VK2ABC")?.cont, "OC");
});

test("US and Canada zones by call area", () => {
  assert.deepEqual(look("K6XX"), { pfx: "K", cq: 3, cont: "NA" });
  assert.equal(look("W1AW")?.cq, 5);
  assert.equal(look("N0AX")?.cq, 4);
  assert.equal(look("AA7X")?.cq, 3);
  assert.equal(look("VE3XX")?.cq, 4);
  assert.equal(look("VE7XX")?.cq, 3);
  assert.equal(look("VE1XX")?.cq, 5);
});

test("Alaska and Hawaii are their own entities", () => {
  assert.equal(look("KL7XX")?.pfx, "KL");
  assert.equal(look("KH6XX")?.pfx, "KH6");
});

test("portable forms", () => {
  assert.equal(look("K1ABC/6")?.cq, 3);
  assert.equal(look("K1ABC/P")?.pfx, "K");
  assert.equal(look("W1AW/KH6")?.pfx, "KH6");
  assert.equal(look("VP2E/K1ABC")?.pfx, "VP2E");
  assert.equal(look("DL1ABC/MM"), null);
});

test("WAE entities are flagged", () => {
  const h = db.lookup("IT9ABC");
  assert.ok(h && h.entity.wae, "IT9 should be WAE");
  assert.equal(h!.entity.prefix, "IT9");
});

test("WL_CTY template lines are skipped", () => {
  const wl = CtyDb.parse("United States: 05: 08: NA: 37.6: 91.87: 5.0: K:\n#   K: K6(3)[6];\n    K,N,W;\n");
  assert.equal(wl.lookup("K6XX")?.cq, 3);
  assert.equal(wl.lookup("W1XX")?.cq, 5);
});
