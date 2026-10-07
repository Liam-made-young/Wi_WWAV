// ⌘⇧N, quick capture (docs/SPEC.md 2.7, 3.13): a 420 × 160 pt panel from
// any room. Type or paste a link and press Enter: it saves and stays open,
// and the footer reads "3 in inbox · captured ✓". A dropped file comes into
// the library and is captured by name. Esc closes it and keeps whatever is
// typed for next time. Captures wait in Heat's inbox until triaged.

import { forwardRef, useEffect, useImperativeHandle, useRef, useState } from 'react';
import { call } from '../bridge';
import { KINDS } from '../heat/kinds';
import type { Capture as CaptureRecord } from '../heat/model/records';
import { ulid } from './ids';
import { type UndoRoom, useCoreEvent } from './hooks';

interface Props {
  shown: boolean;
  /** Where the capture is journaled: ⌘Z there takes it back. */
  room: UndoRoom;
  onError(message: string): void;
}

export interface CaptureHandle {
  save(): void;
  drop(paths: string[]): void;
}

const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

export const Capture = forwardRef<CaptureHandle, Props>(function Capture({ shown, room, onError }, ref) {
  const [text, setText] = useState('');
  const [inbox, setInbox] = useState(0);
  const [captured, setCaptured] = useState(false);
  const field = useRef<HTMLTextAreaElement>(null);

  const count = () =>
    call<{ records: CaptureRecord[] }>('records.list', { kind: KINDS.capture }).then(
      (r) => setInbox(r.records.filter((c) => !c.triagedAt).length),
      () => {},
    );
  useEffect(() => void count(), []);
  useCoreEvent<{ kinds: string[] }>('records', ({ kinds }) => {
    if (kinds.includes(KINDS.capture)) void count();
  });
  useEffect(() => {
    if (shown) field.current?.focus();
  }, [shown]);

  const put = async (value: Record<string, unknown>) => {
    await call('records.mutate', {
      label: 'capture',
      room,
      ops: [{ op: 'put', kind: KINDS.capture, id: ulid(), value }],
    });
    setCaptured(true);
  };

  useImperativeHandle(ref, () => ({
    save() {
      const typed = text.trim();
      if (!typed) return;
      put({ text: typed }).then(
        () => setText(''),
        (e: Error) => onError(e.message),
      );
    },
    drop(paths) {
      call<{ clips: { id: string }[] }>('library.import', { paths, label: 'import' })
        .then(({ clips }) => put({ text: paths.map(fileName).join(', '), clips: clips.map((c) => c.id) }))
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
          setCaptured(false);
        }}
      />
      <p className="capture-foot" data-text="secondary">
        {inbox} in inbox{captured ? ' · captured ✓' : ''}
      </p>
    </div>
  );
});
