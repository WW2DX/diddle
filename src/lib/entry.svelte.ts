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

  /// Which entry field takes focus once the call is loaded.
  requestedFocus: "call" | "exch" = "exch";

  setCall(c: string, focus: "call" | "exch" = "exch") {
    this.requestedCall = c;
    this.requestedFocus = focus;
    this.token++;
  }

  /// The stack (N1MM/WriteLog "NEXT" callers): right-click callers while
  /// working one; the first goes straight into Call if it's empty, the
  /// rest wait here in the order clicked. In Run, ESM's TU step sends the
  /// stack-TU macro (F8 by default), whose <LOGIT> logs the QSO and whose
  /// <POPSTACK> loads the next caller for the rest of the message.
  nextQueue = $state<string[]>([]);
  /// Bumped on every right-click so EntryWindow takes keyboard focus back
  /// from the RX window / waterfall — otherwise Enter would land on the
  /// call chip that was just right-clicked.
  queueToken = $state<number>(0);

  queueNext(c: string) {
    const call = c.trim().toUpperCase();
    if (!call) return;
    if (!this.currentCall.trim()) {
      this.setCall(call, "call");
      return;
    }
    this.queueToken++;
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

  clearNext() {
    this.nextQueue = [];
  }

  /// Hooks EntryWindow installs for the <LOGIT> and <POPSTACK> macro
  /// tokens, which act on the entry form mid-expansion.
  /// logIt: log the QSO in the form; true if it was logged.
  /// popStack: load the next stacked caller as a station we've sent our
  /// exchange to; returns his call, or undefined when the stack is empty.
  actions: { logIt(): boolean; popStack(): string | undefined } | null = null;

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
