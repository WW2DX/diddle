import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { CtyDb } from "../src/lib/cty.ts";
import { parseExchange, scoreLog, wpxPrefix, MultTracker, gridDistanceKm, makrothenPoints } from "../src/lib/scoring.ts";

const db = CtyDb.parse(readFileSync(new URL("../static/cty.dat", import.meta.url), "utf8"));
const lookup = (c: string) => db.lookup(c);
const me = db.lookup("W1AW");

test("WPX prefixes", () => {
  for (const [c, p] of [
    ["K1ABC", "K1"], ["9A1A", "9A1"], ["4X4AA", "4X4"], ["3DA0RU", "3DA0"],
    ["K1ABC/4", "K4"], ["K1ABC/P", "K1"], ["VP2E/K1ABC", "VP2E"], ["W1AW/KH6", "KH6"],
    ["RAEM", "RA0"], ["OH2AQ", "OH2"],
  ]) assert.equal(wpxPrefix(c), p, c);
});

test("CQ WW exchange parsing", () => {
  assert.deepEqual(parseExchange("cqww-rtty", "599 05 NY", db.lookup("K2XX")), { region: "NY", zone: 5, dx: false });
  assert.deepEqual(parseExchange("cqww-rtty", "14", db.lookup("DL1ABC")), { zone: 14, dx: true });
  assert.equal(parseExchange("cqww-rtty", "04 PQ", db.lookup("VE2XX")).region, "QC");
});

test("CQ WW score: zones, countries, states per band; 1/2/3 points", () => {
  const log = [
    { call: "DL1ABC", band: "20m", exchRcvd: "14" },  // 3 pts, zone 14, DL
    { call: "K6XX", band: "20m", exchRcvd: "03 CA" }, // 1 pt, zone 3, K, CA
    { call: "VE3XX", band: "20m", exchRcvd: "04 ON" },// 2 pts, zone 4, VE, ON
    { call: "DL2ABC", band: "20m", exchRcvd: "14" },  // 3 pts, nothing new
    { call: "DL1ABC", band: "20m", exchRcvd: "14" },  // dupe
    { call: "DL1ABC", band: "40m", exchRcvd: "14" },  // 3 pts, new on 40
  ];
  const s = scoreLog("cqww-rtty", log, lookup, me);
  assert.equal(s.qsos, 5);
  assert.equal(s.dupes, 1);
  assert.equal(s.points, 3 + 1 + 2 + 3 + 3);
  assert.deepEqual(s.mults, { zone: 4, country: 4, region: 2 });
  assert.equal(s.score, 12 * 10);
  assert.deepEqual(s.bands.map((b) => b.band), ["40m", "20m"]);
});

test("new-mult prediction, and the zone comes from the country file", () => {
  const t = new MultTracker("cqww-rtty", lookup, me);
  assert.deepEqual(t.newMults({ call: "UR5ZZ", band: "20m", exchRcvd: "" }), ["Zone 16", "Ukraine"]);
  t.add({ call: "UR5ZZ", band: "20m", exchRcvd: "16" });
  assert.deepEqual(t.newMults({ call: "UT1AA", band: "20m", exchRcvd: "" }), []);
  assert.deepEqual(t.newMults({ call: "UT1AA", band: "15m", exchRcvd: "" }), ["Zone 16", "Ukraine"]);
});

test("Roundup: states/provinces/DXCC once per contest, 1 point each", () => {
  const s = scoreLog("rtty-roundup", [
    { call: "K2XX", band: "20m", exchRcvd: "NY" },
    { call: "K2YY", band: "40m", exchRcvd: "NY" },
    { call: "DL1ABC", band: "20m", exchRcvd: "001" },
    { call: "IT9ABC", band: "20m", exchRcvd: "002" }, // WAE → counts as Italy
    { call: "I1ABC", band: "15m", exchRcvd: "003" },
  ], lookup, me);
  assert.equal(s.points, 5);
  assert.deepEqual(s.mults, { region: 1, country: 2 });
  assert.equal(s.score, 15);
});

test("WPX: prefixes once, low bands double", () => {
  const s = scoreLog("wpx-rtty", [
    { call: "DL1ABC", band: "40m", exchRcvd: "001" }, // diff continent low: 6
    { call: "K1ABC", band: "20m", exchRcvd: "002" },  // same continent: 1
    { call: "DL1XYZ", band: "20m", exchRcvd: "003" }, // 3, DL1 already
  ], lookup, me);
  assert.equal(s.points, 10);
  assert.deepEqual(s.mults, { prefix: 2 });
  assert.equal(s.score, 20);
});

test("Makrothen: grid distance, band factors, same square", () => {
  // The worked example from the rules: CM87 ↔ EL49.
  const d = gridDistanceKm("CM87", "EL49")!;
  assert.ok(Math.abs(d - 3084.2234824787) < 1e-6, String(d));
  assert.equal(makrothenPoints("CM87", "EL49", "20m"), 3084);
  assert.equal(makrothenPoints("CM87", "EL49", "15m"), 3084);
  assert.equal(makrothenPoints("CM87", "EL49", "40m"), 4626);
  assert.equal(makrothenPoints("CM87", "EL49", "80m"), 6168);
  // 6-character locators count as their square; same square is 100 flat.
  assert.equal(makrothenPoints("cm87wj", "CM87", "80m"), 100);
  // No grid, or off the contest bands: nothing.
  assert.equal(makrothenPoints("CM87", "", "20m"), 0);
  assert.equal(makrothenPoints("CM87", "EL49", "160m"), 0);

  // Whole-log score: points summed, no multipliers, one QSO per band.
  const s = scoreLog(
    "makrothen-rtty",
    [
      { call: "K4XYZ", band: "20m", exchRcvd: "EL49" },
      { call: "K4XYZ", band: "40m", exchRcvd: "EL49" },
      { call: "K4XYZ", band: "40m", exchRcvd: "EL49" }, // dupe
    ],
    () => null,
    null,
    { myGrid: "CM87" },
  );
  assert.equal(s.points, 3084 + 4626);
  assert.equal(s.score, 3084 + 4626);
  assert.equal(s.dupes, 1);
});
