import { describe, expect, test } from 'vitest';
import { CoreError, createBridge } from './index';
import { type SocketLike, socketTransport } from './socket';

// A WebSocket that records what is sent and lets the test answer.
class FakeSocket implements SocketLike {
  binaryType = 'blob';
  readyState = 0;
  sent: string[] = [];
  onopen: SocketLike['onopen'] = null;
  onmessage: SocketLike['onmessage'] = null;
  onclose: SocketLike['onclose'] = null;
  onerror: SocketLike['onerror'] = null;

  send(data: string) {
    this.sent.push(data);
  }
  close() {
    this.readyState = 3;
    this.onclose?.({});
  }
  open() {
    this.readyState = 1;
    this.onopen?.({});
  }
  receive(data: unknown) {
    this.onmessage?.({ data: typeof data === 'string' || data instanceof ArrayBuffer ? data : JSON.stringify(data) });
  }
  lastRequest() {
    return JSON.parse(this.sent[this.sent.length - 1]);
  }
}

function setup() {
  const sockets: FakeSocket[] = [];
  const bridge = createBridge(
    socketTransport(() => {
      const s = new FakeSocket();
      sockets.push(s);
      return s;
    }),
  );
  return { bridge, sockets, socket: () => sockets[sockets.length - 1] };
}

describe('the dev bridge transport', () => {
  test('a call waits for the socket to open, then sends {id, cmd, args}', async () => {
    const { bridge, socket } = setup();
    const answer = bridge.call('player.seek', { seconds: 42 });
    expect(socket().sent).toEqual([]);
    socket().open();
    expect(socket().lastRequest()).toEqual({ id: 1, cmd: 'player.seek', args: { seconds: 42 } });
    socket().receive({ id: 1, ok: true, result: { state: { position: 42 } } });
    await expect(answer).resolves.toEqual({ state: { position: 42 } });
  });

  test('answers in any order reach their own calls', async () => {
    const { bridge, socket } = setup();
    const a = bridge.call('library.list');
    const b = bridge.call('history.get', { room: 'heat' });
    socket().open();
    expect(socket().sent.map((s) => JSON.parse(s).id)).toEqual([1, 2]);
    socket().receive({ id: 2, ok: true, result: { undo: 'Undo mark done' } });
    socket().receive({ id: 1, ok: true, result: { clips: [] } });
    await expect(a).resolves.toEqual({ clips: [] });
    await expect(b).resolves.toEqual({ undo: 'Undo mark done' });
  });

  test("an error is a CoreError with the core's code and sentence", async () => {
    const { bridge, socket } = setup();
    const p = bridge.call('history.undo', { room: 'heat' });
    socket().open();
    socket().receive({ id: 1, ok: false, error: { code: 'nothing_to_undo', message: 'Nothing to undo.' } });
    const e = await p.catch((e: unknown) => e);
    expect(e).toBeInstanceOf(CoreError);
    expect(e).toMatchObject({ code: 'nothing_to_undo', message: 'Nothing to undo.' });
  });

  test('events reach their handlers until unsubscribed, and meters arrive as bytes', () => {
    const { bridge, socket } = setup();
    const heard: unknown[] = [];
    const off = bridge.on('player', (p) => heard.push(p));
    const frames: ArrayBuffer[] = [];
    bridge.onMeters((f) => frames.push(f));
    socket().open();
    expect(socket().binaryType).toBe('arraybuffer');
    socket().receive({ event: 'player', payload: { playing: true } });
    socket().receive({ event: 'clock', payload: { sample: 4800 } });
    const frame = new ArrayBuffer(32 + 16);
    socket().receive(frame);
    off();
    socket().receive({ event: 'player', payload: { playing: false } });
    expect(heard).toEqual([{ playing: true }]);
    expect(frames).toEqual([frame]);
  });

  test('a closed socket fails what was waiting, and the next call reconnects', async () => {
    const { bridge, sockets, socket } = setup();
    const lost = bridge.call('account.signIn');
    socket().open();
    socket().close();
    await expect(lost).rejects.toMatchObject({ code: 'bridge_closed' });
    const again = bridge.call('app.hello');
    expect(sockets).toHaveLength(2);
    socket().open();
    socket().receive({ id: 2, ok: true, result: { version: '0.1.0' } });
    await expect(again).resolves.toEqual({ version: '0.1.0' });
  });
});
