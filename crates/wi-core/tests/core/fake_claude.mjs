#!/usr/bin/env node
// A stand-in for the Claude Code command line, for the prompt box's tests
// (ask.rs). It is started as `claude -p … --mcp-config <json>` would be, reads
// the request on stdin, calls the core's tools over HTTP as Claude Code does,
// and prints the result envelope. What it "decides" is one of the scenarios
// below, named by a #tag at the end of the request.

const argv = process.argv.slice(2);
const flag = (name) => {
  const i = argv.indexOf(name);
  return i < 0 ? null : argv[i + 1];
};
const config = JSON.parse(flag('--mcp-config') ?? '{}');
const server = config.mcpServers?.learn ?? {};
const system = flag('--system-prompt') ?? '';
const allowed = (flag('--allowedTools') ?? '').split(',').filter(Boolean);

let nextId = 1;
async function rpc(method, params, headers = server.headers) {
  const r = await fetch(server.url, {
    method: 'POST',
    headers: { 'content-type': 'application/json', accept: 'application/json, text/event-stream', ...headers },
    body: JSON.stringify({ jsonrpc: '2.0', id: nextId++, method, params }),
  });
  if (r.status !== 200) return { status: r.status };
  return { status: 200, ...(await r.json()) };
}

/** Calls a tool; the answer is its JSON, or {error} with the tool's sentence. */
async function tool(name, args) {
  const r = await rpc('tools/call', { name, arguments: args });
  const text = r.result?.content?.[0]?.text ?? '';
  if (r.result?.isError) return { error: text };
  try {
    return JSON.parse(text);
  } catch {
    return { text };
  }
}

const dayBefore = (due) => {
  // "2026-10-09 23:59" -> "2026-10-08 23:59"
  const [date, time] = due.split(' ');
  const d = new Date(`${date}T12:00:00Z`);
  d.setUTCDate(d.getUTCDate() - 1);
  return `${d.toISOString().slice(0, 10)}${time ? ` ${time}` : ''}`;
};

const scenarios = {
  // "Move everything due this week in JPN 101 to the day before"
  async 'day-before'() {
    const found = await tool('list_rows', {
      table: 'Tasks',
      filters: [
        { column: 'Course', op: 'eq', value: 'JPN 101' },
        { column: 'Due', op: 'ge', value: '2026-10-07' },
        { column: 'Due', op: 'le', value: '2026-10-13' },
        { column: 'Done', op: 'eq', value: false },
      ],
      columns: ['Title', 'Due'],
    });
    const changes = found.rows.map((r) => ({ id: r.id, set: { Due: dayBefore(r.Due) } }));
    const staged = await tool('update_rows', { table: 'Tasks', changes });
    const links = found.rows.map((r) => `[${r.Title}](learn://task/${r.id})`).join(' and ');
    return `I've staged ${staged.staged} moves: ${links} each go to the day before.`;
  },
  // A general-knowledge question from anywhere in Learn.
  async fourier() {
    const found = await tool('wiki_search', { query: 'Fourier transform' });
    const title = found.results[0].title;
    return `A Fourier transform breaks a signal into the frequencies that make it up.\n\nWikipedia: [${title}](wiki://${title.replace(/ /g, '_')})`;
  },
  // In the Wiki tab: cards from the section being read, kept as a note.
  async flashcards(request) {
    const screen = JSON.parse(request.split('On screen now:\n')[1].split('\n\nTheir request:')[0]);
    const article = await tool('wiki_get_article', { title: screen.wiki.title, section: screen.wiki.section });
    const made = await tool('create_rows', {
      table: 'Notes',
      rows: [{ Title: `Flashcards: ${article.title}`, Markdown: `Q: Who?\nA: ${article.text.split('\n').pop()}` }],
    });
    return `Staged a note of cards from ${screen.wiki.section}: ${made.ids[0]}.`;
  },
  // A task made and put on the day in one go: the block names a row that doesn't exist yet.
  async 'create-and-schedule'() {
    const made = await tool('create_rows', { table: 'Tasks', rows: [{ Title: 'Vocabulary cards', Course: 'JPN 101', 'Estimate (min)': 30 }] });
    await tool('schedule_block', { task: made.ids[0], date: '2026-10-07', start: '14:00', minutes: 30 });
    return 'Staged a new task and a block for it at 2 PM.';
  },
  // A mistake a tool names, put right on the next call.
  async 'wrong-then-right'() {
    const found = await tool('search', { query: 'kanji' });
    const id = found.results[0].id;
    const first = await tool('update_rows', { table: 'Tasks', changes: [{ id, set: { Deadline: '2026-10-08' } }] });
    const second = await tool('update_rows', { table: 'Tasks', changes: [{ id, set: { Heat: 9 } }] });
    const third = await tool('update_rows', { table: 'Tasks', changes: [{ id, set: { Due: 'someday' } }] });
    await tool('update_rows', { table: 'Tasks', changes: [{ id, set: { Due: '2026-10-08' } }] });
    return JSON.stringify([first.error, second.error, third.error]);
  },
  // Mail leaves this Mac: a reply, a thread filed and a new mail, each asked about on its own.
  async mail() {
    const threads = await tool('list_rows', { table: 'Mail', columns: ['Subject', 'From'] });
    const id = threads.rows[0].id;
    const out = { row: await tool('get_row', { table: 'Mail', id }) };
    out.reply = await tool('mail_reply', { thread: id, body: 'Thank you.\nI will be there at 2.' });
    out.filed = await tool('mail_file', { thread: id, action: 'archive' });
    out.sent = await tool('mail_send', { to: 'p@uri.edu', subject: 'Office hours', body: 'Could we meet Tuesday?' });
    out.missing = await tool('mail_reply', { thread: 'nope', body: 'x' });
    return JSON.stringify(out);
  },
  async public() {
    const found = await tool('search', { query: 'essay' });
    const asked = await tool('make_public', { table: 'Tasks', id: found.results[0].id, public: true });
    await tool('update_rows', { table: 'Tasks', changes: [{ id: found.results[0].id, set: { Notes: 'Shared draft' } }] });
    return `That is waiting for your answer. ${asked.note}`;
  },
  // Every kind of change the box can stage, each once.
  async everything() {
    const tasks = await tool('list_rows', { table: 'Tasks', sorts: [{ column: 'Title' }], columns: ['Title'] });
    const id = (title) => tasks.rows.find((r) => r.Title === title).id;
    const out = {};
    out.done = await tool('complete_tasks', { ids: [id('Lab report')] });
    out.deleted = await tool('delete_rows', { table: 'Tasks', ids: [id('Laundry')] });
    out.capture = await tool('capture', { text: 'Ask about office hours' });
    out.formula = await tool('add_formula_column', { table: 'Tasks', name: 'Hours', formula: '[Estimate (min)] / 60' });
    out.badFormula = await tool('add_formula_column', { table: 'Tasks', name: 'Bad', formula: '[Nope] * 2' });
    out.view = await tool('save_view', {
      table: 'Tasks', name: 'Open by course', filters: [{ column: 'Done', op: 'eq', value: false }],
      sorts: [{ column: 'Due' }], group: 'Course',
      pivot: { rows: ['Course'], values: [{ column: 'Estimate (min)', agg: 'sum' }] },
      chart: { type: 'bar', x: 'Course', y: ['Estimate (min)'], agg: 'sum' },
    });
    out.table = await tool('create_table', {
      name: 'Reading list', columns: [{ name: 'Book', type: 'text' }, { name: 'Pages', type: 'number' }],
      rows: [{ Book: 'Genki I', Pages: 384 }],
    });
    out.plan = await tool('plan_day', {});
    out.total = await tool('summarize', { table: 'Tasks', group_by: ['Course'], values: [{ column: 'Estimate (min)', total: 'sum' }] });
    out.today = await tool('today', {});
    out.row = await tool('get_row', { table: 'Tasks', id: id('Kanji quiz') });
    out.open = await tool('open', { what: 'tab', tab: 'database' });
    return JSON.stringify(out);
  },
  // What the run was given, for the test to read.
  async echo(request) {
    const listed = await rpc('tools/list', {});
    return JSON.stringify({
      request,
      system,
      allowed,
      flags: argv.filter((a) => a.startsWith('--')),
      builtIn: flag('--tools'),
      tools: listed.result.tools.map((t) => t.name),
      model: flag('--model'),
    });
  },
  // Who may call the tools: nobody without this run's token, and no web page.
  async trespass() {
    const base = { 'content-type': 'application/json' };
    const call = (headers) => rpc('tools/list', {}, { ...base, ...headers }).then((r) => r.status);
    const get = await fetch(server.url, { headers: server.headers }).then((r) => r.status);
    return JSON.stringify({
      none: await call({}),
      wrong: await call({ Authorization: 'Bearer 0000' }),
      page: await call({ ...server.headers, Origin: 'https://evil.example' }),
      right: await call(server.headers),
      get,
      unknown: (await rpc('resources/list', {})).error?.code,
      elsewhere: await fetch(server.url.replace('/mcp', '/other'), { method: 'POST', headers: server.headers, body: '{}' }).then((r) => r.status),
    });
  },
  async slow() {
    await new Promise((r) => setTimeout(r, 30_000));
    return 'too late';
  },
  async 'signed-out'() {
    console.log(JSON.stringify({ type: 'result', is_error: true, result: 'Invalid API key · Please run /login' }));
    process.exit(1);
  },
  async plain() {
    return 'Just words.';
  },
};

let request = '';
process.stdin.setEncoding('utf8');
for await (const chunk of process.stdin) request += chunk;
const tag = /#([a-z-]+)\s*$/.exec(request.trim())?.[1] ?? 'plain';
// A run with no tools of the core's (Mail's own worker, say) has nobody to greet.
if (server.url) await rpc('initialize', { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'fake-claude', version: '0' } });
const result = await scenarios[tag](request);
console.log(JSON.stringify({ type: 'result', subtype: 'success', is_error: false, result, num_turns: 2 }));
