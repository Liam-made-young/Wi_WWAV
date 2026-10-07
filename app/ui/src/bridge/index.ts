// How the UI reaches the app's core (docs/COMMANDS.md). The same calls
// travel two ways: through Tauri when the UI runs inside the app, and
// through wi-devbridge's WebSocket anywhere else (the dev server,
// Playwright). Nothing outside this folder knows which.
//
//   const { state } = await call('player.play');
//   const off = on('player', (state) => …);
//   onMeters((frame) => …);

import { type SocketLike, socketTransport } from './socket';
import { tauriTransport } from './tauri';
import type { Args, EventHandler, MetersHandler, Transport } from './types';

export { CoreError } from './types';
export type { Args, EventHandler, MetersHandler, Transport } from './types';

export interface Bridge {
  call<T = unknown>(cmd: string, args?: Args): Promise<T>;
  /** Hears one event until the returned function is called. */
  on(event: string, handler: EventHandler): () => void;
  /** Hears the meters, raw bytes once a frame, until the returned function is called. */
  onMeters(handler: MetersHandler): () => void;
}

export function createBridge(transport: Transport): Bridge {
  const handlers = new Map<string, Set<EventHandler>>();
  const meters = new Set<MetersHandler>();
  let listening = false;

  function listen() {
    if (listening) return;
    listening = true;
    transport.listen({
      event(name, payload) {
        for (const h of handlers.get(name) ?? []) h(payload);
      },
      meters(frame) {
        for (const h of meters) h(frame);
      },
    });
  }

  return {
    call: <T>(cmd: string, args: Args = {}) => transport.call(cmd, args) as Promise<T>,
    on(event, handler) {
      listen();
      const set = handlers.get(event) ?? new Set();
      set.add(handler);
      handlers.set(event, set);
      return () => set.delete(handler);
    },
    onMeters(handler) {
      listen();
      meters.add(handler);
      return () => meters.delete(handler);
    },
  };
}

/** The dev bridge's address: VITE_BRIDGE_URL, or its default port. */
export const BRIDGE_URL: string = import.meta.env.VITE_BRIDGE_URL ?? 'ws://localhost:8790';

function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

// SocketLike is the part of WebSocket the transport uses, with plainer
// handler types than the DOM's, so a test's fake fits it too.
const openSocket = () => new WebSocket(BRIDGE_URL) as unknown as SocketLike;
const real = inTauri() ? tauriTransport() : socketTransport(openSocket);

// `npm run dev` with VITE_FAKE_CORE=1 answers from Heat's fake core instead
// (heat/fake/install.ts, loaded by main.tsx before anything renders).
let standIn: Transport | null = null;
export function installStandIn(transport: Transport) {
  standIn = transport;
}

const bridge = createBridge({
  call: (cmd, args) => (standIn ?? real).call(cmd, args),
  listen: (deliver) => (standIn ?? real).listen(deliver),
});

export const call = bridge.call;
export const on = bridge.on;
export const onMeters = bridge.onMeters;
