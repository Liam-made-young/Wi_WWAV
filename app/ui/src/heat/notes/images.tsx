// An image in a note (docs/NOTES.md). A web address or a data URL is shown
// as it is; any other path is a file in the notes folder, which a view can't
// open itself, so the core is asked for it once and the answer is kept by
// its path. A quiet line stands in while it loads and if it can't be had. A
// click opens the image larger, over the tab; Esc closes it.

import { useEffect, useRef, useState } from 'react';
import type { HeatClient } from '../client';
import { useHeat } from '../store';

export interface Shown {
  src: string;
  alt: string;
}

const held = new Map<string, string>();
const asking = new Map<string, Promise<string>>();
const direct = (src: string) => /^(https?:|data:)/i.test(src);

function load(client: HeatClient, path: string): Promise<string> {
  let going = asking.get(path);
  if (!going) {
    going = client.notes.attachment(path).then((r) => {
      held.set(path, r.dataUrl);
      return r.dataUrl;
    });
    asking.set(path, going);
    // One that failed is asked for again the next time it is drawn.
    going.catch(() => asking.delete(path));
  }
  return going;
}

/** Forgets every image asked for, so a test starts with none. */
export function forgetImages() {
  held.clear();
  asking.clear();
}

export function NoteImage({ src, alt, onOpen }: { src: string; alt: string; onOpen(image: Shown): void }) {
  const { client } = useHeat();
  const [url, setUrl] = useState<string | null>(() => (direct(src) ? src : (held.get(src) ?? null)));
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    if (direct(src)) return setUrl(src);
    const kept = held.get(src);
    if (kept) return setUrl(kept);
    let live = true;
    setUrl(null);
    setFailed(false);
    load(client, src).then(
      (dataUrl) => live && setUrl(dataUrl),
      () => live && setFailed(true),
    );
    return () => {
      live = false;
    };
  }, [client, src]);

  if (!url || failed) {
    return (
      <span className="notes-img-wait" role="img" aria-label={alt || 'Image'} aria-busy={!failed} data-failed={failed || undefined}>
        {failed ? 'This image couldn’t be loaded.' : 'Loading the image…'}
      </span>
    );
  }
  return (
    <button
      type="button"
      className="notes-img-open"
      data-dense
      aria-label={alt ? `Open larger: ${alt}` : 'Open the image larger'}
      onClick={() => onOpen({ src: url, alt })}
    >
      <img className="notes-img" src={url} alt={alt} onError={() => setFailed(true)} />
    </button>
  );
}

/** The image larger, over the tab. A click anywhere closes it, and so does Esc (the tab's). */
export function Lightbox({ image, onClose }: { image: Shown; onClose(): void }) {
  const close = useRef<HTMLButtonElement>(null);
  useEffect(() => close.current?.focus(), []);
  return (
    <div className="notes-lightbox" role="dialog" aria-label={image.alt || 'Image'} onClick={onClose}>
      <img src={image.src} alt={image.alt} />
      <button ref={close} type="button" className="gel" onClick={onClose}>
        Close
      </button>
    </div>
  );
}
