// The player's waveform (docs/SPEC.md 4.6, after web v4's combinePeaks.js):
// 400 buckets drawn from only the audible stems, "mute the drums and the
// drum shape vanishes", against a normaliser fixed to the full mix, so a
// mute takes that stem's share away and moves nothing else.

import { audible, type Mix } from './mix';
import { STEMS, type Stem } from './stems';

export const WAVE_BUCKETS = 400;
// The full mix every stem is measured against: all four at web v4's
// default level.
const FULL_MIX_LEVEL = 0.8;

export type StemPeaks = Partial<Record<Stem, Float32Array>>;

// The loudest sample in each bucket across the first two channels, scaled
// to the stem's own loudest so a quiet stem still reads as a shape.
export function computePeaks(channels: Float32Array[], buckets = WAVE_BUCKETS): Float32Array {
  const used = channels.slice(0, 2);
  const length = used.length === 0 ? 0 : used[0].length;
  const per = Math.max(1, Math.floor(length / buckets));
  const peaks = new Float32Array(buckets);
  let loudest = 0;
  for (let b = 0; b < buckets; b++) {
    let peak = 0;
    for (const ch of used) {
      for (let i = b * per; i < Math.min(b * per + per, length); i++) peak = Math.max(peak, Math.abs(ch[i]));
    }
    peaks[b] = peak;
    loudest = Math.max(loudest, peak);
  }
  if (loudest > 0) for (let b = 0; b < buckets; b++) peaks[b] /= loudest;
  return peaks;
}

// The loudest bucket of the full mix, worked out once per song.
export function fullMixNormaliser(peaks: StemPeaks): number {
  let loudest = 0;
  for (let b = 0; b < WAVE_BUCKETS; b++) {
    let sum = 0;
    for (const s of STEMS) sum += FULL_MIX_LEVEL * (peaks[s]?.[b] ?? 0);
    loudest = Math.max(loudest, sum);
  }
  return loudest || 1;
}

export function waveform(mix: Mix, peaks: StemPeaks, normaliser: number): Float32Array {
  const out = new Float32Array(WAVE_BUCKETS);
  const sounding = audible(mix);
  for (const s of STEMS) {
    const p = peaks[s];
    if (!sounding[s] || !p) continue;
    for (let b = 0; b < WAVE_BUCKETS; b++) out[b] += mix[s].level * p[b];
  }
  for (let b = 0; b < WAVE_BUCKETS; b++) out[b] = Math.min(1, out[b] / normaliser);
  return out;
}
