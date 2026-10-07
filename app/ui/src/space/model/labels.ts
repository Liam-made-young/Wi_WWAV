// What the DOM labels show at each zoom, and what VoiceOver reads
// (docs/SPEC.md 4.4, 4.5). Text stays flat and in the DOM; this decides
// which text and in what order.

import type { World } from './catalogue';
import { factParts } from './facts';
import type { PlacedSystem } from './system';

export interface LabelTier {
  show: 'dots' | 'names' | 'titles';
  artistLine: boolean;
}

// v3's levels (CosmosRenderer.swift): dots with haloed suns under 0.35,
// orbs and system names to 0.9, every title above, the artist line from 1.6.
export function labelTier(k: number): LabelTier {
  const show = k < 0.35 ? 'dots' : k < 0.9 ? 'names' : 'titles';
  return { show, artistLine: k >= 1.6 };
}

// "LMY · gen 2"
export function artistLine(artist: string, generation: number): string {
  return `${artist} · gen ${generation}`;
}

// "Low Tide. Song, A minor, 86 BPM. World 3 of 12 in World Ending."
// `index` is the world's place in orbit order, from 0.
export function worldLabel(world: World, index: number, total: number, systemTitle: string): string {
  return `${world.title}. ${factParts(world).join(', ')}. World ${index + 1} of ${total} in ${systemTitle}.`;
}

export function gatheredLabel(count: number, systemTitle: string): string {
  return `${count} more ${count === 1 ? 'world' : 'worlds'}, gathered. In ${systemTitle}.`;
}

// Tab's order through a system: its sun, its worlds by seat (ring by ring,
// round each ring), then the gathered world.
export function orbitOrder(placed: PlacedSystem): string[] {
  const worlds = [...placed.worlds].sort((a, b) => a.seat - b.seat).map((w) => `world:${w.id}`);
  return ['sun', ...worlds, ...(placed.overflow ? ['gathered'] : [])];
}

// The next body in orbit order. Tab passes the end and leaves the sky
// (null); the arrows go round.
export function nextInOrbit(order: string[], current: string | null, step: 1 | -1, wrap: boolean): string | null {
  if (order.length === 0) return null;
  if (current === null) return step > 0 ? order[0] : order[order.length - 1];
  const next = order.indexOf(current) + step;
  if (next >= 0 && next < order.length) return order[next];
  return wrap ? order[(next + order.length) % order.length] : null;
}
