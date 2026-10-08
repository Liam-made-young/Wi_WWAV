// The Focus layout's one sound (docs/FOCUS.md): a soft click on Done and on
// starting the timer, like a switch reaching its stop. Off until it is
// switched on in Settings → Appearance.

import { clicksOn } from './layout';

/** A short, quiet tick: a few milliseconds of a falling tone. */
export function click(): void {
  if (!clicksOn()) return;
  const Context =
    window.AudioContext ?? (window as unknown as { webkitAudioContext?: typeof AudioContext }).webkitAudioContext;
  if (!Context) return;
  const ctx = new Context();
  const t = ctx.currentTime;
  const osc = ctx.createOscillator();
  const gain = ctx.createGain();
  osc.type = 'triangle';
  osc.frequency.setValueAtTime(1800, t);
  osc.frequency.exponentialRampToValueAtTime(600, t + 0.03);
  gain.gain.setValueAtTime(0.12, t);
  gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.05);
  osc.connect(gain).connect(ctx.destination);
  osc.start(t);
  osc.stop(t + 0.06);
  setTimeout(() => void ctx.close(), 200);
}
