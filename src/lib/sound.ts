// Small UI sounds, synthesised with WebAudio (no asset files).

let ctx: AudioContext | null = null;

// One bell strike: a few inharmonic partials with a fast attack and a long
// ring-out, which carries over band noise far better than a pure sine blip.
function strike(ac: AudioContext, out: AudioNode, f: number, t: number, level: number) {
  const partials: [number, number, number][] = [
    // [frequency ratio, relative level, decay seconds]
    [1, 1, 1.4],
    [2, 0.5, 0.9],
    [2.76, 0.35, 0.6],
    [5.4, 0.2, 0.3],
  ];
  for (const [ratio, rel, decay] of partials) {
    const o = ac.createOscillator();
    const g = ac.createGain();
    o.type = "sine";
    o.frequency.value = f * ratio;
    g.gain.setValueAtTime(0.0001, t);
    g.gain.exponentialRampToValueAtTime(level * rel, t + 0.005);
    g.gain.exponentialRampToValueAtTime(0.0001, t + decay);
    o.connect(g).connect(out);
    o.start(t);
    o.stop(t + decay + 0.05);
  }
}

/// A two-strike rising bell — the "new multiplier" reward.
export function chime() {
  try {
    ctx ??= new AudioContext();
    if (ctx.state === "suspended") void ctx.resume();
    const t0 = ctx.currentTime + 0.01;
    // A compressor keeps the stacked partials loud without clipping.
    const comp = ctx.createDynamicsCompressor();
    comp.connect(ctx.destination);
    strike(ctx, comp, 1047, t0, 0.5); // C6
    strike(ctx, comp, 1568, t0 + 0.16, 0.5); // G6
  } catch {
    // No audio output available — silently skip.
  }
}
