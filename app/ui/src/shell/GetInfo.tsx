// Get Info (⌘I, or Return in the library): every field of one clip, in
// Heat's Get Info drawer style. The verdict is the reader's sentence, word
// for word what wwav_pack.py info says, and each chunk is listed with what
// the reader made of it. Renaming changes the library's title only: the file
// keeps its ULID name and its bytes (2.5).

import { useEffect, useState } from 'react';
import { call, CoreError } from '../bridge';
import { useCoreEvent } from './hooks';
import { type Clip, COLOURS } from './LibraryDrawer';
import { tagName } from './library';
import { elapsed } from './strip';

interface Chunk {
  id: string;
  at: number;
  size: number;
  detail: string;
}

interface Props {
  id: string | null;
  shown: boolean;
}

const KIND_NAMES: Record<string, string> = {
  wwav: 'Song (.wwav)',
  swav: 'Film (.swav)',
  audio: 'Plain audio',
  video: 'Video',
  image: 'Image',
  text: 'Text',
};

export function GetInfo({ id, shown }: Props) {
  const [info, setInfo] = useState<{ clip: Clip; verdict: string; chunks: Chunk[] } | null>(null);
  const [title, setTitle] = useState('');
  const [tag, setTag] = useState('');
  const [refusal, setRefusal] = useState<string | null>(null);

  const load = () => {
    if (!id) return;
    call<{ clip: Clip; verdict: string; chunks: Chunk[] }>('library.get', { id }).then(
      (r) => {
        setInfo(r);
        setTitle(r.clip.title);
      },
      (e: Error) => setRefusal(e.message),
    );
  };
  useEffect(() => {
    setRefusal(null);
    load();
  }, [id]);
  useCoreEvent('library', load);

  const change = (cmd: string, args: Record<string, unknown>) =>
    call(cmd, args).then(
      () => setRefusal(null),
      (e: unknown) => setRefusal(e instanceof CoreError || e instanceof Error ? e.message : String(e)),
    );

  if (!info) {
    return (
      <div className="sheet info register-desk" role="dialog" aria-label="Get Info" hidden={!shown}>
        {refusal && <p role="alert">{refusal}</p>}
      </div>
    );
  }
  const { clip, verdict, chunks } = info;
  const facts: [string, string][] = [
    ['Kind', KIND_NAMES[clip.kind] ?? clip.kind],
    ['Length', elapsed(clip.duration)],
    ['Tempo', clip.bpm ? `${clip.bpm} BPM` : 'Not known'],
    ['Key', clip.key ?? 'Not known'],
    ['Size', `${clip.bytes.toLocaleString('en-US')} bytes`],
    ['File', `${clip.id}, in media/`],
    ['SHA-256', clip.sha256],
  ];

  return (
    <div className="sheet info register-desk" role="dialog" aria-label={`Get Info: ${clip.title}`} hidden={!shown}>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          if (title.trim() && title !== clip.title)
            void change('library.rename', { id: clip.id, title, label: 'rename clip' });
        }}
      >
        <label className="info-title">
          <span data-text="secondary">Title</span>
          <input
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            onBlur={(e) => e.currentTarget.form?.requestSubmit()}
          />
        </label>
      </form>
      <p className="info-verdict">{verdict}</p>
      {refusal && (
        <p className="info-refusal" role="alert">
          {refusal}
        </p>
      )}
      <dl className="info-facts">
        {facts.map(([k, v]) => (
          <div key={k}>
            <dt data-text="secondary">{k}</dt>
            <dd>{v}</dd>
          </div>
        ))}
      </dl>
      <h3 className="info-heading">Tags</h3>
      <div className="info-tags">
        {clip.tags.map((t) => (
          <span key={t} className="tag">
            {t}
            <button
              type="button"
              className="tag-remove"
              data-dense
              aria-label={`Remove the tag ${t}`}
              onClick={() => void change('library.tag', { ids: [clip.id], remove: [t], label: 'untag clip' })}
            >
              ×
            </button>
          </span>
        ))}
        <form
          onSubmit={(e) => {
            e.preventDefault();
            const name = tagName(tag);
            if (name)
              void change('library.tag', { ids: [clip.id], add: [name], label: 'tag clip' }).then(() => setTag(''));
          }}
        >
          <input
            aria-label="Add a tag"
            placeholder="Add a tag"
            value={tag}
            onChange={(e) => setTag(e.target.value.toLowerCase())}
          />
        </form>
      </div>
      <h3 className="info-heading">Colour label</h3>
      <div className="swatches">
        {COLOURS.map((c) => (
          <button
            key={c}
            type="button"
            className="swatch"
            data-colour={c}
            data-dense
            aria-label={`Label ${c}`}
            aria-pressed={clip.colour === c}
            onClick={() =>
              void change('library.colour', {
                ids: [clip.id],
                colour: clip.colour === c ? null : c,
                label: 'colour clip',
              })
            }
          />
        ))}
      </div>
      <button
        type="button"
        className="gel"
        onClick={() =>
          void change('library.pin', {
            id: clip.id,
            slot: clip.pinned === null ? 0 : null,
            label: clip.pinned === null ? 'pin clip' : 'unpin clip',
          })
        }
      >
        {clip.pinned === null ? 'Pin' : 'Unpin'}
      </button>
      {chunks.length > 0 && (
        <table className="info-chunks">
          <thead>
            <tr>
              <th data-text="secondary">Chunk</th>
              <th data-text="secondary">At</th>
              <th data-text="secondary">Bytes</th>
              <th data-text="secondary">Holds</th>
            </tr>
          </thead>
          <tbody>
            {chunks.map((c) => (
              <tr key={`${c.id}${c.at}`}>
                <td>{c.id}</td>
                <td>{c.at}</td>
                <td>{c.size}</td>
                <td>{c.detail}</td>
              </tr>
            ))}
          </tbody>
        </table>
      )}
    </div>
  );
}
