// "Send to Wi-WWAV": the one screen that says how a notebook page gets
// here (docs/NOTES.md, "The capture inbox"). From a phone, the Shortcut;
// on this Mac, a drop, a paste or a folder; then what Learn does with the
// page, the switch for what Claude may read, and whether the reader on this
// Mac is there.

import { useEffect, useRef } from 'react';
import { useHeat } from '../store';

const READER = {
  ready: 'The reader on this Mac is ready.',
  building: 'The reader on this Mac is being built. Pages wait until it is.',
  missing: 'The reader on this Mac is not available: install Xcode’s command-line tools.',
} as const;

export function GuideSheet({ onClose }: { onClose(): void }) {
  const { snap, client, act, refetch } = useHeat();
  const done = useRef<HTMLButtonElement>(null);
  useEffect(() => done.current?.focus(), []);
  const capture = snap?.notes?.capture;
  const folders = capture?.folders ?? [];

  const setClaude = async (on: boolean) => {
    if (await act(client.pages.setClaude(on))) await refetch();
  };

  return (
    <div className="sheet heat-sheet notes-guide" role="dialog" aria-label="Send to Wi-WWAV">
      <h2 className="sheet-title">Send to Wi-WWAV</h2>

      <h3 className="notes-guide-step">On your iPhone or iPad</h3>
      <ol className="notes-guide-list">
        <li>
          Get the Shortcut. The file is <code>tools/shortcut/Send to Wi-WWAV.shortcut</code>: AirDrop it to your phone or
          open it from iCloud Drive, then Add Shortcut.
        </li>
        <li>Take a photo of a page.</li>
        <li>Share, then Send to Wi-WWAV.</li>
      </ol>

      <h3 className="notes-guide-step">On this Mac</h3>
      <p>Drop a photo or a PDF on the Notes tab, or paste an image there. Or put files in:</p>
      <ul className="notes-guide-folders">
        {folders.map((f) => (
          <li key={f}>
            <code>{f}</code>
          </li>
        ))}
        {folders.length === 0 && <li data-text="secondary">No folder is set up yet.</li>}
      </ul>
      <p className="why" data-text="secondary" role="status">
        {capture ? (capture.watching ? 'These folders are being watched.' : 'These folders aren’t being watched right now.') : ''}
      </p>

      <h3 className="notes-guide-step">What happens</h3>
      <p>
        The page is read on this Mac. It is filed to the class it was taken in; with no class, Claude suggests where it
        goes; otherwise it waits in the Notes inbox, to be filed with one click.
      </p>
      <label className="check heat-switch" data-dense>
        <input
          type="checkbox"
          role="switch"
          checked={capture?.claude ?? false}
          disabled={!capture}
          onChange={(e) => void setClaude(e.target.checked)}
        />
        Let Claude read what this Mac can’t, and suggest where a page goes
      </label>
      <p className="why" data-text="secondary" role="status">
        {capture ? READER[capture.reader] : ''}
      </p>

      <div className="note-actions">
        <button ref={done} type="button" className="gel" onClick={onClose}>
          Done
        </button>
        <button type="button" className="gel plain" onClick={() => void act(client.notes.reveal())}>
          Show the notes folder
        </button>
      </div>
    </div>
  );
}
