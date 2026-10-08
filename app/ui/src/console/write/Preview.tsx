import { useEffect, useState } from 'react';
import { openExternal } from '../../heat/external';
import { write, type Mode, type Stats } from './client';
import { errorText } from '../client';

export function Preview({ text, mode, onStats }: { text: string; mode: Mode; onStats?(stats: Stats): void }) {
  const [html, setHtml] = useState('');
  const [error, setError] = useState('');
  useEffect(() => {
    let live = true;
    const timer = setTimeout(() => { write.render(text, mode).then(r => { if (live) { setHtml(r.html); setError(''); onStats?.(r.stats); } }, e => { if (live) setError(errorText(e)); }); }, 120);
    return () => { live = false; clearTimeout(timer); };
  }, [text, mode, onStats]);
  return <div className={`write-preview ${mode === 'screenplay' ? 'write-screenplay' : ''}`} onClick={e => {
    const link = (e.target as HTMLElement).closest('a'); if (!link) return;
    e.preventDefault(); const href = link.getAttribute('href'); if (href && /^https?:\/\//.test(href)) openExternal(href);
  }}>{error ? <p role="alert">{error}</p> : <div dangerouslySetInnerHTML={{ __html: html }} />}</div>;
}
