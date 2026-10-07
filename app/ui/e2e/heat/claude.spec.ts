/// <reference types="node" />
// Heat through the real core and Claude's helper at once (docs/PLAN.md S2.8,
// docs/SPEC.md 8.8): the core runs behind the dev bridge on a fresh library,
// and wi-mcp, started as Claude Desktop would start it, writes to the same
// library from its own process. What a fail looks like: Claude's task not
// reaching the core while it runs; its entry not reading "Undo Claude's
// task"; ⌘Z in Heat not taking it back.

import { spawn } from 'node:child_process';
import { join } from 'node:path';
import { expect, test } from '../shell/kit';

interface Reply {
  result?: { isError: boolean; structuredContent?: Record<string, unknown>; content: { text: string }[] };
}

/** One tool call through a fresh wi-mcp, as Claude Desktop makes it. */
function claude(tool: string, args: Record<string, unknown>): Promise<Reply> {
  return new Promise((ok, fail) => {
    const helper = spawn(join(process.env.WI_E2E_ROOT!, 'target/debug/wi-mcp'), ['--library', process.env.WI_E2E_LIBRARY!]);
    let out = '';
    const timer = setTimeout(() => {
      helper.kill();
      fail(new Error(`wi-mcp answered nothing: ${out}`));
    }, 15_000);
    helper.stdout.on('data', (d) => {
      out += d;
      const lines = out.split('\n').filter(Boolean);
      if (lines.length >= 2) {
        clearTimeout(timer);
        helper.kill();
        ok(JSON.parse(lines[1]) as Reply);
      }
    });
    helper.on('error', fail);
    const send = (m: unknown) => helper.stdin.write(`${JSON.stringify(m)}\n`);
    send({ jsonrpc: '2.0', id: 1, method: 'initialize', params: { protocolVersion: '2025-06-18', capabilities: {}, clientInfo: { name: 'e2e', version: '1' } } });
    send({ jsonrpc: '2.0', method: 'notifications/initialized' });
    send({ jsonrpc: '2.0', id: 2, method: 'tools/call', params: { name: tool, arguments: args } });
  });
}

interface Snapshot {
  records: { task: { id: string; title: string; source: string }[]; space: { id: string; name: string }[] };
}

test("a task Claude adds through MCP reaches the running core as Claude's, and ⌘Z takes it back", async ({ core }) => {
  const today = new Date().toISOString().slice(0, 10);
  const snap = await core.call<Snapshot>('heat.snapshot', { date: today });
  if (!snap.records.space.length) {
    await core.call('heat.put', {
      kind: 'space',
      record: { name: 'Classes', hue: 210, groupKind: 'course', groupLabel: 'Course', types: ['Homework', 'Quiz'], persona: '' },
    });
  }

  const reply = await claude('add_task', { title: 'Grammar quiz 4', type: 'Quiz', reason: 'The notice gives a Friday deadline.' });
  expect(reply.result?.isError, reply.result?.content[0]?.text).toBe(false);
  expect(reply.result?.structuredContent?.undo_label).toBe("Undo Claude's task");

  // The core reads the same library: the task is there, made by Claude.
  await expect
    .poll(async () => (await core.call<Snapshot>('heat.snapshot', { date: today })).records.task.find((t) => t.title === 'Grammar quiz 4')?.source, { timeout: 10_000 })
    .toBe('claude');

  // The Edit menu names whose change it is, and ⌘Z in Heat takes it back.
  const menu = await core.call<{ undo: string | null }>('history.get', { room: 'heat' });
  expect(menu.undo).toBe("Undo Claude's task");
  await core.call('history.undo', { room: 'heat' });
  const after = await core.call<Snapshot>('heat.snapshot', { date: today });
  expect(after.records.task.find((t) => t.title === 'Grammar quiz 4')).toBeUndefined();
});

test('a switched-off tool is refused, and Settings → Claude reads the switch back', async ({ core }) => {
  await core.call('heat.claude.setTool', { name: 'log_focus', on: false });
  const reply = await claude('log_focus', { task_id: 'nope', minutes: 5, reason: 'r' });
  expect(reply.result?.isError).toBe(true);
  expect(reply.result?.content[0]?.text).toBe('This tool is switched off in Wi_WWAV.');
  const settings = await core.call<{ tools: { name: string; on: boolean }[] }>('heat.claude.get');
  expect(settings.tools.find((t) => t.name === 'log_focus')?.on).toBe(false);
  await core.call('heat.claude.setTool', { name: 'log_focus', on: true });
});
