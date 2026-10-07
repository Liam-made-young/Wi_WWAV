// Export everything… (⌘⇧E, docs/SPEC.md 2.9): one action that writes every
// file, session, purchase and record to a folder or a zip. It is never
// behind a paywall and works while you are signed out.

import { useEffect, useState } from 'react';
import { call } from '../bridge';
import { useCoreEvent } from './hooks';

function suggested(library: string): string {
  const parent = library.replace(/[\\/][^\\/]*[\\/]?$/, '');
  return `${parent}/Wi_WWAV export ${new Date().toISOString().slice(0, 10)}`;
}

export function ExportSheet({ shown, library }: { shown: boolean; library: string }) {
  const [to, setTo] = useState('');
  const [zip, setZip] = useState(false);
  const [said, setSaid] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (library && !to) setTo(suggested(library));
  }, [library, to]);
  useCoreEvent<{ sentence: string }>('export', ({ sentence }) => {
    if (busy) setSaid(`${sentence}…`);
  });

  const go = () => {
    setBusy(true);
    setSaid('Exporting…');
    call<{ sentence: string }>('export.everything', { to: zip ? `${to}.zip` : to, zip })
      .then(
        (r) => setSaid(r.sentence),
        (e: Error) => setSaid(e.message),
      )
      .finally(() => setBusy(false));
  };

  return (
    <div className="sheet export register-desk" role="dialog" aria-label="Export everything" hidden={!shown}>
      <h2 className="sheet-title">Export everything</h2>
      <p>
        Every file byte for byte, your sessions, purchases, Heat, notes and galaxy, and a page that plays it all
        offline.
      </p>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (!busy && to.trim()) go();
        }}
      >
        <label className="field">
          <span data-text="secondary">To this folder</span>
          <input value={to} onChange={(e) => setTo(e.target.value)} />
        </label>
        <label className="check" data-dense>
          <input type="checkbox" checked={zip} onChange={(e) => setZip(e.target.checked)} />
          As one zip
        </label>
        <button type="submit" className="gel" disabled={busy}>
          Export
        </button>
      </form>
      {said && <p role="status">{said}</p>}
    </div>
  );
}
