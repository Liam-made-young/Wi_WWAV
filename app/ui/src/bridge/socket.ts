// The dev bridge (crates/wi-devbridge): the same core over a WebSocket, so the
// UI runs in any browser and Playwright drives the real core. Commands go out
// as {id, cmd, args} and come back as {id, ok, result | error}; events arrive
// as {event, payload}; meters as binary frames.

import { type Args, CoreError, type Transport, coreError } from './types';

/** What the transport needs of a WebSocket, so a test can hand it a fake. */
export interface SocketLike {
  binaryType: string;
  readyState: number;
  onopen: ((e: unknown) => void) | null;
  onmessage: ((e: { data: unknown }) => void) | null;
  onclose: ((e: unknown) => void) | null;
  onerror: ((e: unknown) => void) | null;
  send(data: string): void;
  close(): void;
}

const OPEN = 1;

interface Pending {
  resolve(v: unknown): void;
  reject(e: CoreError): void;
}

export function socketTransport(open: () => SocketLike): Transport {
  let socket: SocketLike | null = null;
  let nextId = 1;
  const pending = new Map<number, Pending>();
  const queued: string[] = [];
  let deliver: Parameters<Transport['listen']>[0] | null = null;

  function connect(): SocketLike {
    if (socket) return socket;
    const s = open();
    s.binaryType = 'arraybuffer';
    s.onopen = () => {
      for (const frame of queued.splice(0)) s.send(frame);
    };
    s.onmessage = ({ data }) => {
      if (data instanceof ArrayBuffer) {
        deliver?.meters(data);
        return;
      }
      const msg = JSON.parse(String(data));
      if ('event' in msg) {
        deliver?.event(msg.event, msg.payload);
        return;
      }
      const waiting = pending.get(msg.id);
      if (!waiting) return;
      pending.delete(msg.id);
      if (msg.ok) waiting.resolve(msg.result);
      else waiting.reject(coreError(msg.error));
    };
    s.onclose = () => {
      // Whatever was asked and not answered won't be; the next call reconnects.
      socket = null;
      queued.length = 0;
      const lost = new CoreError('bridge_closed', 'The connection to the app’s core closed. Try again.');
      for (const p of pending.values()) p.reject(lost);
      pending.clear();
    };
    s.onerror = () => {};
    socket = s;
    return s;
  }

  return {
    call(cmd: string, args: Args) {
      const id = nextId++;
      const frame = JSON.stringify({ id, cmd, args });
      return new Promise((resolve, reject) => {
        pending.set(id, { resolve, reject });
        const s = connect();
        if (s.readyState === OPEN) s.send(frame);
        else queued.push(frame);
      });
    },
    listen(d) {
      deliver = d;
      connect();
    },
  };
}
