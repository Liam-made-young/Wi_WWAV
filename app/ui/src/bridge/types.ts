// The core's contract as the UI sees it (docs/COMMANDS.md): a command answers
// its result or a CoreError; events are {event, payload}; meters are raw
// bytes, the newest meter entry of docs/ENGINE.md 4.4, once a frame.

/** A refusal from the core: `code` to branch on, `message` to show as is. */
export class CoreError extends Error {
  readonly code: string;

  constructor(code: string, message: string) {
    super(message);
    this.name = 'CoreError';
    this.code = code;
  }
}

export type Args = Record<string, unknown>;
export type EventHandler = (payload: unknown) => void;
export type MetersHandler = (frame: ArrayBuffer) => void;

/** One way to reach the core: Tauri's IPC in the app, the dev bridge elsewhere. */
export interface Transport {
  call(cmd: string, args: Args): Promise<unknown>;
  /** Starts delivering events and meters to `deliver`; called once. */
  listen(deliver: { event(name: string, payload: unknown): void; meters(frame: ArrayBuffer): void }): void;
}

/** An error as it crossed the wire, made a CoreError. */
export function coreError(e: unknown): CoreError {
  if (e instanceof CoreError) return e;
  if (e && typeof e === 'object' && 'code' in e && 'message' in e) {
    return new CoreError(String((e as { code: unknown }).code), String((e as { message: unknown }).message));
  }
  return new CoreError('bridge', e instanceof Error ? e.message : String(e));
}
