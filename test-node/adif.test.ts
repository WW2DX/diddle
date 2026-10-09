// ADIF import: Diddle's own export round-trips, other loggers' files load.
import { test } from "node:test";
import assert from "node:assert/strict";
import { importAdif, parseAdifRecords } from "../src/lib/adifImport.ts";

const band = (hz: number) => (hz >= 14_000_000 && hz < 14_500_000 ? "20m" : hz >= 7_000_000 && hz < 7_300_000 ? "40m" : "?");

test("reads Diddle's own export, exchange and serial intact", () => {
  const adi = `ADIF export from Diddle
<ADIF_VER:5>3.1.4
<PROGRAMID:6>Diddle
<EOH>

<CALL:5>UR5ZZ <QSO_DATE:8>20261002 <TIME_ON:4>1803 <BAND:3>20M <FREQ:9>14.085400 <MODE:4>RTTY <RST_SENT:3>599 <RST_RCVD:3>599 <STX_STRING:3>005 <SRX_STRING:6>012 KR <APP_DIDDLE_SERIAL:1>5 <COMMENT:7>rig QRM <EOR>
<CALL:4>W1AW <QSO_DATE:8>20261002 <TIME_ON:6>180130 <FREQ:8>7.085000 <MODE:4>RTTY <STX_STRING:3>004 <SRX_STRING:2>CT <APP_DIDDLE_SERIAL:1>4 <EOR>
`;
  const r = importAdif(adi, band);
  assert.equal(r.skipped, 0);
  assert.equal(r.qsos.length, 2);
  // Sorted by time: W1AW first.
  const [a, b] = r.qsos;
  assert.equal(a.call, "W1AW");
  assert.equal(a.band, "40m");
  assert.equal(a.ts, Date.UTC(2026, 9, 2, 18, 1, 30));
  assert.equal(a.serialSent, 4);
  assert.equal(b.call, "UR5ZZ");
  assert.equal(b.freqHz, 14_085_400);
  assert.equal(b.band, "20m");
  assert.equal(b.exchRcvd, "012 KR");
  assert.equal(b.exchSent, "005");
  assert.equal(b.serialSent, 5);
  assert.equal(b.note, "rig QRM");
});

test("other loggers: lower-case tags, type indicators, serial-only fields", () => {
  const adi = `<eoh><call:6:S>DL1ABC<qso_date:8>20261002<time_on:4>0912<band:3>40m<mode:4>RTTY<stx:2>17<srx:3>233<contest_id:10>CQ-WW-RTTY<eor>
<call:0><qso_date:8>20261002<time_on:4>0913<eor>`;
  const r = importAdif(adi, band);
  assert.equal(r.qsos.length, 1);
  assert.equal(r.skipped, 1);
  assert.equal(r.contestId, "CQ-WW-RTTY");
  const q = r.qsos[0];
  assert.equal(q.call, "DL1ABC");
  assert.equal(q.band, "40m");
  assert.equal(q.exchRcvd, "233");
  assert.equal(q.serialSent, 17);
});

test("field values may contain '<' and spaces", () => {
  const recs = parseAdifRecords("<EOH><COMMENT:9>a <b> c d<CALL:3>K1A<EOR>");
  assert.equal(recs[0].COMMENT, "a <b> c d");
  assert.equal(recs[0].CALL, "K1A");
});

test("QSOs without a serial are numbered after the rest", () => {
  const adi = `<EOH><CALL:3>K1A<QSO_DATE:8>20261002<TIME_ON:4>1000<EOR><CALL:3>K2B<QSO_DATE:8>20261002<TIME_ON:4>0900<APP_DIDDLE_SERIAL:1>7<EOR>`;
  const r = importAdif(adi, band);
  assert.deepEqual(r.qsos.map((q) => [q.call, q.serialSent]), [["K2B", 7], ["K1A", 8]]);
});

test("exchange built from per-part fields when there's no SRX_STRING", () => {
  const ru = `<EOH><CALL:4>W1AW<QSO_DATE:8>20250104<TIME_ON:4>1800<CONTEST_ID:9>ARRL-RTTY<STATE:2>CT<EOR>
<CALL:5>DL1AB<QSO_DATE:8>20250104<TIME_ON:4>1801<CONTEST_ID:9>ARRL-RTTY<SRX:3>123<EOR>
<CALL:5>VE3XX<QSO_DATE:8>20250104<TIME_ON:4>1802<CONTEST_ID:9>ARRL-RTTY<VE_PROV:2>ON<EOR>`;
  assert.deepEqual(importAdif(ru, band).qsos.map((q) => q.exchRcvd), ["CT", "123", "ON"]);
  const ww = `<EOH><CALL:4>K1AR<QSO_DATE:8>20250927<TIME_ON:4>0001<CONTEST_ID:10>CQ-WW-RTTY<CQZ:1>5<STATE:2>MA<EOR>`;
  assert.equal(importAdif(ww, band).qsos[0].exchRcvd, "5 MA");
  const nq = `<EOH><CALL:4>K1AR<QSO_DATE:8>20250927<TIME_ON:4>0001<CONTEST_ID:9>NAQP-RTTY<NAME:4>JOHN<STATE:2>MA<EOR>`;
  assert.equal(importAdif(nq, band).qsos[0].exchRcvd, "JOHN MA");
});
