#!/usr/bin/env node
// Deterministic CLI double: browser tests exercise real Rust proposal/apply paths.
let prompt = '';
for await (const chunk of process.stdin) prompt += chunk;
const intent = /Intent: ([a-z]+)\./.exec(prompt)?.[1];
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
