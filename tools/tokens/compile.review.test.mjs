// node --test tools/tokens/compile.review.test.mjs
//
// An adversarial review of the token compiler (F7). Each test failed when it
// was written, because of a finding in the review of build/tokens. Each
// finding is fixed, so each test now runs with the suite.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const repo = join(here, '../..');

const run = (root, ...args) =>
  spawnSync(process.execPath, [join(root, 'tools/tokens/compile.mjs'), ...args], { encoding: 'utf8' });

function scratch(t) {
  const root = mkdtempSync(join(tmpdir(), 'tokens-review-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  for (const p of ['tools/tokens/compile.mjs', 'design/tokens.json']) {
    mkdirSync(dirname(join(root, p)), { recursive: true });
    cpSync(join(repo, p), join(root, p));
  }
  return root;
}

test(
  '--check fails on a token written into the UI in any of the usual ways',
  (t) => {
    const root = scratch(t);
    run(root);
    const ui = join(root, 'app/ui/src/space');
    mkdirSync(ui, { recursive: true });
    // three.js (Space and Unquantized, 4.5 and 7.18) takes colours as numbers.
    writeFileSync(join(ui, 'Sky.ts'), 'export const night = new THREE.Color(0x070a18);\n');
    // A named colour, a modern colour function and a JSON theme.
    writeFileSync(join(ui, 'orb.css'), '.orb {\n  outline-color: white;\n}\n.rim {\n  color: oklch(62% 0.2 30);\n}\n');
    writeFileSync(join(ui, 'theme.json'), '{ "vocals": "#D23C2A" }\n');
    // Motion is a token too (8.6): the beat and the curve, typed out.
    writeFileSync(join(ui, 'fade.css'), '.fade {\n  transition: opacity 140ms cubic-bezier(0.22, 1, 0.36, 1);\n}\n');
    const r = run(root, '--check');
    assert.notEqual(r.status, 0, 'passed with tokens written outside design/tokens.json');
    for (const f of ['Sky.ts', 'orb.css:2', 'orb.css:5', 'theme.json', 'fade.css']) assert.match(r.stderr, new RegExp(f));
  },
);

// A note that names a glob path ("app/ui/src/heat/*/") is enough. The
// compiler writes it into a CSS comment unescaped, the comment closes early,
// and a CSS parser drops the next declaration (lightningcss loses
// --heat-cool), while --check and crates/wwav-tokens' parity test both stay
// green, because the parity test reads tokens.css line by line and ignores
// comments. The CSS then lacks a token that the Rust and C++ outputs have.
test(
  "a note in the token file can't break the CSS",
  (t) => {
    const root = scratch(t);
    const json = join(root, 'design/tokens.json');
    const tokens = JSON.parse(readFileSync(json, 'utf8'));
    tokens.heat.$doc = 'Heat level colours (3.1); see heat.css */ for the tubes.';
    writeFileSync(json, JSON.stringify(tokens, null, 2));
    const r = run(root);
    assert.equal(r.status, 0, r.stderr);
    const css = readFileSync(join(root, 'app/ui/src/styles/tokens.css'), 'utf8');
    // Every comment the compiler opens closes once, with nothing after it.
    const opened = css.split('/*').length - 1;
    const closed = css.split('*/').length - 1;
    assert.equal(closed, opened, 'a comment closes twice, so its tail is read as CSS');
  },
);

test(
  'an anchor or an id in the UI is not a colour',
  (t) => {
    const root = scratch(t);
    run(root);
    const ui = join(root, 'app/ui/src/shell');
    mkdirSync(ui, { recursive: true });
    writeFileSync(join(ui, 'nav.html'), '<a href="#feed">Since you last looked</a>\n');
    writeFileSync(join(ui, 'shell.css'), '#add {\n  color: var(--desk-ink);\n}\n');
    const r = run(root, '--check');
    assert.equal(r.status, 0, r.stderr);
  },
);
