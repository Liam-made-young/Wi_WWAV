import { afterEach, describe, expect, it } from 'vitest';
import type { call } from '../bridge';
import { $, $$, button, click, mountWith, type Rig, settle } from '../heat/testkit';
import { ClaudePane } from './ClaudePane';
import { CLAUDE_TOOLS } from './claudeTools';

// docs/PLAN.md S2.11 against Learn's fake core. What a fail looks like: the
// lines aren't the core's own, a tool has no switch or its switch doesn't
// reach `heat.claude.setTool`, or a change of Claude's can't be undone here.

let rig: Rig;
afterEach(() => rig?.unmount());

async function mount() {
  rig = await mountWith(null);
  rig.rerender(<ClaudePane ask={rig.call as typeof call} />);
  await settle();
}

describe('Settings → Claude', () => {
  it('shows the core’s own lines and a switch for every tool', async () => {
    await mount();
    const lines = $$(rig, '.claude-line').map((l) => l.textContent);
    expect(lines[0]).toBe('claude mcp add --scope user wi-wwav -- /Applications/Wi_WWAV.app/Contents/Helpers/wi-mcp');
    expect(JSON.parse(lines[1]!)).toEqual({
      mcpServers: { 'wi-wwav': { command: '/Applications/Wi_WWAV.app/Contents/Helpers/wi-mcp' } },
    });
    const switches = $$(rig, '.claude-tools input') as HTMLInputElement[];
    expect(switches.map((s) => s.getAttribute('aria-label'))).toEqual(CLAUDE_TOOLS.map((t) => t.name));
    expect(switches).toHaveLength(23);
    expect(switches.every((s) => s.checked)).toBe(true);
    // Every tool says what it does.
    expect($$(rig, '.claude-tools li').every((li) => li.querySelector('span')!.textContent !== '')).toBe(true);
  });

  it('switches a tool off through heat.claude.setTool', async () => {
    await mount();
    await click($(rig, '.claude-tools input[aria-label="log_focus"]'));
    await settle();
    expect(rig.calls.some((c) => c.cmd === 'heat.claude.setTool' && c.args.name === 'log_focus' && c.args.on === false)).toBe(true);
    expect(($(rig, '.claude-tools input[aria-label="log_focus"]') as HTMLInputElement).checked).toBe(false);
  });

  it('lists Claude’s changes with their reasons, and undoes one', async () => {
    await mount();
    const listed = $$(rig, '.claude-recent li');
    const data = await rig.call<{ recent: { label: string; undone: boolean }[] }>('heat.claude.get');
    expect(listed).toHaveLength(data.recent.length);
    if (data.recent.length === 0) {
      expect(rig.host.textContent).toContain('None yet.');
      return;
    }
    const first = data.recent.find((r) => !r.undone)!;
    await click(button(rig, `Undo ${first.label}`));
    await settle();
    expect($(rig, '[role="status"]')!.textContent).toBe(`Undid ${first.label}.`);
    const after = await rig.call<{ recent: { label: string; undone: boolean }[] }>('heat.claude.get');
    expect(after.recent.filter((r) => r.undone).length).toBe(data.recent.filter((r) => r.undone).length + 1);
  });
});
