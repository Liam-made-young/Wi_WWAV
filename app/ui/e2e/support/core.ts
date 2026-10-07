/// <reference types="node" />
// The test's own line to the app's core, through the dev bridge the global
// setup started: what the UI would call, from Node.

export interface Core {
  call<T = unknown>(cmd: string, args?: Record<string, unknown>): Promise<T>;
  close(): void;
}

export async function connect(url = process.env.WI_E2E_BRIDGE!): Promise<Core> {
  const ws = new WebSocket(url);
  await new Promise((ok, fail) => {
    ws.onopen = ok;
    ws.onerror = () => fail(new Error(`no dev bridge at ${url}`));
  });
  let next = 1;
  const waiting = new Map<number, { ok(v: unknown): void; fail(e: Error): void }>();
  ws.onmessage = ({ data }) => {
    if (typeof data !== 'string') return;
    const msg = JSON.parse(data);
    const w = waiting.get(msg.id);
    if (!w) return;
    waiting.delete(msg.id);
    if (msg.ok) w.ok(msg.result);
    else w.fail(new Error(`${msg.error.code}: ${msg.error.message}`));
  };
  return {
    call<T>(cmd: string, args: Record<string, unknown> = {}) {
      const id = next++;
      ws.send(JSON.stringify({ id, cmd, args }));
      return new Promise<T>((ok, fail) => waiting.set(id, { ok: ok as (v: unknown) => void, fail }));
    },
    close: () => ws.close(),
  };
}
