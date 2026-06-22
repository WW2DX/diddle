// Tiny cross-component bus for the Entry window's callsign.
//
// Inbound: the decoder window, waterfall labels, and cluster spots call
// `setCall()` to load a call; EntryWindow watches `token` and copies it into
// its Call field. `token` increments per request so clicking the same call
// twice still re-fires.
//
// Outbound: EntryWindow mirrors its live Call field into `currentCall` so
// macros fired from the F-keys (when ESM is off, with no per-QSO context)
// can still expand <CALL>.

class EntryBus {
  requestedCall = $state<string>("");
  token = $state<number>(0);
  currentCall = $state<string>("");

  setCall(c: string) {
    this.requestedCall = c;
    this.token++;
  }
}

export const entryBus = new EntryBus();
