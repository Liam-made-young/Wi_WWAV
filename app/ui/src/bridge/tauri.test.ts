import { beforeEach, describe, expect, test, vi } from 'vitest';
import { CoreError } from './index';

// The window's side of app/src-tauri: what it asks of Tauri, and what Tauri
// says back. @tauri-apps/api needs a real window's internals, so the two
// modules it would call are stood in for.
const calls: { cmd: string; args: unknown }[] = [];
let refuse: unknown = null;
const listeners = new Map<string, (e: { payload: unknown }) => void>();
let nextChannel = 7;

vi.mock('@tauri-apps/api/core', () => ({
  invoke: async (name: string, args: unknown) => {
    calls.push({ cmd: name, args });
    if (refuse) throw refuse;
    return { ran: name };
  },
  Channel: class {
    id = nextChannel++;
    onmessage: (m: unknown) => void = () => {};
    toJSON() {
      return `__CHANNEL__:${this.id}`;
    }
  },
}));
vi.mock('@tauri-apps/api/event', () => ({
  listen: async (event: string, handler: (e: { payload: unknown }) => void) => {
    listeners.set(event, handler);
    return () => listeners.delete(event);
  },
}));

const { tauriTransport } = await import('./tauri');

beforeEach(() => {
  calls.length = 0;
  listeners.clear();
  refuse = null;
});

describe('the Tauri transport', () => {
  test('a call is the one command, `core`, with {cmd, args}', async () => {
    await expect(tauriTransport().call('player.seek', { seconds: 4 })).resolves.toEqual({ ran: 'core' });
    expect(calls).toEqual([{ cmd: 'core', args: { cmd: 'player.seek', args: { seconds: 4 } } }]);
  });

  test("a refusal is a CoreError with the core's code and sentence", async () => {
    refuse = { code: 'not_allowed', message: "This window can't use library.delete." };
    const e = await tauriTransport()
      .call('library.delete', {})
      .catch((e: unknown) => e);
    expect(e).toBeInstanceOf(CoreError);
    expect(e).toMatchObject({ code: 'not_allowed' });
  });

  test('events come on the Tauri event `core` as {event, payload}', async () => {
    const heard: [string, unknown][] = [];
    tauriTransport().listen({ event: (name, payload) => heard.push([name, payload]), meters: () => {} });
    await Promise.resolve();
    listeners.get('core')!({ payload: { event: 'history', payload: { room: 'heat', undo: 'Undo mark done' } } });
    expect(heard).toEqual([['history', { room: 'heat', undo: 'Undo mark done' }]]);
  });

  test('meters come on a channel the page names with meters.listen, as bytes', () => {
    const frames: ArrayBuffer[] = [];
    tauriTransport().listen({ event: () => {}, meters: (f) => frames.push(f) });
    const [ask] = calls;
    expect(ask).toEqual({ cmd: 'core', args: { cmd: 'meters.listen', args: { channel: expect.anything() } } });
    const channel = (ask.args as { args: { channel: { id: number; toJSON(): string; onmessage(m: unknown): void } } })
      .args.channel;
    // What the shell receives is the channel's name, as JSON writes it.
    expect(JSON.parse(JSON.stringify({ channel })).channel).toBe(`__CHANNEL__:${channel.id}`);
    const frame = new ArrayBuffer(48);
    channel.onmessage(frame);
    expect(frames).toEqual([frame]);
  });
});
