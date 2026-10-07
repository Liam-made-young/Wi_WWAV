// `npm run dev` with VITE_FAKE_CORE=1 (main.tsx): the whole UI runs on the
// fake core with the seeded sample, so Heat can be seen and driven with no
// Rust behind it. The fake answers the bridge's calls: Heat's `heat.*` and
// `history.*` from the fake, and the few the shell asks at start from here.
//
//   ?fakeNow=2026-10-07T10:00:00-04:00   starts the fake's clock there (it then runs at the machine's pace)
//   ?fakeZone=America/New_York           the person's time zone
//
// Nothing in a production build reaches this file: main.tsx loads it with a
// dynamic import behind a compile-time switch (vite.config.ts).

import { CoreError, installStandIn } from '../../bridge';
import type { Transport as BridgeTransport } from '../../bridge/types';
import { createFake, type Fake } from './all';
import { FakeError } from './core';
import { seed } from './seed';
import { setEvents } from './today';

const SETTINGS = {
  appearance: 'system',
  textSize: 13,
  reduceMotion: false,
  library: { watchedFolders: [], leaveInPlace: false },
  audio: { device: null, buffer: 256, pluginFolders: [] },
  video: { hardwareEncode: true, proxyMedia: false },
  claude: { scoring: 'unasked', mail: 'unasked', feedback: 'unasked', clerk: 'unasked' },
  heat: { timeZone: null, school: null },
};

export function installFakeCore(options: { now?: number; zone?: string } = {}): Fake {
  const params = new URLSearchParams(typeof location === 'undefined' ? '' : location.search);
  const fromParam = params.get('fakeNow') ? Date.parse(params.get('fakeNow')!) : NaN;
  const start = options.now ?? (Number.isFinite(fromParam) ? fromParam : Date.now());
  const zone = options.zone ?? params.get('fakeZone') ?? Intl.DateTimeFormat().resolvedOptions().timeZone;

  const sample = seed(start, zone);
  const { fake, transport } = createFake(sample.records, { now: start, zone });
  fake.state.currentTaskId = sample.currentTaskId;
  setEvents(fake, sample.events);

  // The fake's clock starts at `start` and then runs at the machine's pace.
  let base = start;
  let since = Date.now();
  Object.defineProperty(fake, 'now', {
    configurable: true,
    get: () => base + (Date.now() - since),
    set: (ms: number) => {
      base = ms;
      since = Date.now();
    },
  });

  let settings: Record<string, unknown> = { ...SETTINGS };
  const historyOf = () =>
    transport.call<{ undo: string | null; redo: string | null; cant: string | null }>('history.get', {});
  const refused = (e: unknown): never => {
    throw e instanceof FakeError ? new CoreError(e.code, e.message) : e;
  };

  const own: Record<string, (args: Record<string, unknown>) => unknown> = {
    'app.hello': () => ({
      version: '0.1.0 (fake core)',
      library: '~/Music/Wi_WWAV (fake core)',
      signedIn: false,
      platform: 'web',
      reduceMotion: false,
    }),
    'app.settings.get': () => settings,
    'app.settings.set': (a) => (settings = { ...settings, ...(a.patch as object) }),
    'account.status': () => ({ signedIn: false }),
    'engine.status': () => ({ state: 'stopped', device: null, sampleRate: 48000, block: 128 }),
    'history.redo': () => {
      throw new CoreError('nothing_to_redo', 'Nothing to redo.');
    },
  };

  const standIn: BridgeTransport = {
    async call(cmd, args) {
      if (cmd === 'history.get' && args.room !== 'heat') return { undo: null, redo: null, cant: null };
      const answer = own[cmd];
      if (answer) return answer(args);
      if (cmd.startsWith('heat.') || cmd.startsWith('history.')) return transport.call(cmd, args).catch(refused);
      throw new CoreError('not_in_fake_core', `The fake core has no ${cmd}.`);
    },
    listen(deliver) {
      transport.on('heat', (payload) => {
        deliver.event('heat', payload);
        void historyOf().then((h) => deliver.event('history', { room: 'heat', ...h }));
      });
    },
  };
  installStandIn(standIn);
  // End-to-end specs move the clock on: as if minutes had passed, without waiting for them.
  (window as unknown as { __wiFake: unknown }).__wiFake = {
    fake,
    advance(ms: number) {
      fake.now += ms;
      fake.emit([]);
    },
  };
  return fake;
}
