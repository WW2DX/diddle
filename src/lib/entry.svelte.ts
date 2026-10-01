// Tiny cross-component bus for the Entry window's callsign.
//
// Inbound: the decoder window, waterfall labels, and cluster spots call
// `setCall()` to load a call; EntryWindow watches `token` and copies it into
// its Call field. `token` increments per request so clicking the same call
// twice still re-fires.
//
// Exchange words clicked in the decoder window arrive the same way through
// `addExchWord()` / `exchToken`; EntryWindow folds each into its Exch field.
//
// Outbound: EntryWindow mirrors its live Call field into `currentCall` so
// macros fired from the F-keys (when ESM is off, with no per-QSO context)
// can still expand <CALL>.

class EntryBus {
  requestedCall = $state<string>("");
  token = $state<number>(0);
  currentCall = $state<string>("");
  requestedExchWord = $state<string>("");
  exchToken = $state<number>(0);

  setCall(c: string) {
    this.requestedCall = c;
    this.token++;
  }

  /// NEXT queue (N1MM/WriteLog style): right-click callers while working
  /// one; the first goes straight into Call if it's empty, the rest wait
  /// here in the order clicked. ESM's TU step sends TU, logs, loads the
  /// next one and sends it the exchange — one Enter per QSO.
  nextQueue = $state<string[]>([]);

  queueNext(c: string) {
    const call = c.trim().toUpperCase();
    if (!call) return;
    if (!this.currentCall.trim()) {
      this.setCall(call);
      return;
    }
    if (call === this.currentCall.trim().toUpperCase() || this.nextQueue.includes(call)) return;
    this.nextQueue = [...this.nextQueue, call];
  }

  popNext(): string | undefined {
    const [first, ...rest] = this.nextQueue;
    this.nextQueue = rest;
    return first;
  }

  dropNext(c: string) {
    this.nextQueue = this.nextQueue.filter((x) => x !== c);
  }

  /// The TU macro (F3) was sent by hand — EntryWindow logs the QSO in Run
  /// mode with ESM on, like the ESM Enter step does.
  tuToken = $state<number>(0);
  tuSent() {
    this.tuToken++;
  }

  addExchWord(w: string) {
    this.requestedExchWord = w;
    this.exchToken++;
  }
}

export const entryBus = new EntryBus();
