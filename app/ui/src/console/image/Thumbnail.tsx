import { useEffect, useRef, useState } from 'react';
import { Image } from 'lucide-react';
import { call } from '../../bridge';
import type { Document } from '../client';
import type { Preview } from './client';
const cache = new Map<string, string>();
export function ImageThumbnail({ document: d }: { document: Document }) {
  const host = useRef<HTMLSpanElement>(null); const key = `${d.id}/${d.head}`; const [src, setSrc] = useState(cache.get(key));
  useEffect(() => {
    let live = true; setSrc(cache.get(key)); if (cache.has(key)) return;
    const observer = new IntersectionObserver(entries => {
      if (!entries.some(e => e.isIntersecting)) return; observer.disconnect();
      call<Preview>('console.image.preview', { id: d.id, version: d.head, maxEdge: 240 }).then(p => { const url = `data:${p.mime};base64,${p.base64}`; if (cache.size >= 128) cache.delete(cache.keys().next().value!); cache.set(key, url); if (live) setSrc(url); }).catch(() => {});
    }); observer.observe(host.current!);
    return () => { live = false; observer.disconnect(); };
  }, [key, d.id, d.head]);
  return <span className="image-library-thumbnail" ref={host}>{src ? <img src={src} alt="" loading="lazy" /> : <Image size={20} />}</span>;
}
