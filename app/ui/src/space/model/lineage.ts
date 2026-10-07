// "Lineage is the only social graph" (docs/SPEC.md 4.9). Fork edges are
// facts the machine declared; links are claims a person made, and one that
// touches someone else's work waits for them to agree.
//
// The family tree is v3's Cosmos (`ios_v3/WWAV/Models/LineageForest.swift`,
// `Views/Cosmos/CosmosLayout.swift`): the root at the centre, generation g
// on the ring at 90 + (g − 1)·70, each subtree's arc in proportion to its
// leaves so families stay together, and families packed on a spiral.

import { cos, sin, sqrt, unitJitter } from '../../shared/dmath';
import type { Box, Point } from './orbits';

export interface LineageNode {
  trackId: string;
  parentTrackId: string | null;
  secondaryParentTrackId: string | null;
  lineageRootTrackId: string | null;
  remixDepth: number;
  createdAt: string | null;
  title: string;
  artist: string;
}

export interface Edge {
  parent: string;
  child: string;
}

// One root and everything descended from it, by generation.
export interface Family {
  id: string; // the root's track id
  root: LineageNode;
  generations: LineageNode[][];
  edges: Edge[];
  merges: Edge[];
}

// The server caps lineage depth at 64, which also guards against cycles.
const MAX_DEPTH = 64;

// Siblings by when they were made, then by id: the same on every device.
function sortKey(node: LineageNode): string {
  return `${node.createdAt ?? ''}|${node.trackId}`;
}

function bySortKey(a: LineageNode, b: LineageNode): number {
  const ka = sortKey(a);
  const kb = sortKey(b);
  return ka < kb ? -1 : ka > kb ? 1 : 0;
}

export function buildFamilies(roots: string[], nodes: LineageNode[]): Family[] {
  const byId = new Map<string, LineageNode>();
  for (const n of nodes) if (!byId.has(n.trackId)) byId.set(n.trackId, n);

  const childrenOf = new Map<string, LineageNode[]>();
  for (const n of nodes) {
    if (n.parentTrackId === null || n.parentTrackId === n.trackId) continue;
    childrenOf.set(n.parentTrackId, [...(childrenOf.get(n.parentTrackId) ?? []), n]);
  }
  for (const list of childrenOf.values()) list.sort(bySortKey);

  // The server's order first, then any parentless node it didn't list.
  const rootIds: string[] = [];
  const seen = new Set<string>();
  for (const id of roots) {
    if (byId.has(id) && !seen.has(id)) {
      seen.add(id);
      rootIds.push(id);
    }
  }
  const extra = nodes
    .filter((n) => n.parentTrackId === null && !seen.has(n.trackId))
    .map((n) => n.trackId)
    .sort();
  for (const id of extra) {
    if (!seen.has(id)) {
      seen.add(id);
      rootIds.push(id);
    }
  }

  // Breadth first from each root: distance from the root is the generation.
  const visited = new Set<string>();
  const families: Family[] = [];
  const familyOfRoot = new Map<string, number>();
  for (const rootId of rootIds) {
    const root = byId.get(rootId)!;
    if (visited.has(rootId)) continue;
    visited.add(rootId);
    const generations = [[root]];
    const edges: Edge[] = [];
    let frontier = [root];
    for (let depth = 1; frontier.length > 0 && depth <= MAX_DEPTH; depth++) {
      const next: LineageNode[] = [];
      for (const parent of frontier) {
        for (const child of childrenOf.get(parent.trackId) ?? []) {
          if (visited.has(child.trackId)) continue;
          visited.add(child.trackId);
          next.push(child);
          edges.push({ parent: parent.trackId, child: child.trackId });
        }
      }
      if (next.length > 0) generations.push(next);
      frontier = next;
    }
    familyOfRoot.set(rootId, families.length);
    families.push({ id: rootId, root, generations, edges, merges: [] });
  }

  // A fork whose parent is missing joins the family its root names, on the
  // ring its depth claims; with no such family it stands alone. Nothing is
  // lost.
  const orphans = nodes.filter((n) => !visited.has(n.trackId)).sort(bySortKey);
  const alone: LineageNode[] = [];
  for (const orphan of orphans) {
    if (visited.has(orphan.trackId)) continue;
    visited.add(orphan.trackId);
    const index = orphan.lineageRootTrackId === null ? undefined : familyOfRoot.get(orphan.lineageRootTrackId);
    if (index === undefined) {
      alone.push(orphan);
      continue;
    }
    const gens = families[index].generations;
    const gen = Math.min(Math.max(1, orphan.remixDepth), MAX_DEPTH);
    while (gens.length <= gen) gens.push([]);
    gens[gen].push(orphan);
  }
  for (const orphan of alone) {
    familyOfRoot.set(orphan.trackId, families.length);
    families.push({ id: orphan.trackId, root: orphan, generations: [[orphan]], edges: [], merges: [] });
  }

  // A second parent is drawn as a merge only when both ends share a family.
  const familyOfNode = new Map<string, number>();
  families.forEach((f, i) => f.generations.flat().forEach((n) => familyOfNode.set(n.trackId, i)));
  for (const n of [...nodes].sort(bySortKey)) {
    if (n.secondaryParentTrackId === null) continue;
    const a = familyOfNode.get(n.secondaryParentTrackId);
    if (a === undefined || a !== familyOfNode.get(n.trackId)) continue;
    families[a].merges.push({ parent: n.secondaryParentTrackId, child: n.trackId });
  }
  return families;
}

// The path from the root down to one node: "your branch wears the accent".
export function branchTo(family: Family, trackId: string): Set<string> {
  const parentOf = new Map(family.edges.map((e) => [e.child, e.parent]));
  const path = new Set<string>();
  for (let id: string | undefined = trackId; id !== undefined && !path.has(id); id = parentOf.get(id)) {
    path.add(id);
  }
  return path;
}

// --- The tree's layout -------------------------------------------------------

const ROOT_RADIUS = 34;
const NODE_RADIUS = 13;
const FIRST_RING = 90;
const RING_GAP = 70;
const FAMILY_MARGIN = 90;
const TWO_PI = 2 * Math.PI;

export interface PlacedNode extends Point {
  trackId: string;
  familyId: string;
  radius: number;
  generation: number;
}

export interface PlacedEdge {
  parent: string;
  child: string;
  from: Point;
  to: Point;
  merge: boolean; // drawn dashed
}

export interface TreeLayout {
  nodes: PlacedNode[];
  edges: PlacedEdge[];
  frames: Record<string, Box>; // per family, for a double-click to frame it
  bounds: Box;
}

interface Local {
  nodes: PlacedNode[];
  radius: number;
}

function layoutFamily(family: Family): Local {
  const childrenOf = new Map<string, string[]>();
  for (const e of family.edges) childrenOf.set(e.parent, [...(childrenOf.get(e.parent) ?? []), e.child]);
  const leaves = new Map<string, number>();
  const countLeaves = (id: string): number => {
    const known = leaves.get(id);
    if (known !== undefined) return known;
    const children = childrenOf.get(id) ?? [];
    const count = children.length === 0 ? 1 : children.reduce((sum, c) => sum + countLeaves(c), 0);
    leaves.set(id, count);
    return count;
  };
  countLeaves(family.root.trackId);

  // The whole family turns by its own angle, so neighbours don't align.
  const base = unitJitter(family.id, 'rotation') * TWO_PI;
  const span = new Map<string, [number, number]>([[family.root.trackId, [base, base + TWO_PI]]]);
  const nodes: PlacedNode[] = [
    { trackId: family.root.trackId, familyId: family.id, x: 0, y: 0, radius: ROOT_RADIUS, generation: 0 },
  ];
  let outer = ROOT_RADIUS;

  family.generations.forEach((generation, gen) => {
    if (gen === 0) return;
    const ring = FIRST_RING + (gen - 1) * RING_GAP;
    const isOrphan = (n: LineageNode) => n.parentTrackId === null || !span.has(n.parentTrackId);
    const orphanCount = Math.max(1, generation.filter(isOrphan).length);
    let orphanIndex = 0;

    for (const n of generation) {
      let mine: [number, number];
      const siblings = n.parentTrackId === null ? undefined : childrenOf.get(n.parentTrackId);
      const parentSpan = n.parentTrackId === null ? undefined : span.get(n.parentTrackId);
      if (parentSpan && siblings && siblings.includes(n.trackId)) {
        // A slice of the parent's arc, carved sibling by sibling in order.
        const total = siblings.reduce((sum, s) => sum + (leaves.get(s) ?? 1), 0);
        const width = parentSpan[1] - parentSpan[0];
        let cursor = parentSpan[0];
        mine = [cursor, cursor];
        for (const s of siblings) {
          const slice = (width * (leaves.get(s) ?? 1)) / Math.max(1, total);
          if (s === n.trackId) {
            mine = [cursor, cursor + slice];
            break;
          }
          cursor += slice;
        }
      } else {
        // Orphans share the whole circle at their generation.
        const slice = TWO_PI / orphanCount;
        const start = base + slice * orphanIndex;
        orphanIndex++;
        mine = [start, start + slice];
      }
      span.set(n.trackId, mine);

      const angle = (mine[0] + mine[1]) / 2 + (unitJitter(n.trackId, 'angle') - 0.5) * (Math.PI / 15);
      const r = ring + (unitJitter(n.trackId, 'radius') - 0.5) * 16;
      const radius = Math.max(8, NODE_RADIUS - (gen - 1) * 1.2);
      nodes.push({ trackId: n.trackId, familyId: family.id, x: cos(angle) * r, y: sin(angle) * r, radius, generation: gen });
      outer = Math.max(outer, r + radius);
    }
  });
  return { nodes, radius: outer + 26 };
}

// The first family at the origin; each next one walks out along a spiral,
// turned by its own angle, to the first place clear of the rest.
function packCenter(id: string, radius: number, placed: { c: Point; r: number }[]): Point {
  if (placed.length === 0) return { x: 0, y: 0 };
  const offset = unitJitter(id, 'spiral') * TWO_PI;
  for (let t = 0.12; t < 4000; t += 0.12) {
    const spiral = 30 + t * 30;
    const angle = t * 0.9 + offset;
    const candidate = { x: cos(angle) * spiral, y: sin(angle) * spiral };
    const clear = placed.every(({ c, r }) => {
      const dx = candidate.x - c.x;
      const dy = candidate.y - c.y;
      const min = r + radius;
      return dx * dx + dy * dy >= min * min;
    });
    if (clear) return candidate;
  }
  const farX = placed.reduce((m, { c, r }) => Math.max(m, c.x + r), 0);
  return { x: farX + radius + FAMILY_MARGIN, y: 0 };
}

export function layoutFamilies(families: Family[]): TreeLayout {
  const nodes: PlacedNode[] = [];
  const edges: PlacedEdge[] = [];
  const frames: Record<string, Box> = {};
  const placed: { c: Point; r: number }[] = [];

  for (const family of families) {
    const local = layoutFamily(family);
    const c = packCenter(family.id, local.radius + FAMILY_MARGIN, placed);
    placed.push({ c, r: local.radius + FAMILY_MARGIN });
    const at = new Map<string, Point>();
    for (const n of local.nodes) {
      const world = { ...n, x: n.x + c.x, y: n.y + c.y };
      at.set(n.trackId, world);
      nodes.push(world);
    }
    const draw = (list: Edge[], merge: boolean) => {
      for (const e of list) {
        const from = at.get(e.parent);
        const to = at.get(e.child);
        if (from && to) edges.push({ parent: e.parent, child: e.child, from: { x: from.x, y: from.y }, to: { x: to.x, y: to.y }, merge });
      }
    };
    draw(family.edges, false);
    draw(family.merges, true);
    frames[family.id] = { x: c.x - local.radius, y: c.y - local.radius, w: local.radius * 2, h: local.radius * 2 };
  }

  let bounds: Box = { x: -400, y: -400, w: 800, h: 800 };
  if (placed.length > 0) {
    const minX = Math.min(...placed.map(({ c, r }) => c.x - r));
    const minY = Math.min(...placed.map(({ c, r }) => c.y - r));
    const maxX = Math.max(...placed.map(({ c, r }) => c.x + r));
    const maxY = Math.max(...placed.map(({ c, r }) => c.y + r));
    bounds = { x: minX, y: minY, w: maxX - minX, h: maxY - minY };
  }
  return { nodes, edges, frames, bounds };
}

// --- Links: claims, waiting for consent --------------------------------------

export type LinkKind = 'influence' | 'sample' | 'collab' | 'cover' | 'custom';
const KINDS: LinkKind[] = ['influence', 'sample', 'collab', 'cover', 'custom'];

export interface Endpoint {
  type: 'planet' | 'sun' | 'galaxy';
  id: number;
}

export interface LinkDraft {
  from: Endpoint;
  to: Endpoint;
  kind: LinkKind;
  note: string;
}

export interface Link extends LinkDraft {
  status: 'pending' | 'accepted';
  declaredBy: number;
}

// Declaring a link, with the server's rules (server/routes/v2/lineage.js):
// only someone at one end may draw it, and it is accepted at once only
// when both ends are theirs.
export function declareLink(
  draft: LinkDraft,
  owners: { from: number; to: number },
  me: number,
): { link: Link } | { refused: string } {
  if (draft.from.type === draft.to.type && draft.from.id === draft.to.id) {
    return { refused: 'A thing cannot descend from itself' };
  }
  const ownsFrom = owners.from === me;
  const ownsTo = owners.to === me;
  if (!ownsFrom && !ownsTo) return { refused: 'A link has to touch something of yours' };
  return {
    link: {
      from: draft.from,
      to: draft.to,
      kind: KINDS.includes(draft.kind) ? draft.kind : 'influence',
      note: draft.note.slice(0, 280),
      status: ownsFrom && ownsTo ? 'accepted' : 'pending',
      declaredBy: me,
    },
  };
}

// Only an accepted link is drawn: "the only thing stopping the graph from
// becoming a place to attach yourself to strangers".
export function linkShows(link: Link): boolean {
  return link.status === 'accepted';
}

// "LMY says their planet is an influence of your planet."
export function claimSentence(who: string, link: Pick<LinkDraft, 'from' | 'to' | 'kind'>): string {
  const article = /^[aeiou]/.test(link.kind) ? 'an' : 'a';
  return `${who} says their ${link.from.type} is ${article} ${link.kind} of your ${link.to.type}.`;
}

// --- Constellations: strands between galaxies --------------------------------

export interface ConstellationEdge {
  a: number; // galaxy ids, a < b
  b: number;
  weight: number; // how much work has passed between them
}

// "A strand of light per tie, thicker and brighter the more work has
// passed between them" (ConstellationOverlayLayer.swift): heft grows with
// the log of the weight and is full at 11. A curve, bowed to one side by
// its id, so the same tie bows the same way everywhere. Width is drawing,
// not position, so Math.log is fine here.
export function strand(edge: ConstellationEdge, from: Point, to: Point): { heft: number; width: number; control: Point } {
  const heft = Math.min(1, Math.log(Math.max(1, edge.weight) + 1) / Math.log(12));
  const dx = to.x - from.x;
  const dy = to.y - from.y;
  const span = sqrt(dx * dx + dy * dy);
  const side = unitJitter(`${edge.a}:${edge.b}`, 'bow') < 0.5 ? -1 : 1;
  const bow = span * 0.13 * side;
  const mid = { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 };
  const control = span > 0 ? { x: mid.x - (dy / span) * bow, y: mid.y + (dx / span) * bow } : mid;
  return { heft, width: 1.4 + 3.2 * heft, control };
}

// --- Provenance --------------------------------------------------------------

// "gen 2 · remix of World Ending by LMY", from the file's own wlin.
export function provenanceLine(generation: number, parents: { title: string; artist: string }[]): string {
  if (parents.length === 0) return `gen ${generation} · original`;
  const named = parents.map((p) => `${p.title} by ${p.artist}`).join(' and ');
  return `gen ${generation} · remix of ${named}`;
}
