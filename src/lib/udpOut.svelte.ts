// QSOs broadcast over UDP to other software — loggers (World Radio League,
// Log4OM…), GridTracker, JTAlert, N1MM-aware tools — in WSJT-X and/or N1MM
// format (see udp_out.rs). Hooks qsoLog.onChange: a QSO logged by hand, an
// edit in the Logbook and a deleted QSO go out; bulk changes (ADIF import,
// Clear, switching logs) don't.

import { invoke } from "@tauri-apps/api/core";
import { settings } from "./settings.svelte";
import { qsoLog } from "./qsoLog.svelte";
import { cty } from "./ctyStore.svelte";
import { scoreStore } from "./score.svelte";
import { CONTESTS } from "./contests";
import { parseExchange, rulesFor, wpxPrefix } from "./scoring";
import { toAdif } from "./exports";
import type { Qso } from "./types";

class UdpOut {
  /// Result of the last send, for Settings: what went where, or why not.
  lastStatus = $state<string>("");
  lastError = $state<boolean>(false);
  private started = false;

  init() {
    if (this.started) return;
    this.started = true;
    qsoLog.onChange = (kind, qso) => this.send(kind, qso);
  }

  /// Push the Settings to the backend (on load and on every change).
  async configure() {
    try {
      await invoke("udp_set_config", {
        config: {
          wsjtxEnabled: settings.udpWsjtx,
          wsjtxAddrs: settings.udpWsjtxAddrs,
          n1mmEnabled: settings.udpN1mm,
          n1mmAddrs: settings.udpN1mmAddrs,
        },
      });
    } catch (e) {
      console.error("udp_set_config failed", e);
    }
  }

  private send(kind: "add" | "replace" | "delete", q: Qso) {
    if (!settings.udpWsjtx && !settings.udpN1mm) return;
    // Built now, synchronously: for an "add" the log doesn't hold the QSO
    // yet, so the multiplier check is against the QSOs before it.
    const qso = this.describe(kind, q);
    invoke("udp_send_qso", { event: kind, qso })
      .then(() => {
        this.lastError = false;
        this.lastStatus = `${q.call} ${kind === "add" ? "sent" : kind === "replace" ? "edit sent" : "delete sent"} ${new Date().toISOString().slice(11, 19)}Z`;
      })
      .catch((e) => {
        this.lastError = true;
        this.lastStatus = `${q.call}: ${e}`;
      });
  }

  private describe(kind: string, q: Qso) {
    const contestId = settings.activeContest;
    const hit = cty.lookup(q.call);
    const p = parseExchange(contestId, q.exchRcvd, hit);
    const zone = p.zone ?? hit?.cq ?? 0;
    const points = rulesFor(contestId).points(scoreStore.my, hit, q.band, p, { myGrid: settings.myGrid });
    const isMult = kind === "add" && scoreStore.newMults(q.call, q.band, q.exchRcvd).length > 0;
    return {
      id: q.id,
      ts: q.ts,
      call: q.call,
      freqHz: Math.round(q.freqHz),
      band: q.band,
      mode: "RTTY",
      rstSent: q.rstSent || "599",
      rstRcvd: q.rstRcvd || "599",
      exchSent: q.exchSent,
      exchRcvd: q.exchRcvd,
      serialSent: q.serialSent,
      serialRcvd: p.serial ?? null,
      myCall: settings.myCall,
      myGrid: settings.myGrid,
      dxGrid: p.grid ?? "",
      name: p.name ?? "",
      comment: q.note ?? "",
      countryPrefix: hit?.entity.prefix ?? "",
      wpxPrefix: wpxPrefix(q.call),
      continent: hit?.cont ?? "",
      zone,
      points,
      isMult,
      run: !settings.spMode,
      contest: CONTESTS.find((c) => c.id === contestId)?.cabrilloName || "RTTY",
      adif: toAdif([q]),
    };
  }
}

export const udpOut = new UdpOut();
