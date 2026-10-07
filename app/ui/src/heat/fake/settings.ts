// The fake core's School sheet and calendars (docs/HEAT.md, docs/SPEC.md
// 3.11): the school's name, Brightspace host, course-code pattern and term
// dates, and the calendars that come in as private iCal addresses. An address
// goes to the Keychain and never comes back: a calendar keeps a reference,
// and the school only says whether a link is saved.

import type { Calendar, Id, School } from '../client';
import { derive, type Fake, refuse, register } from './core';

const schools = new WeakMap<Fake, School>();

derive((snap, fake) => {
  snap.school = schools.get(fake) ?? null;
});

register('heat.school.set', (args, fake) => {
  const name = String(args.name ?? '').trim();
  const pattern = String(args.codePattern ?? '');
  if (!name) refuse('Give the school a name first.');
  try {
    new RegExp(pattern.replace(/^\/|\/[a-z]*$/g, ''));
  } catch {
    refuse('That course-code pattern isn’t a pattern Heat can read.');
  }
  const was = schools.get(fake);
  const url = typeof args.icalUrl === 'string' ? args.icalUrl.trim() : '';
  schools.set(fake, {
    name,
    host: String(args.host ?? '').trim(),
    icalSaved: url !== '' || (was?.icalSaved ?? false),
    codePattern: pattern,
    termStart: String(args.termStart ?? ''),
    termEnd: String(args.termEnd ?? ''),
  });
  if (url !== '' && ![...fake.store.calendar.values()].some((c) => c.kind === 'brightspace')) {
    const calendar: Calendar = {
      id: fake.newId(),
      name: 'Brightspace',
      kind: 'brightspace',
      keychainRef: 'keychain:brightspace',
      lastSyncedAt: null,
    };
    fake.write('add calendar', ['calendar'], () => fake.store.calendar.set(calendar.id, calendar));
  }
  fake.emit([]);
  return {};
});

register('heat.calendars.add', (args, fake) => {
  const name = String(args.name ?? '').trim();
  if (!name) refuse('Give the calendar a name first.');
  if (!/^(https?|webcal):\/\//i.test(String(args.url ?? '')))
    refuse('An iCal address starts with https:// or webcal://.');
  const id = fake.newId();
  const calendar: Calendar = {
    id,
    name,
    kind: args.kind === 'brightspace' ? 'brightspace' : 'ical',
    keychainRef: `keychain:${id}`,
    lastSyncedAt: null,
  };
  fake.write('add calendar', ['calendar'], () => fake.store.calendar.set(id, calendar));
  return { calendar };
});

register('heat.calendars.remove', (args, fake) => {
  const id = args.id as Id;
  if (!fake.store.calendar.has(id)) refuse('No calendar has that id.');
  fake.write('delete calendar', ['calendar'], () => fake.store.calendar.delete(id));
  return {};
});
