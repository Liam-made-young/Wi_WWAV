// A stand-in for the core, for the tests of the prompt box, the Database tab
// and the Wiki tab: it answers the commands a test names, remembers what it
// was asked, and can send an event. Learn's own snapshot still comes from
// the fake core (heat/testkit.tsx); this answers the bridge's other calls.

import { CoreError, installStandIn } from '../bridge';

type Answer = (args: Record<string, unknown>) => unknown;

export interface StandIn {
  calls: { cmd: string; args: Record<string, unknown> }[];
  /** The calls made to one command, by their arguments. */
  asked(cmd: string): Record<string, unknown>[];
  emit(event: string, payload: unknown): void;
  /** Adds or replaces answers. */
  answer(more: Record<string, Answer>): void;
}

// The bridge starts listening once, on whichever transport is standing in
// then; so there is one transport for the whole test run, and each test
// gives it the answers it wants.
let table: Record<string, Answer> = {};
let calls: StandIn['calls'] = [];
let deliver: { event(name: string, payload: unknown): void } | null = null;
let installed = false;

export function standIn(answers: Record<string, Answer> = {}): StandIn {
  table = { ...answers };
  calls = [];
  const mine = calls;
  if (!installed) {
    installed = true;
    installStandIn({
      async call(cmd, args) {
        calls.push({ cmd, args });
        const answer = table[cmd];
        if (!answer) throw new CoreError('unknown_command', `There is no command called '${cmd}'.`);
        return answer(args);
      },
      listen(d) {
        deliver = d;
      },
    });
  }
  return {
    calls: mine,
    asked: (cmd) => mine.filter((c) => c.cmd === cmd).map((c) => c.args),
    emit: (event, payload) => deliver?.event(event, payload),
    answer: (more) => Object.assign(table, more),
  };
}
