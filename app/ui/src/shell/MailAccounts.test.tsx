import { afterEach, describe, expect, it } from 'vitest';
import type { call } from '../bridge';
import { $, $$, button, click, mountWith, type Rig, settle, type } from '../heat/testkit';
import { MailAccounts } from './MailAccounts';

// docs/SPEC.md 3.10 against Learn's fake core. What a fail looks like: an
// account that isn't saved through `heat.mail.accounts.set` as the whole
// list, a forwarded account with nowhere to be forwarded to, or a refusal
// that isn't the core's own sentence.

let rig: Rig;
afterEach(() => rig?.unmount());

async function mount() {
  rig = await mountWith(null, { empty: true });
  rig.rerender(<MailAccounts ask={rig.call as typeof call} />);
  await settle();
}
const field = (name: string) => $(rig, `[aria-label="${name}"]`) as HTMLInputElement;
const said = () => $(rig, '[role="status"]')?.textContent ?? null;
const kept = async () => (await rig.call<{ mailAccounts: unknown[] }>('heat.snapshot', { date: '2026-10-07' })).mailAccounts;

async function choose(el: HTMLElement, value: string) {
  const select = el as unknown as HTMLSelectElement;
  select.value = value;
  select.dispatchEvent(new Event('change', { bubbles: true }));
  await settle();
}

describe('Settings → Learn’s mail accounts', () => {
  it('adds the connector’s own account and a forwarded one, and removes one', async () => {
    await mount();
    expect(button(rig, 'Add')!.hasAttribute('disabled')).toBe(true);
    await type(field('Mail address'), 'Made.LiamYoung@gmail.com');
    await type(field('Account name'), 'Personal');
    await click(button(rig, 'Add'));
    await settle();
    expect(await kept()).toEqual([{ address: 'made.liamyoung@gmail.com', name: 'Personal', via: 'connector' }]);
    expect(said()).toBe('Added made.liamyoung@gmail.com. Ask Claude to read your mail.');
    expect(field('Mail address').value).toBe('');

    await type(field('Mail address'), 'liam.young@uri.edu');
    await choose(field('How its mail arrives'), 'forward');
    // A forwarded account can't be added until it says where to.
    expect(button(rig, 'Add')!.hasAttribute('disabled')).toBe(true);
    await type(field('Forwarded to'), 'made.liamyoung+uri@gmail.com');
    await click(button(rig, 'Add'));
    await settle();
    expect(await kept()).toEqual([
      { address: 'made.liamyoung@gmail.com', name: 'Personal', via: 'connector' },
      { address: 'liam.young@uri.edu', name: 'liam.young@uri.edu', via: 'forward', forwardTo: 'made.liamyoung+uri@gmail.com' },
    ]);
    expect($$(rig, '.mail-accounts li')).toHaveLength(2);
    expect($$(rig, '.mail-accounts li')[1].textContent).toContain('forwarded to made.liamyoung+uri@gmail.com');

    await click(button(rig, 'Remove made.liamyoung@gmail.com'));
    await settle();
    expect((await kept()).length).toBe(1);
    expect(said()).toBe('Removed made.liamyoung@gmail.com. What was recorded from it stays in Mail.');
  });

  it('says the core’s own sentence and keeps what was typed', async () => {
    await mount();
    await type(field('Mail address'), 'a@uri.edu');
    await click(button(rig, 'Add'));
    await settle();
    await type(field('Mail address'), 'A@uri.edu');
    await click(button(rig, 'Add'));
    await settle();
    expect(said()).toBe('a@uri.edu is in the list twice.');
    expect(field('Mail address').value).toBe('A@uri.edu');
    expect((await kept()).length).toBe(1);
  });
});
