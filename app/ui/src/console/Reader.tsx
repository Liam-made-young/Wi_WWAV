import { useEffect, useState } from 'react';
import { File, LoaderCircle } from 'lucide-react';
import { api, errorText, type ReadResult } from './client';

export function Media({ data }: { data: ReadResult }) {
  const [url, setUrl] = useState<string | null>(null);
  useEffect(() => {
    if (!data.base64) { setUrl(null); return; }
    const bytes = Uint8Array.from(atob(data.base64), c => c.charCodeAt(0));
    const objectUrl = URL.createObjectURL(new Blob([bytes], { type: data.version.asset.mime }));
    setUrl(objectUrl);
    return () => URL.revokeObjectURL(objectUrl);
  }, [data]);
  const mime = data.version.asset.mime;
  if (data.text !== null) return <pre className="console-text-preview">{data.text || 'Empty document'}</pre>;
  if (!url) return <LoaderCircle aria-label="Loading preview" />;
  if (mime.startsWith('image/')) return <img src={url} alt={data.version.title} className="console-media-image" />;
  if (mime.startsWith('audio/')) return <div className="console-audio"><File size={48} aria-hidden /><p>{data.version.asset.name}</p><audio controls preload="metadata" src={url} /></div>;
  if (mime.startsWith('video/')) return <video controls preload="metadata" src={url} className="console-media-video" />;
  return <p>No preview for this file.</p>;
}

export function Reader({ id, version }: { id: string; version?: string }) {
  const [data, setData] = useState<ReadResult | null>(null);
  const [error, setError] = useState('');
  useEffect(() => {
    let live = true;
    setData(null); setError('');
    api.read(id, version).then(d => { if (live) setData(d); }, e => { if (live) setError(errorText(e)); });
    return () => { live = false; };
  }, [id, version]);
  return <div className="console-reader">
    {error && <p role="alert">{error}</p>}
    {!error && !data && <p role="status">Opening file...</p>}
    {data && <><div className="console-reader-meta"><span>{data.version.title}</span><span>Read only</span></div><Media data={data} /><p className="console-file-path">{data.path}</p></>}
  </div>;
}
