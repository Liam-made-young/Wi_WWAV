// ⌘⇧N, quick capture (docs/SPEC.md 2.7, 3.13): a 420 × 160 pt panel from
// any room. Type or paste a link and press Enter: it saves and stays open,
// and the footer reads "3 in inbox · captured ✓". A dropped file comes into
// the library and is captured by name. Esc closes it and keeps whatever is
// typed for next time. Captures wait in Heat's inbox until triaged.

import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { call } from '../bridge';
import { useHeat } from '../heat/store';

interface Props {
  shown: boolean;
  onError(message: string): void;
}

export interface CaptureHandle {
  save(): void;
  drop(paths: string[]): void;
}

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

export const Capture = forwardRef<CaptureHandle, Props>(function Capture({ shown, onError }, ref) {
  const { client, snap } = useHeat();
  const [text, setText] = useState('');
  // The core's own line after a capture ("4 in inbox · captured ✓"), until the next keystroke.
  const [said, setSaid] = useState<string | null>(null);
  const field = useRef<HTMLTextAreaElement>(null);

  const inbox = snap?.derived.lists.inbox.length ?? 0;
  useEffect(() => {
    if (shown) field.current?.focus();
  }, [shown]);

  const put = async (value: string) => {
    const r = await client.capture(value);
    setSaid(r.inbox);
  };

  useImperativeHandle(ref, () => ({
    save() {
      const typed = text.trim();
      if (!typed) return;
      put(typed).then(
        () => setText(''),
        (e: Error) => onError(e.message),
      );
    },
    drop(paths) {
      call('library.import', { paths, label: 'import' })
        .then(() => put(paths.map(fileName).join(', ')))
        .catch((e: Error) => onError(e.message));
    },
  }));

  return (
    <div className="sheet capture" role="dialog" aria-label="Quick capture" hidden={!shown} data-drop="capture">
      <p className="capture-head" data-text="secondary">
        Quick capture
      </p>
      <textarea
        ref={field}
        className="capture-field"
        aria-label="Capture"
        placeholder="A thought, a task or a link. Drop a file here."
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          setSaid(null);
        }}
      />
      <p className="capture-foot" data-text="secondary">
        {said ?? `${inbox} in inbox`}
      </p>
    </div>
  );
});
