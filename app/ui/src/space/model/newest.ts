// Newest, and Since you last looked (docs/SPEC.md 4.10, 2.10). Newest first
// is the only order; every list ends; nothing is counted.

import type { Medium, WorkFacts } from './catalogue';
import { cardFacts, price, verbFor } from './facts';
import { claimSentence, type LinkDraft } from './lineage';

export interface NewestItem extends WorkFacts {
  id: string;
  createdAt: string; // ISO 8601
  title: string;
  artist: string;
  galaxyId: number;
}

export interface NewestFilter {
  media?: Medium[]; // Mi · Si · Ri · Gi
  addedOnly?: boolean; // "Galaxies I've added"
  key?: string | null;
  bpm?: { min: number; max: number } | null;
}

// Newest first, then by id, as the server's feed orders (createdAt DESC,
// id DESC). Times are compared as the instants they name, so a time with
// milliseconds or an offset sorts where it belongs; ids as numbers when
// both are numbers.
export function filterNewest(items: NewestItem[], filter: NewestFilter, added: Set<number>): NewestItem[] {
  const { media, addedOnly, key, bpm } = filter;
  return items
    .filter((i) => !media || media.length === 0 || media.includes(i.medium))
    .filter((i) => !addedOnly || added.has(i.galaxyId))
    .filter((i) => !key || i.key === key)
    .filter((i) => !bpm || (i.bpm !== null && i.bpm >= bpm.min && i.bpm <= bpm.max))
    .sort((a, b) => byTime(b.createdAt, a.createdAt) || byId(b.id, a.id));
}

// An unreadable time falls back to its text, so the order is still total.
function byTime(a: string, b: string): number {
  const ta = Date.parse(a);
  const tb = Date.parse(b);
  if (Number.isNaN(ta) || Number.isNaN(tb)) return cmp(a, b);
  return ta - tb;
}

const NUMERIC = /^\d+$/;

function byId(a: string, b: string): number {
  if (NUMERIC.test(a) && NUMERIC.test(b)) return Number(a) - Number(b) || cmp(a, b);
  return cmp(a, b);
}

function cmp(a: string, b: string): number {
  return a < b ? -1 : a > b ? 1 : 0;
}

// "A card shows you what a thing IS and waits to be chosen."
export interface Card {
  id: string;
  title: string;
  artist: string;
  facts: string;
  verb: string;
}

export function cardOf(item: NewestItem): Card {
  return { id: item.id, title: item.title, artist: item.artist, facts: cardFacts(item), verb: verbFor(item.medium) };
}

export const PAGE = 30;

// The first `pages` × 30 cards, then "Show older" while any are left, and
// "That's everything." once none are.
export function pageOf(items: NewestItem[], pages: number): { cards: Card[]; end: string } {
  const shown = items.slice(0, pages * PAGE);
  const end = shown.length < items.length ? 'Show older' : "That's everything.";
  return { cards: shown.map(cardOf), end };
}

// --- Since you last looked ----------------------------------------------------

export type SinceEvent =
  | { kind: 'fork'; at: string; who: string; work: string; fork: string }
  | { kind: 'link'; at: string; who: string; linkId: number; link: Pick<LinkDraft, 'from' | 'to' | 'kind'> }
  | { kind: 'sale'; at: string; work: string; priceCents: number }
  | { kind: 'payout'; at: string; amountCents: number }
  | { kind: 'work'; at: string; who: string; title: string }
  | { kind: 'letter'; at: string; who: string; greeting: string; opening: string };

export interface SinceItem {
  at: string;
  line: string;
  linkId?: number;
  actions: ('Agree' | 'Refuse')[];
}

function lineOf(e: SinceEvent): string {
  switch (e.kind) {
    case 'fork':
      return `${e.who} forked ${e.work}: ${e.fork}.`;
    case 'link':
      return claimSentence(e.who, e.link);
    case 'sale':
      return `${e.work} sold for ${price(e.priceCents)}.`;
    case 'payout':
      return `${price(e.amountCents)} paid out.`;
    case 'work':
      return `New from ${e.who}: ${e.title}.`;
    case 'letter':
      return `A letter from ${e.who}: "${e.greeting} ${e.opening}"`;
  }
}

const MONTHS = ['Jan', 'Feb', 'Mar', 'Apr', 'May', 'Jun', 'Jul', 'Aug', 'Sep', 'Oct', 'Nov', 'Dec'];

// "4:30 PM" the same day, "Oct 4" the same year, "Oct 4, 2025" before that.
function since(when: Date, now: Date): string {
  const sameYear = when.getFullYear() === now.getFullYear();
  if (sameYear && when.getMonth() === now.getMonth() && when.getDate() === now.getDate()) {
    const h = when.getHours() % 12 || 12;
    const m = String(when.getMinutes()).padStart(2, '0');
    return `${h}:${m} ${when.getHours() < 12 ? 'AM' : 'PM'}`;
  }
  const day = `${MONTHS[when.getMonth()]} ${when.getDate()}`;
  return sameYear ? day : `${day}, ${when.getFullYear()}`;
}

// Built when you open Space: what happened since you last looked, newest
// first, plus any link still waiting on you however old it is. Null when
// there is nothing, so the list doesn't open at all.
export function sinceYouLastLooked(
  events: SinceEvent[],
  lastLooked: Date,
  now: Date,
): { items: SinceItem[]; end: string } | null {
  const after = lastLooked.getTime();
  const items = events
    .filter((e) => e.kind === 'link' || Date.parse(e.at) > after)
    .sort((a, b) => Date.parse(b.at) - Date.parse(a.at))
    .map(
      (e): SinceItem =>
        e.kind === 'link'
          ? { at: e.at, line: lineOf(e), linkId: e.linkId, actions: ['Agree', 'Refuse'] }
          : { at: e.at, line: lineOf(e), actions: [] },
    );
  if (items.length === 0) return null;
  return { items, end: `That's everything since ${since(lastLooked, now)}.` };
}
