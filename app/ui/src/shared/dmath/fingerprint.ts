import { fnv1a64 } from './fnv';
import { atan2, cos, sin, sqrt } from './trig';

// The exact bits of sin, cos, atan2 and sqrt over a fixed grid, hashed.
// An engine that rounds any one of them differently prints another value.
// The unit test pins it in Node; e2e/space-layout pins it in the browser.
export const TRIG_FINGERPRINT = 'b544580f58916ce7';

export function hexBits(x: number): string {
  const view = new DataView(new ArrayBuffer(8));
  view.setFloat64(0, x);
  return view.getBigUint64(0).toString(16).padStart(16, '0');
}

export function trigFingerprint(): string {
  const out: string[] = [];
  for (let i = -2000; i <= 2000; i++) {
    const x = i * 0.0137 + i * i * 1e-4;
    out.push(hexBits(sin(x)), hexBits(cos(x)), hexBits(atan2(x, 1 - i * 0.003)), hexBits(sqrt(Math.abs(x))));
  }
  return fnv1a64(out.join(',')).toString(16).padStart(16, '0');
}
