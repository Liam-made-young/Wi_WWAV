/// <reference types="node" />
// Plain 44.1 kHz, 16-bit stereo WAVs of sine tones: the master and stems a
// test song is packed from.

import { writeFileSync } from 'node:fs';

/** `tones` are [Hz, amplitude] pairs, summed. */
export function writeWav(path: string, tones: [number, number][], seconds: number) {
  const rate = 44_100;
  const frames = Math.round(rate * seconds);
  const data = Buffer.alloc(frames * 4);
  for (let i = 0; i < frames; i++) {
    let v = 0;
    for (const [hz, amp] of tones) v += Math.sin((2 * Math.PI * hz * i) / rate) * amp;
    const s = Math.max(-32768, Math.min(32767, Math.round(v * 32767)));
    data.writeInt16LE(s, i * 4);
    data.writeInt16LE(s, i * 4 + 2);
  }
  const head = Buffer.alloc(44);
  head.write('RIFF', 0);
  head.writeUInt32LE(36 + data.length, 4);
  head.write('WAVE', 8);
  head.write('fmt ', 12);
  head.writeUInt32LE(16, 16);
  head.writeUInt16LE(1, 20);
  head.writeUInt16LE(2, 22);
  head.writeUInt32LE(rate, 24);
  head.writeUInt32LE(rate * 4, 28);
  head.writeUInt16LE(4, 32);
  head.writeUInt16LE(16, 34);
  head.write('data', 36);
  head.writeUInt32LE(data.length, 40);
  writeFileSync(path, Buffer.concat([head, data]));
}
