// The Public switch (docs/SPEC.md 3.15): off by default, one item at a time,
// with a sentence that says plainly what it does. A grade's says that your
// school keeps grades as education records. Turning it on or off is its own
// undo step ("Undo make public"). The core answers a grade's switch with the
// sentence too, which is said once more in the status bar.

import { useId } from 'react';
import type { Id, Kind } from '../client';
import { copy } from '../fmt';
import { useHeat } from '../store';

type Sentenced = keyof typeof copy.publicSwitch & Kind;

export function PublicSwitch({
  kind,
  id,
  on,
  name,
  disabled,
}: {
  kind: Kind;
  id: Id;
  on: boolean;
  /** What the switch is for, so a screen reader hears "Public: Kanji quiz 3". */
  name: string;
  /** Why it can't be turned on now, in one sentence. */
  disabled?: string | null;
}) {
  const { client, act, say } = useHeat();
  const hint = useId();
  const words = copy.publicSwitch[kind as Sentenced] as { off: string; on: string } | undefined;

  const turn = async (next: boolean) => {
    const r = await act(client.setPublic(kind, id, next));
    if (r?.sentence && next) say(r.sentence);
  };

  return (
    <div className="heat-switch-block">
      <label className="check heat-switch" data-dense>
        <input
          type="checkbox"
          role="switch"
          checked={on}
          disabled={!!disabled}
          aria-label={`${copy.publicSwitch.label}: ${name}`}
          aria-describedby={hint}
          onChange={(e) => void turn(e.target.checked)}
        />
        {copy.publicSwitch.label}
      </label>
      <p id={hint} className="heat-switch-hint" data-text="secondary">
        {disabled ?? (on ? words?.on : words?.off)}
      </p>
    </div>
  );
}
