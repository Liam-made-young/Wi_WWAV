import { afterEach, describe, expect, it } from 'vitest';
import { $, $$, button, click, mountWith, type Rig, settle, type } from '../heat/testkit';
import type { call } from '../bridge';
import { SchoolSheet } from './SchoolSheet';

// docs/SPEC.md 2.13 and 3.11 against Learn's fake core. What a fail looks
// like: a link that is shown again after it is saved, a Brightspace link
// that doesn't go through `heat.school.set`, another calendar that isn't
// added as `ical`, or a refusal that isn't said in the core's own sentence.

let rig: Rig;
afterEach(() => rig?.unmount());

const field = (name: string) => $(rig, `input[aria-label="${name}"]`) as HTMLInputElement;
/** The sheet on a fake core with no calendars and no school yet. */
async function mount() {
  rig = await mountWith(null, { empty: true });
  rig.rerender(<SchoolSheet ask={rig.call as typeof call} />);
  await settle();
}

const said = () => $(rig, '[role="status"]')?.textContent ?? null;

describe('Settings → Learn’s School sheet', () => {
  it('saves the school and its Brightspace link, and never shows the link again', async () => {
    await mount();
    expect(field('Course-code pattern').value).toBe('^([A-Z]{3})\\s?(\\d{3})');
    expect(field('Brightspace calendar link').type).toBe('password');
    await type(field('School name'), 'University of Rhode Island');
    await type(field('Brightspace host'), 'brightspace.uri.edu');
    await type(field('Brightspace calendar link'), 'webcal://brightspace.uri.edu/d2l/le/calendar/feed/user/feed.ics?token=tok3n-abc');
    await click(button(rig, 'Save'));
    await settle();

    const calendars = [...rig.fake.store.calendar.values()];
    expect(calendars.map((c) => [c.name, c.kind])).toEqual([['Brightspace', 'brightspace']]);
    expect(JSON.stringify(calendars)).not.toContain('tok3n-abc');
    expect(field('Brightspace calendar link').value).toBe('');
    expect(field('Brightspace calendar link').placeholder).toBe('Saved in the keychain. Paste a new link to replace it.');
    expect(rig.host.innerHTML).not.toContain('tok3n-abc');
    expect(said()).not.toBeNull();
    const snap = await rig.call<{ school: { name: string; host: string; icalSaved: boolean } }>('heat.snapshot', {
      date: '2026-10-07',
    });
    expect(snap.school).toMatchObject({ name: 'University of Rhode Island', host: 'brightspace.uri.edu', icalSaved: true });

    await click(button(rig, 'Remove the link'));
    await settle();
    expect(rig.fake.store.calendar.size).toBe(0);
    expect(said()).toBe('Removed the Brightspace calendar. The tasks it made stay.');
  });

  it('adds another calendar as an iCal address, lists it, and removes it', async () => {
    await mount();
    expect(button(rig, 'Add')!.hasAttribute('disabled')).toBe(true);
    expect(button(rig, 'Read the calendars now')!.hasAttribute('disabled')).toBe(true);
    await type(field('Calendar name'), 'Work');
    await type(field('Calendar iCal address'), 'https://calendar.google.com/calendar/ical/x/private-k3y-xyz/basic.ics');
    await click(button(rig, 'Add'));
    await settle();
    expect([...rig.fake.store.calendar.values()].map((c) => [c.name, c.kind])).toEqual([['Work', 'ical']]);
    expect($$(rig, '.calendars li').map((li) => li.querySelector('.calendar-name')!.textContent)).toEqual(['Work']);
    expect(field('Calendar iCal address').value).toBe('');
    expect(rig.host.innerHTML).not.toContain('private-k3y-xyz');

    await click(button(rig, 'Remove Work'));
    await settle();
    expect($$(rig, '.calendars li')).toEqual([]);
    expect(said()).toBe('Removed Work. Its events are gone from the time column.');
  });

  it('says the core’s own sentence when an address isn’t one', async () => {
    await mount();
    await type(field('Calendar name'), 'Work');
    await type(field('Calendar iCal address'), 'calendar.google.com/x');
    await click(button(rig, 'Add'));
    await settle();
    expect(said()).toBe('An iCal address starts with https:// or webcal://.');
    expect(rig.fake.store.calendar.size).toBe(0);
    // What was typed stays, to be fixed rather than typed again.
    expect(field('Calendar name').value).toBe('Work');
  });
});
