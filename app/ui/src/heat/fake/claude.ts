// The fake core's side of Settings -> Claude (docs/HEAT.md, docs/SPEC.md 3.13):
// the two lines that add Wi_WWAV to Claude's config, a switch per tool (a tool
// switched off is missing from what Claude can see), and Claude's recent
// changes from the journal, each of which `history.undoEntry` can undo. The
// app holds no key and calls no model; Claude comes in through `wi-mcp`.

import { refuse, register } from './core';

export const TOOLS = [
  'list_tasks',
  'add_task',
  'update_task',
  'plan_day',
  'get_grades',
  'add_pending_grade',
  'log_focus',
  'record_mail_thread',
  'get_schedule',
  'draft_block',
  'list_habits',
  'list_projects',
  'add_project',
  'add_milestone',
  'get_notes',
  'add_note',
  'list_inbox',
  'add_capture',
  'list_mail_accounts',
  'list_mail',
  'save_mail_text',
  'list_mail_outbox',
  'finish_mail_action',
] as const;

const HELPER = '/Applications/Wi_WWAV.app/Contents/Helpers/wi-mcp';
const off = new WeakMap<object, Set<string>>();
const switchedOff = (fake: object) => off.get(fake) ?? off.set(fake, new Set()).get(fake)!;

register('heat.claude.get', (_args, fake) => ({
  helper: HELPER,
  desktop: JSON.stringify({ mcpServers: { 'wi-wwav': { command: HELPER } } }, null, 2),
  code: `claude mcp add --scope user wi-wwav -- ${HELPER}`,
  tools: TOOLS.map((name) => ({ name, on: !switchedOff(fake).has(name) })),
  recent: fake.journal
    .filter((e) => e.actor === 'claude')
    .map((e) => ({ txnId: e.id, label: e.label, reason: e.reason ?? '', at: e.at, undone: e.undone }))
    .reverse()
    .slice(0, 20),
}));

register('heat.claude.setTool', (args, fake) => {
  const name = String(args.name);
  if (!(TOOLS as readonly string[]).includes(name)) refuse(`Wi_WWAV offers no tool called ${name}.`);
  if (args.on === true) switchedOff(fake).delete(name);
  else switchedOff(fake).add(name);
  return {};
});
