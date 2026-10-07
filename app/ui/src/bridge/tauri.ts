// The app: the shell (app/src-tauri) gives the window one command, `core`,
// taking {cmd, args} and answering the result or rejecting with {code,
// message}. Events come as one Tauri event, `core`, carrying {event,
// payload}, and meters on a Channel the page names with `core`
// `meters.listen {channel}`, raw bytes, never JSON (docs/SPEC.md 8.2;
// app/src-tauri/README.md has the contract).

import { Channel, invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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
      void listen<{ event: string; payload: unknown }>('core', (e) =>
        deliver.event(e.payload.event, e.payload.payload),
      );
      const meters = new Channel<ArrayBuffer>();
      meters.onmessage = (frame) => deliver.meters(frame);
      void invoke('core', { cmd: 'meters.listen', args: { channel: meters } });
    },
  };
}
