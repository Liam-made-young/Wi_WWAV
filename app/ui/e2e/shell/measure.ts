/// <reference types="node" />
// Gate 2.4's measurements for the shell (docs/GATES.md 2.4, docs/SPEC.md
// 2.12, 3.18, 8.9), taken from what Chromium actually draws:
//
// - every text element's contrast against what is behind it: the computed
//   text colour laid over every colour its grounds can show there (each
//   stop of each gradient, through every translucent layer, the gels'
//   ::before included), the worst of them counting. Body text needs 7:1,
//   secondary text 4.5:1. Secondary is what 8.9 says: labels on controls,
//   headings and an LCD's second line, marked data-text="secondary" or
//   inside a control;
// - every control's box: buttons, tabs and orbs at least 44 × 44, dense
//   rows and grid cells (data-dense, options, menu items, fields) at least
//   24 × 24 (3.18's rule);
// - every text at least 11 pt.
//
// What is measured is what can be seen: the case, and then either the
// sheet on top (when a backdrop dims the room) or the current room with
// the drawer over it. Rooms that aren't current are hidden and skipped.

import type { Page } from '@playwright/test';

export interface TextSample {
  text: string;
  kind: 'body' | 'secondary';
  ratio: number;
  size: number;
}

export interface TargetSample {
  name: string;
  w: number;
  h: number;
  need: number;
}

export interface Audit {
  texts: TextSample[];
  targets: TargetSample[];
}

export async function audit(page: Page): Promise<Audit> {
  // Let any sheet finish arriving: measure what rests, not a frame of a fade.
  await page.evaluate(() => Promise.all(document.getAnimations().map((a) => a.finished.catch(() => {}))));
  return page.evaluate(() => {
    type RGBA = [number, number, number, number];
    const parse = (s: string): RGBA[] => {
      const out: RGBA[] = [];
      const re = /rgba?\(([^)]*)\)|color\(srgb ([^)]*)\)|\btransparent\b/g;
      for (const m of s.matchAll(re)) {
        if (m[0] === 'transparent') out.push([0, 0, 0, 0]);
        else if (m[1] !== undefined) {
          const v = m[1]
            .split(/[\s,/]+/)
            .filter(Boolean)
            .map(Number);
          out.push([v[0], v[1], v[2], v[3] ?? 1]);
        } else {
          const v = m[2]
            .split(/[\s/]+/)
            .filter(Boolean)
            .map(Number);
          out.push([v[0] * 255, v[1] * 255, v[2] * 255, v[3] ?? 1]);
        }
      }
      return out;
    };
    const over = (top: RGBA, under: RGBA): RGBA => {
      const a = top[3];
      return [top[0] * a + under[0] * (1 - a), top[1] * a + under[1] * (1 - a), top[2] * a + under[2] * (1 - a), 1];
    };
    const lum = (c: RGBA) => {
      const lin = (x: number) => {
        x /= 255;
        return x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4;
      };
      return 0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2]);
    };
    const ratio = (a: RGBA, b: RGBA) => {
      const [x, y] = [lum(a), lum(b)];
      return (Math.max(x, y) + 0.05) / (Math.min(x, y) + 0.05);
    };
    const key = (c: RGBA) => c.map((v) => Math.round(v)).join(',');

    // Each layer an element paints under its text, bottom first.
    const layers = (el: Element): RGBA[][] => {
      const out: RGBA[][] = [];
      const paint = (s: CSSStyleDeclaration) => {
        const bg = parse(s.backgroundColor);
        if (bg.length && bg[0][3] > 0) out.push(bg);
        // background-image lists its layers top first.
        const images =
          s.backgroundImage === 'none'
            ? []
            : s.backgroundImage.split(/,(?![^(]*\))(?=\s*(?:linear|radial|repeating|conic|url))/);
        for (const image of images.reverse()) {
          const stops = parse(image);
          if (stops.length) out.push(stops);
        }
      };
      paint(getComputedStyle(el));
      const before = getComputedStyle(el, '::before');
      if (before.content !== 'none') paint(before);
      return out;
    };

    const grounds = (el: Element): RGBA[] => {
      const chain: Element[] = [];
      for (let e: Element | null = el; e; e = e.parentElement) chain.unshift(e);
      let shown: RGBA[] = [[255, 255, 255, 1]];
      for (const e of chain) {
        for (const stops of layers(e)) {
          const next = new Map<string, RGBA>();
          for (const stop of stops) for (const c of shown) next.set(key(over(stop, c)), over(stop, c));
          shown = [...next.values()];
        }
      }
      return shown;
    };

    const visible = (el: Element) =>
      el.checkVisibility({ opacityProperty: true, visibilityProperty: true }) && !el.closest('[inert]');

    // What can be seen: the case, and the top of the room area.
    const scopes: Element[] = [...document.querySelectorAll('.case-bar, .case-status')];
    const backdrop = document.querySelector('.backdrop');
    const first = document.querySelector('.first');
    const area = document.querySelector('.room-area')!;
    if (first && visible(first)) scopes.push(first);
    else if (backdrop) {
      const z = Number(getComputedStyle(backdrop).zIndex);
      for (const el of area.children) {
        if (el !== backdrop && visible(el) && Number(getComputedStyle(el).zIndex) > z) scopes.push(el);
      }
    } else {
      scopes.push(document.querySelector('.room[data-current="true"]')!);
      for (const el of area.querySelectorAll(':scope > .drawer, :scope > .info, :scope > .toast')) {
        if (visible(el)) scopes.push(el);
      }
    }

    const texts: TextSample[] = [];
    const targets: TargetSample[] = [];
    const secondary = (el: Element) => {
      const marked = el.closest('[data-text]');
      if (marked) return marked.getAttribute('data-text') === 'secondary';
      return !!el.closest(
        'button, [role="tab"], [role="menuitem"], label, th, .source-heading, .info-heading, .setting-label',
      );
    };
    for (const scope of scopes) {
      for (const el of [scope, ...scope.querySelectorAll('*')]) {
        if (!visible(el)) continue;
        const own = [...el.childNodes].filter((n) => n.nodeType === Node.TEXT_NODE && n.textContent!.trim());
        if (own.length && el.getBoundingClientRect().width > 0) {
          const s = getComputedStyle(el);
          const ink = parse(s.color)[0];
          const worst = Math.min(...grounds(el).map((g) => ratio(over(ink, g), g)));
          texts.push({
            text: own
              .map((n) => n.textContent!.trim())
              .join(' ')
              .slice(0, 60),
            kind: secondary(el) ? 'secondary' : 'body',
            ratio: Math.round(worst * 100) / 100,
            size: parseFloat(s.fontSize),
          });
        }
        const control = el.matches(
          'button, [role="tab"], [role="option"], [role="menuitem"], [role="button"], a[href], input, select, textarea, label.check',
        );
        if (control && !(el instanceof HTMLInputElement && el.closest('label.check'))) {
          const r = el.getBoundingClientRect();
          const dense =
            el.hasAttribute('data-dense') ||
            el.matches('[role="option"], [role="menuitem"], input, select, textarea, label.check');
          const name = el.getAttribute('aria-label') ?? (el.textContent ?? '').trim().slice(0, 40);
          targets.push({
            name: `${el.tagName.toLowerCase()} ${name}`,
            w: Math.round(r.width * 10) / 10,
            h: Math.round(r.height * 10) / 10,
            need: dense ? 24 : 44,
          });
        }
      }
    }
    return { texts, targets };
  });
}

export interface Findings {
  texts: number;
  targets: number;
  minBody: number | null;
  minSecondary: number | null;
  smallestText: number | null;
  smallestTarget: string | null;
  fails: string[];
}

export function judge(a: Audit): Findings {
  const fails: string[] = [];
  for (const t of a.texts) {
    const need = t.kind === 'body' ? 7 : 4.5;
    if (t.ratio < need) fails.push(`${t.kind} text "${t.text}" at ${t.ratio}:1, needs ${need}:1`);
    if (t.size < 11) fails.push(`text "${t.text}" at ${t.size} pt, under 11`);
  }
  for (const t of a.targets) {
    if (t.w < t.need || t.h < t.need) fails.push(`${t.name} is ${t.w} × ${t.h}, needs ${t.need} × ${t.need}`);
  }
  const min = (xs: number[]) => (xs.length ? Math.min(...xs) : null);
  const smallest = [...a.targets].sort((x, y) => Math.min(x.w, x.h) / x.need - Math.min(y.w, y.h) / y.need)[0];
  return {
    texts: a.texts.length,
    targets: a.targets.length,
    minBody: min(a.texts.filter((t) => t.kind === 'body').map((t) => t.ratio)),
    minSecondary: min(a.texts.filter((t) => t.kind === 'secondary').map((t) => t.ratio)),
    smallestText: min(a.texts.map((t) => t.size)),
    smallestTarget: smallest ? `${smallest.w} × ${smallest.h} (needs ${smallest.need})` : null,
    fails,
  };
}

/** Two screenshots 5 s apart, compared byte for byte: nothing may move while nothing plays. */
export async function stillFor5s(page: Page): Promise<boolean> {
  await page.evaluate(() => (document.activeElement as HTMLElement | null)?.blur());
  const a = await page.screenshot({ animations: 'allow', caret: 'initial' });
  await page.waitForTimeout(5000);
  const b = await page.screenshot({ animations: 'allow', caret: 'initial' });
  return a.equals(b);
}
