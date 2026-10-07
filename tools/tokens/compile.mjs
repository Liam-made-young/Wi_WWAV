#!/usr/bin/env node
// Compiles design/tokens.json into the three places tokens are read
// (docs/SPEC.md 8.11): CSS custom properties for the web UI, a Rust module for
// the compositor and a C++ header for the engine. Node 22, no dependencies.
//
//   node tools/tokens/compile.mjs           write each output that changed
//   node tools/tokens/compile.mjs --check   exit 1 if an output is stale, or if
//                                           the UI writes a colour of its own

import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '../..');
const SOURCE = 'design/tokens.json';
const CSS_OUT = 'app/ui/src/styles/tokens.css';
const RUST_OUT = 'crates/wwav-tokens/src/generated.rs';
const CPP_OUT = 'engine/include/wwav_tokens.h';
const HEADER = [
  `Generated from ${SOURCE} by tools/tokens/compile.mjs.`,
  "Don't edit it: change the JSON and run the compiler again.",
];

class TokenError extends Error {}
const fail = (path, msg) => {
  throw new TokenError(`${SOURCE}: ${path.join('.')}: ${msg}`);
};

// ---- Reading ------------------------------------------------------------------
//
// The file becomes a tree of groups ({doc, path, children}) and tokens ({path,
// kind, unit, light, dark, themed}). kind is color, colors, number, numbers,
// names or curve; a dimension like "140ms" is a number with a unit.

const HEX = /^#([0-9a-fA-F]{6})$/;
const RGBA = /^rgba\((\d{1,3}), (\d{1,3}), (\d{1,3}), (0|1|0?\.\d+)\)$/;
const DIMENSION = /^(-?\d+(?:\.\d+)?)(px|ms|em)$/;
const NUM = '(-?\\d+(?:\\.\\d+)?)';
const CURVE = new RegExp(`^cubic-bezier\\(${NUM}, ${NUM}, ${NUM}, ${NUM}\\)$`);

function color(s) {
  let m = HEX.exec(s);
  if (m) {
    const n = parseInt(m[1], 16);
    return { r: n >> 16, g: (n >> 8) & 255, b: n & 255, a: 1 };
  }
  m = RGBA.exec(s);
  if (m && [m[1], m[2], m[3]].every((c) => Number(c) <= 255)) {
    return { r: Number(m[1]), g: Number(m[2]), b: Number(m[3]), a: Number(m[4]) };
  }
  return null;
}

function value(v, path) {
  if (typeof v === 'number') return { kind: 'number', value: v };
  if (typeof v === 'string') {
    const c = color(v);
    if (c) return { kind: 'color', value: c };
    let m = DIMENSION.exec(v);
    if (m) return { kind: 'number', value: Number(m[1]), unit: m[2] };
    m = CURVE.exec(v);
    if (m) return { kind: 'curve', value: m.slice(1).map(Number) };
  }
  if (Array.isArray(v) && v.length > 0) {
    if (v.every((x) => typeof x === 'string' && color(x))) return { kind: 'colors', value: v.map(color) };
    if (v.every((x) => typeof x === 'number')) return { kind: 'numbers', value: v };
    // A name list (font families, a register's rules) can't look like a colour.
    if (v.every((x) => typeof x === 'string' && !/^(#|rgb)/.test(x))) return { kind: 'names', value: v };
  }
  fail(path, `can't read ${JSON.stringify(v)}`);
}

const isThemed = (v) =>
  v && typeof v === 'object' && !Array.isArray(v) && Object.keys(v).sort().join() === 'dark,light';

function read(node, path) {
  const group = { path, doc: node.$doc, children: [] };
  for (const [key, v] of Object.entries(node)) {
    if (key.startsWith('$')) continue;
    const p = [...path, key];
    if (!/^[a-z][A-Za-z0-9]*$/.test(key)) fail(p, 'a key is camelCase, starting with a letter');
    if (v && typeof v === 'object' && !Array.isArray(v) && !isThemed(v)) {
      group.children.push(read(v, p));
      continue;
    }
    if (path.length === 0) fail(p, 'a token belongs to a group');
    if (isThemed(v)) {
      const light = value(v.light, p);
      const dark = value(v.dark, p);
      if (!['color', 'colors'].includes(light.kind) || light.kind !== dark.kind) {
        fail(p, 'only colours and gradients have a light and a dark value');
      }
      group.children.push({ path: p, kind: light.kind, light: light.value, dark: dark.value, themed: true });
    } else {
      const x = value(v, p);
      group.children.push({ path: p, kind: x.kind, unit: x.unit, light: x.value, dark: x.value, themed: false });
    }
  }
  return group;
}

const tokensOf = (group) => group.children.flatMap((c) => (c.children ? tokensOf(c) : [c]));

function wrap(text, width) {
  const lines = [];
  let line = '';
  for (const word of text.split(/\s+/)) {
    if (line && line.length + 1 + word.length > width) {
      lines.push(line);
      line = word;
    } else {
      line = line ? `${line} ${word}` : word;
    }
  }
  return line ? [...lines, line] : lines;
}

const hex2 = (n) => n.toString(16).padStart(2, '0');

// ---- CSS ----------------------------------------------------------------------
//
// Light values on :root; dark ones for the themed tokens under the system's
// dark mode unless [data-appearance="light"] says otherwise, and under
// [data-appearance="dark"] whatever the system says.

const kebab = (s) => s.replace(/[A-Z]/g, (c) => `-${c.toLowerCase()}`);
const cssName = (path) => `--${path.map(kebab).join('-')}`;
const GENERIC_FAMILIES = ['serif', 'sans-serif', 'monospace', 'system-ui', 'cursive', 'fantasy'];

function cssColor({ r, g, b, a }) {
  return a === 1 ? `#${hex2(r)}${hex2(g)}${hex2(b)}` : `rgb(${r} ${g} ${b} / ${a})`;
}

function cssValue(t, v) {
  switch (t.kind) {
    case 'color':
      return cssColor(v);
    case 'colors':
      return v.map(cssColor).join(', ');
    case 'number':
      return `${v}${t.unit ?? ''}`;
    case 'numbers':
      return v.join(', ');
    case 'names':
      return v.map((n) => (GENERIC_FAMILIES.includes(n) ? n : JSON.stringify(n))).join(', ');
    case 'curve':
      return `cubic-bezier(${v.join(', ')})`;
  }
}

function css(tree) {
  const light = [];
  const dark = [];
  (function emit(group) {
    if (group.doc && group.path.length) {
      const [first, ...rest] = wrap(`${group.path.join('.')}: ${group.doc}`, 72);
      light.push('', `  /* ${first}`, ...rest.map((l) => `     ${l}`));
      light[light.length - 1] += ' */';
    }
    for (const c of group.children) {
      if (c.children) {
        emit(c);
        continue;
      }
      light.push(`  ${cssName(c.path)}: ${cssValue(c, c.light)};`);
      if (c.themed) dark.push(`${cssName(c.path)}: ${cssValue(c, c.dark)};`);
      // The night's ink at each of its opacities, as v4 named them (--ink-70):
      // the UI can't scale a custom property's alpha on every WebKit we run on.
      const base = c.path.at(-1).match(/^(.+)Steps$/)?.[1];
      const ink = base && group.children.find((s) => s.path.at(-1) === base && s.kind === 'color');
      if (ink) {
        for (const step of c.light) {
          const name = `${cssName(ink.path)}-${String(Math.round(step * 100)).padStart(2, '0')}`;
          light.push(`  ${name}: ${cssColor({ ...ink.light, a: step })};`);
          if (ink.themed) dark.push(`${name}: ${cssColor({ ...ink.dark, a: step })};`);
        }
      }
    }
  })(tree);
  return [
    `/* ${HEADER[0]}`,
    `   ${HEADER[1]} */`,
    '',
    ':root {',
    ...light.slice(light[0] === '' ? 1 : 0),
    '}',
    '',
    '@media (prefers-color-scheme: dark) {',
    '  :root:not([data-appearance="light"]) {',
    ...dark.map((l) => `    ${l}`),
    '  }',
    '}',
    '',
    ':root[data-appearance="dark"] {',
    ...dark.map((l) => `  ${l}`),
    '}',
    '',
  ].join('\n');
}

// ---- Rust ---------------------------------------------------------------------
//
// One module per group, one const per token, the unit in the name
// (motion::BEAT_MS), and ALL listing every token by its path.

const snake = (s) => s.replace(/[A-Z]/g, (c) => `_${c.toLowerCase()}`);
const RUST_KEYWORDS = ['type', 'mod', 'fn', 'use', 'struct', 'enum', 'impl', 'trait', 'match', 'move', 'ref'];
const rustModule = (s) => (RUST_KEYWORDS.includes(snake(s)) ? `r#${snake(s)}` : snake(s));
const rustConst = (t) => [snake(t.path.at(-1)), t.unit].filter(Boolean).join('_').toUpperCase();
const rustPath = (t) => [...t.path.slice(0, -1).map(rustModule), rustConst(t)].join('::');
const float = (n) => (Number.isInteger(n) ? n.toFixed(1) : String(n));

function rustColor({ r, g, b, a }) {
  const rgb = `0x${hex2(r)}, 0x${hex2(g)}, 0x${hex2(b)}`;
  return a === 1 ? `crate::Color::rgb(${rgb})` : `crate::Color::rgba(${rgb}, ${float(a)})`;
}

function rustTyped(t) {
  const themed = (type, f) =>
    t.themed ? [`crate::Themed<${type}>`, `crate::Themed { light: ${f(t.light)}, dark: ${f(t.dark)} }`] : [type, f(t.light)];
  switch (t.kind) {
    case 'color':
      return themed('crate::Color', rustColor);
    case 'colors':
      return themed('&[crate::Color]', (v) => `&[${v.map(rustColor).join(', ')}]`);
    case 'number':
      return ['f32', float(t.light)];
    case 'numbers':
      return ['&[f32]', `&[${t.light.map(float).join(', ')}]`];
    case 'names':
      return ['&[&str]', `&[${t.light.map((n) => JSON.stringify(n)).join(', ')}]`];
    case 'curve': {
      const [x1, y1, x2, y2] = t.light.map(float);
      return ['crate::Curve', `crate::Curve { x1: ${x1}, y1: ${y1}, x2: ${x2}, y2: ${y2} }`];
    }
  }
}

const RUST_VARIANT = {
  color: ['Color', 'ThemedColor'],
  colors: ['Colors', 'ThemedColors'],
  number: ['Number'],
  numbers: ['Numbers'],
  names: ['Names'],
  curve: ['Curve'],
};

function rust(tree) {
  const out = [`// ${HEADER[0]}`, `// ${HEADER[1]}`];
  (function emit(group, depth) {
    const pad = '    '.repeat(depth);
    for (const c of group.children) {
      if (c.children) {
        out.push('', `${pad}pub mod ${rustModule(c.path.at(-1))} {`);
        if (c.doc) out.push(...wrap(c.doc, 72 - pad.length).map((l) => `${pad}    //! ${l}`));
        emit(c, depth + 1);
        out.push(`${pad}}`);
      } else {
        const [type, val] = rustTyped(c);
        out.push(`${pad}pub const ${rustConst(c)}: ${type} = ${val};`);
      }
    }
  })(tree, 0);
  out.push('', `/// Every token, by its path in ${SOURCE}.`, 'pub const ALL: &[(&str, crate::Token)] = &[');
  for (const t of tokensOf(tree)) {
    const variant = RUST_VARIANT[t.kind][t.themed ? 1 : 0];
    out.push(`    ("${t.path.join('.')}", crate::Token::${variant}(${rustPath(t)})),`);
  }
  out.push('];', '');
  return out.join('\n');
}

// ---- C++ ----------------------------------------------------------------------
//
// C++17 constexprs in nested namespaces (wwav::tokens::desk::kInk), the unit
// in the name (motion::kBeatMs). A list is a pointer and a count, because a
// gradient can have a different number of stops in light and dark.

const UNIT_SUFFIX = { px: 'Px', ms: 'Ms', em: 'Em' };
const cppConst = (t) => {
  const key = t.path.at(-1);
  return `k${key[0].toUpperCase()}${key.slice(1)}${UNIT_SUFFIX[t.unit] ?? ''}`;
};
const cppColor = ({ r, g, b, a }) => `{0x${hex2(r)}, 0x${hex2(g)}, 0x${hex2(b)}, ${float(a)}f}`;

function cppToken(t) {
  const name = cppConst(t);
  const line = (s) => `inline constexpr ${s};`;
  const list = (type, items, suffix, f) =>
    line(`${type} ${name}${suffix}[] = {${items.map(f).join(', ')}}`);
  switch (t.kind) {
    case 'color':
      return t.themed
        ? [line(`Themed<Color> ${name}{${cppColor(t.light)}, ${cppColor(t.dark)}}`)]
        : [line(`Color ${name}${cppColor(t.light)}`)];
    case 'colors':
      if (t.themed) {
        return [
          list('Color', t.light, 'Light', cppColor),
          list('Color', t.dark, 'Dark', cppColor),
          line(
            `Themed<List<Color>> ${name}{{${name}Light, ${t.light.length}}, {${name}Dark, ${t.dark.length}}}`,
          ),
        ];
      }
      return [list('Color', t.light, 'Items', cppColor), line(`List<Color> ${name}{${name}Items, ${t.light.length}}`)];
    case 'number':
      return [line(`float ${name} = ${float(t.light)}f`)];
    case 'numbers':
      return [
        list('float', t.light, 'Items', (n) => `${float(n)}f`),
        line(`List<float> ${name}{${name}Items, ${t.light.length}}`),
      ];
    case 'names':
      return [
        list('const char*', t.light, 'Items', (n) => JSON.stringify(n)),
        line(`List<const char*> ${name}{${name}Items, ${t.light.length}}`),
      ];
    case 'curve':
      return [line(`Curve ${name}{${t.light.map((n) => `${float(n)}f`).join(', ')}}`)];
  }
}

function cpp(tree) {
  const out = [
    `// ${HEADER[0]}`,
    `// ${HEADER[1]}`,
    '',
    '#ifndef WWAV_TOKENS_H',
    '#define WWAV_TOKENS_H',
    '',
    '#include <cstddef>',
    '#include <cstdint>',
    '',
    'namespace wwav::tokens {',
    '',
    '// An sRGB colour; a is 1 except for the few translucent tokens.',
    'struct Color {',
    '  std::uint8_t r, g, b;',
    '  float a;',
    '};',
    '',
    '// A token with its own value in light and dark.',
    'template <typename T>',
    'struct Themed {',
    '  T light, dark;',
    '};',
    '',
    "// A gradient's stops (top or centre first), or a list of names or numbers.",
    'template <typename T>',
    'struct List {',
    '  const T* items;',
    '  std::size_t count;',
    '};',
    '',
    '// A CSS cubic-bezier() timing curve.',
    'struct Curve {',
    '  float x1, y1, x2, y2;',
    '};',
  ];
  (function emit(group) {
    for (const c of group.children) {
      if (c.children) {
        const name = c.path.at(-1);
        out.push('', `namespace ${name} {`);
        if (c.doc) out.push(...wrap(c.doc, 76).map((l) => `// ${l}`));
        emit(c);
        out.push(`}  // namespace ${name}`);
      } else {
        out.push(...cppToken(c));
      }
    }
  })(tree);
  out.push('', '}  // namespace wwav::tokens', '', '#endif  // WWAV_TOKENS_H', '');
  return out.join('\n');
}

// ---- Stray colours ------------------------------------------------------------
//
// F7: a token is defined nowhere but the token file. In the web UI the tell
// is a colour literal, a hex or a numeric rgb() or hsl(), anywhere but the
// generated CSS. Test files may hold expected colours.

const UI = 'app/ui/src';
const LITERAL = /#[0-9a-fA-F]{3,8}\b|\b(?:rgba?|hsla?)\(\s*[\d.]/;

function strayColors() {
  const dir = join(ROOT, UI);
  if (!existsSync(dir)) return [];
  const found = [];
  for (const file of readdirSync(dir, { recursive: true }).sort()) {
    const rel = `${UI}/${file}`;
    if (rel === CSS_OUT || /\.test\.\w+$/.test(file) || !/\.(css|html|svg|[cm]?[jt]sx?)$/.test(file)) continue;
    readFileSync(join(dir, file), 'utf8')
      .split('\n')
      .forEach((line, i) => {
        if (LITERAL.test(line)) found.push(`${rel}:${i + 1}: ${line.trim()}`);
      });
  }
  return found;
}

// ---- Main ---------------------------------------------------------------------

function main(check) {
  const tree = read(JSON.parse(readFileSync(join(ROOT, SOURCE), 'utf8')), []);
  const outputs = { [CSS_OUT]: css(tree), [RUST_OUT]: rust(tree), [CPP_OUT]: cpp(tree) };
  const current = (p) => existsSync(join(ROOT, p)) && readFileSync(join(ROOT, p), 'utf8') === outputs[p];
  if (!check) {
    for (const p of Object.keys(outputs).filter((p) => !current(p))) {
      mkdirSync(dirname(join(ROOT, p)), { recursive: true });
      writeFileSync(join(ROOT, p), outputs[p]);
      console.log(`wrote ${p}`);
    }
    return 0;
  }
  const stale = Object.keys(outputs).filter((p) => !current(p));
  const strays = strayColors();
  if (stale.length) {
    console.error(`Stale, so run node tools/tokens/compile.mjs:\n${stale.map((p) => `  ${p}`).join('\n')}`);
  }
  if (strays.length) {
    console.error(`Colours written outside ${SOURCE}; use its custom properties:\n${strays.map((s) => `  ${s}`).join('\n')}`);
  }
  return stale.length || strays.length ? 1 : 0;
}

try {
  process.exitCode = main(process.argv.includes('--check'));
} catch (e) {
  if (!(e instanceof TokenError)) throw e;
  console.error(e.message);
  process.exitCode = 1;
}
