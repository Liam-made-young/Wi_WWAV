// Heat's chime (docs/SPEC.md 3.5, 7.7): one struck bell with a 1.2 s decay
// when a focus round ends, off by default, with its own switch. It sounds
// only for a round the person started, never to call them back from a
// break, and the app makes no other sound of its own.

const KEY = 'wi.heat.chime';

/** Whether the person turned the chime on. Off until they do. */
export function chimeOn(): boolean {
  try {
    return localStorage.getItem(KEY) === 'on';
  } catch {
    return false;
  }
}

export function setChime(on: boolean): void {
  try {
    localStorage.setItem(KEY, on ? 'on' : 'off');
  } catch {
    // Private storage refused: the chime stays off for this run.
  }
}

/** A struck bell: a few inharmonic partials, each fading over 1.2 s. */
export function ringChime(): void {
  const Context =
    window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Context) return;
  const ctx = new Context();
  const out = ctx.createGain();
  out.gain.value = 0.25;
  out.connect(ctx.destination);
  const t = ctx.currentTime;
  for (const [ratio, level] of [
    [1, 1],
    [2.76, 0.5],
    [5.4, 0.25],
    [8.93, 0.12],
  ] as const) {
    const osc = ctx.createOscillator();
    const gain = ctx.createGain();
    osc.type = 'sine';
    osc.frequency.value = 660 * ratio;
    gain.gain.setValueAtTime(level, t);
    gain.gain.exponentialRampToValueAtTime(0.0001, t + 1.2);
    osc.connect(gain).connect(out);
    osc.start(t);
    osc.stop(t + 1.3);
  }
  setTimeout(() => void ctx.close(), 1500);
}
