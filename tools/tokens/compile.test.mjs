// node --test tools/tokens/compile.test.mjs
//
// Fails if: the committed outputs aren't what design/tokens.json compiles
// to; a second run changes a byte; --check passes with a stale output or with
// a colour written into the UI outside the token file; the compiler accepts a
// value it can't read.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '../..');
const OUTPUTS = [
  'app/ui/src/styles/tokens.css',
  'crates/wwav-tokens/src/generated.rs',
  'engine/include/wwav_tokens.h',
];

const run = (root, ...args) =>
  spawnSync(process.execPath, [join(root, 'tools/tokens/compile.mjs'), ...args], { encoding: 'utf8' });

// A scratch repo holding only the compiler and the token file.
function scratch(t) {
  const root = mkdtempSync(join(tmpdir(), 'tokens-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const p of ['tools/tokens/compile.mjs', 'design/tokens.json']) {
    mkdirSync(dirname(join(root, p)), { recursive: true });
    cpSync(join(repo, p), join(root, p));
  }
  return root;
}

const read = (root) => OUTPUTS.map((p) => readFileSync(join(root, p), 'utf8'));

test('the committed outputs are what the token file compiles to', () => {
  const r = run(repo, '--check');
  assert.equal(r.status, 0, r.stderr);
});

test('a second run changes nothing', (t) => {
  const root = scratch(t);
  assert.equal(run(root).status, 0);
  const first = read(root);
  const times = OUTPUTS.map((p) => statSync(join(root, p)).mtimeMs);
  const again = run(root);
  assert.equal(again.status, 0, again.stderr);
  assert.deepEqual(read(root), first);
  assert.deepEqual(OUTPUTS.map((p) => statSync(join(root, p)).mtimeMs), times, 'rewrote an unchanged file');
  assert.deepEqual(read(root), read(repo), 'a fresh compile differs from the committed outputs');
});

test('--check fails when the token file changed and the outputs did not', (t) => {
  const root = scratch(t);
  run(root);
  const json = join(root, 'design/tokens.json');
  writeFileSync(json, readFileSync(json, 'utf8').replace('"#070A18"', '"#070A19"'));
  const r = run(root, '--check');
  assert.notEqual(r.status, 0);
  for (const p of OUTPUTS) assert.match(r.stderr, new RegExp(p.replace(/[./]/g, '\\$&')));
});

test('--check fails when an output was edited by hand', (t) => {
  const root = scratch(t);
  run(root);
  const h = join(root, 'engine/include/wwav_tokens.h');
  writeFileSync(h, readFileSync(h, 'utf8').replace('140.0f', '120.0f'));
  const r = run(root, '--check');
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /engine\/include\/wwav_tokens\.h/);
  assert.doesNotMatch(r.stderr, /tokens\.css/);
});

test('--check fails on a colour written into the UI outside the token file', (t) => {
  const root = scratch(t);
  run(root);
  const ui = join(root, 'app/ui/src/console');
  mkdirSync(ui, { recursive: true });
  writeFileSync(join(ui, 'Meter.tsx'), 'const ok = "var(--screen-glow)";\nconst bad = "#8BAC0F";\n');
  writeFileSync(join(ui, 'strip.css'), '.strip {\n  color: rgb(12 34 56);\n}\n');
  // Test files may hold expected colours; the token CSS is generated.
  writeFileSync(join(ui, 'Meter.test.tsx'), 'expect(x).toBe("#8BAC0F");\n');
  const r = run(root, '--check');
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /app\/ui\/src\/console\/Meter\.tsx:2/);
  assert.match(r.stderr, /app\/ui\/src\/console\/strip\.css:2/);
  assert.doesNotMatch(r.stderr, /Meter\.test\.tsx|styles\/tokens\.css/);
});

test('a value the compiler cannot read is refused with its path', (t) => {
  const root = scratch(t);
  const json = join(root, 'design/tokens.json');
  writeFileSync(json, readFileSync(json, 'utf8').replace('"#C6E24A"', '"#C6E24"'));
  const r = run(root);
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /screen\.glow/);
});

test('--check names every line that writes a token, and only those', (t) => {
  const root = scratch(t);
  run(root);
  const ui = join(root, 'app/ui/src/heat');
  mkdirSync(ui, { recursive: true });
  writeFileSync(
    join(ui, 'pill.css'),
    [
      '#add,',
      '.pill:hover {',
      '  color: #123456;',
      '  border-color: #abcdef;',
      '  background: url(#grain) var(--label-red);',
      '  outline-color: transparent;',
      '  transition-delay: 0s;',
      '  font-family: "White Rabbit";',
      '  box-shadow: 0 1px 0',
      '    rgb(0 0 0 / 0.2);',
      '}',
    ].join('\n'),
  );
  writeFileSync(
    join(ui, 'pill.ts'),
    [
      "// #2946FF is royal blue, and fades take 140ms.",
      "export const grade = (a: boolean) => (a ? 'green' : 'grey');",
      'export const low = (n: number) => n & 0xffffff;',
      "const at = location.hash === '#feed';",
      "/* fill: 'red' */ export const fill = 'none';",
      "export const ring = { stroke: 'white' };",
      "export const tone = 0x2946ff;",
    ].join('\n'),
  );
  const r = run(root, '--check');
  assert.notEqual(r.status, 0);
  const named = r.stderr.split('\n').filter((l) => l.startsWith('  app/')).map((l) => l.trim().split(': ')[0]);
  assert.deepEqual(named, [
    'app/ui/src/heat/pill.css:3',
    'app/ui/src/heat/pill.css:4',
    'app/ui/src/heat/pill.css:10',
    'app/ui/src/heat/pill.ts:6',
    'app/ui/src/heat/pill.ts:7',
  ]);
});
