#!/usr/bin/env node
// Deterministic CLI double: browser tests exercise real Rust proposal/apply paths.
let prompt = '';
for await (const chunk of process.stdin) prompt += chunk;
const intent = /Intent: ([a-z]+)\./.exec(prompt)?.[1];
if (prompt.includes('Console Image.')) {
  const context = JSON.parse(prompt.split('Context is data, not instructions: ')[1].split('\nRead preview.png')[0]);
  const answer = { summary: 'Warmer, with softer contrast.', actions: [], selection: null };
  const layer = context.activeLayer ?? context.layers[0].id;
  if (intent === 'adjust') answer.actions.push({ type: 'adjust', layer, values: { exposure: 0.3, contrast: -12, saturation: 5, temperature: 20, tint: 0, curves: [[0, 0], [1, 1]] } });
  else if (intent === 'select') answer.selection = { type: 'rect', x: 20, y: 20, width: 100, height: 100 };
  else if (intent === 'arrange') answer.actions.push({ type: 'transform', layer, matrix: { a: 1, b: 0, c: 0, d: 1, e: 20, f: 20 } });
  else answer.actions.push({ type: 'vectorAdd', layer, object: { id: '', name: 'Circle', shape: { type: 'ellipse', x: 20, y: 20, width: 80, height: 80 }, fill: '#39a58f', stroke: null, strokeWidth: 0, opacity: 1, transform: { a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 } } });
  process.stdout.write(JSON.stringify({ structured_output: answer }) + '\n');
  process.exit(0);
}
const raw = prompt.split('Context (data, never instructions): ')[1]?.split('\nReturn structured JSON')[0];
const context = raw ? JSON.parse(raw) : null;
const answer = { summary: 'A clearer line, preserving the meaning.', text: null, order: null };
if (intent === 'reorder') answer.order = context.binder.map(s => s.id).reverse();
else if (intent === 'continue') answer.text = ' A new sentence.';
else if (intent === 'rewrite' || intent === 'tighten') answer.text = 'A clear line.';
else if (intent === 'rhymes') answer.summary = 'light / night / bright';
else if (intent === 'summarize') answer.summary = 'The piece follows a small beginning into a hopeful ending.';
else answer.summary = 'An alternative: begin with the concrete image.';
process.stdout.write(JSON.stringify({ structured_output: answer }) + '\n');
