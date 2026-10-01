// Small UI sounds, synthesised with WebAudio (no asset files).

let ctx: AudioContext | null = null;

/// A short rising two-note chime — the "new multiplier" reward.
export function chime() {
  try {
    ctx ??= new AudioContext();
    const t0 = ctx.currentTime;
    for (const [i, f] of [880, 1320].entries()) {
      const o = ctx.createOscillator();
      const g = ctx.createGain();
      o.type = "sine";
      o.frequency.value = f;
      const t = t0 + i * 0.12;
      g.gain.setValueAtTime(0.0001, t);
      g.gain.exponentialRampToValueAtTime(0.18, t + 0.01);
      g.gain.exponentialRampToValueAtTime(0.0001, t + 0.18);
      o.connect(g).connect(ctx.destination);
      o.start(t);
      o.stop(t + 0.2);
    }
  } catch {
    // No audio output available — silently skip.
  }
}
