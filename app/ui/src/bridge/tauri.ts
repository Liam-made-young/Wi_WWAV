// The app: one Tauri command, `core`, takes {cmd, args} and answers the
// result or rejects with {code, message}. Events and meters come over two
// Tauri channels the window hands the app once, through `core_listen`; the
// meters channel carries raw bytes, never JSON (docs/SPEC.md 9.2).

import { Channel, invoke } from '@tauri-apps/api/core';
import { type Args, type Transport, coreError } from './types';

export function tauriTransport(): Transport {
  return {
    async call(cmd: string, args: Args) {
      try {
        return await invoke('core', { cmd, args });
      } catch (e) {
        throw coreError(e);
      }
    },
    listen(deliver) {
      const events = new Channel<{ event: string; payload: unknown }>();
      events.onmessage = (e) => deliver.event(e.event, e.payload);
      const meters = new Channel<ArrayBuffer>();
      meters.onmessage = (frame) => deliver.meters(frame);
      void invoke('core_listen', { events, meters });
    },
  };
}
