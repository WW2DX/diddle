// Tiny cross-component bus for sending a callsign into the Entry window.
// The decoder window (and anything else that surfaces a callsign) calls
// `setCall()`; EntryWindow watches `token` and copies the call into its
// Call field. `token` increments on every request so clicking the same
// call twice still re-fires.

class EntryBus {
  requestedCall = $state<string>("");
  token = $state<number>(0);

  setCall(c: string) {
    this.requestedCall = c;
    this.token++;
  }
}

export const entryBus = new EntryBus();
