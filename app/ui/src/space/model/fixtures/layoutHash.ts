// Every position in a catalogue as exact bits, hashed: the cross-machine
// check of docs/PLAN.md S4.1. If any engine rounds any step differently,
// it prints another hash.

import { fnv1a64 } from '../../../shared/dmath';
import { hexBits, trigFingerprint } from '../../../shared/dmath/fingerprint';
import type { Galaxy } from '../catalogue';
import { buildFamilies, layoutFamilies, type LineageNode } from '../lineage';
import { placeGalaxies, placeSystems } from '../orbits';
import { placeSystem, systemOrbits } from '../system';
import { GALAXIES, LINEAGE } from './catalogue';

export const LAYOUT_HASH = '06743e118731ed46';
export const LAYOUT_HASH_LATER = '737e5ace7f5e28e2';

export function layoutText(galaxies: Galaxy[], lineage: { roots: string[]; nodes: LineageNode[] }, t: number): string {
  const b = hexBits;
  const lines: string[] = [];
  for (const g of placeGalaxies(galaxies)) lines.push(`universe ${g.id} ${b(g.x)} ${b(g.y)}`);
  for (const galaxy of [...galaxies].sort((x, y) => x.id - y.id)) {
    for (const s of placeSystems(galaxy)) {
      const dots = s.dots.map((d) => `${b(d.x)},${b(d.y)}`).join(' ');
      lines.push(`galaxy ${galaxy.id} ${s.id} ${b(s.x)} ${b(s.y)} ${dots}`);
    }
    for (const system of galaxy.systems) {
      const placed = placeSystem(systemOrbits(system), t);
      for (const w of placed.worlds) {
        lines.push(`world ${system.id} ${w.id} ${w.seat} ${b(w.x)} ${b(w.y)} ${b(w.z)} ${b(w.radius)}`);
      }
      const o = placed.overflow;
      if (o) {
        const ids = o.worlds.map((w) => w.id).join(',');
        lines.push(`gathered ${system.id} ${ids} ${b(o.x)} ${b(o.y)} ${b(o.z)} ${b(o.radius)}`);
      }
    }
  }
  for (const n of layoutFamilies(buildFamilies(lineage.roots, lineage.nodes)).nodes) {
    lines.push(`tree ${n.trackId} ${b(n.x)} ${b(n.y)} ${b(n.radius)}`);
  }
  return lines.join('\n');
}

export function layoutHash(galaxies: Galaxy[], lineage: { roots: string[]; nodes: LineageNode[] }, t: number): string {
  return fnv1a64(layoutText(galaxies, lineage, t)).toString(16).padStart(16, '0');
}

// What a browser computes for the fixed catalogue, for e2e/space-layout.
export function hashesHere(): { layout: string; later: string; trig: string } {
  return {
    layout: layoutHash(GALAXIES, LINEAGE, 0),
    later: layoutHash(GALAXIES, LINEAGE, 600.25),
    trig: trigFingerprint(),
  };
}
