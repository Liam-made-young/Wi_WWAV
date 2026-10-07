// What a work is, in words: the fact line on a card and in a label, and
// its one verb (docs/SPEC.md 4.10). No "N seen", ever.

import type { Medium, WorkFacts } from './catalogue';

const NAMES: Record<Medium, string> = { song: 'Song', film: 'Film', writing: 'Writing', fashion: 'Fashion' };
const VERBS: Record<Medium, string> = { song: 'Play', film: 'Watch', writing: 'Read', fashion: 'Look' };

export function verbFor(medium: Medium): string {
  return VERBS[medium];
}

// "1,200"
export function grouped(n: number): string {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, ',');
}

// "3:58", "1:02:07"
function runningTime(seconds: number): string {
  const s = Math.round(seconds);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const ss = String(s % 60).padStart(2, '0');
  return h > 0 ? `${h}:${String(m).padStart(2, '0')}:${ss}` : `${m}:${ss}`;
}

function counted(n: number, one: string, many: string): string {
  return `${grouped(n)} ${n === 1 ? one : many}`;
}

export function factParts(f: WorkFacts): string[] {
  const parts = [NAMES[f.medium]];
  switch (f.medium) {
    case 'song':
      if (f.key) parts.push(f.key);
      if (f.bpm) parts.push(`${Math.round(f.bpm)} BPM`);
      break;
    case 'film':
      if (f.durationSeconds) parts.push(runningTime(f.durationSeconds));
      break;
    case 'writing':
      if (f.words != null) parts.push(counted(f.words, 'word', 'words'));
      break;
    case 'fashion':
      if (f.photos != null) parts.push(counted(f.photos, 'photo', 'photos'));
      break;
  }
  return parts;
}

// "Song · A minor · 128 BPM"
export function cardFacts(f: WorkFacts): string {
  return factParts(f).join(' · ');
}
