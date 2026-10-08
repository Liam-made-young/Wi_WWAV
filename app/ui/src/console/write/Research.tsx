import { useEffect, useState } from 'react';
import { Link2, Plus, Search, Trash2, X } from 'lucide-react';
import { openExternal } from '../../heat/external';
import { errorText } from '../client';
import { IconButton, Modal } from '../Panels';
import { Preview } from './Preview';
import { write, type Edit, type Manuscript } from './client';

export function ResearchPanel({ manuscript, busy, onNotes, onEdit, onClose, onError }: {
  manuscript: Manuscript; busy: boolean; onNotes(text: string): void; onEdit(action: Edit): Promise<boolean>; onClose(): void; onError(message: string): void;
}) {
  const [query, setQuery] = useState('');
  const [hits, setHits] = useState<{ id: string; title: string; snippet: string }[]>([]);
  const [source, setSource] = useState(false);
  const [title, setTitle] = useState('');
  const [url, setUrl] = useState('');
  const [excerpt, setExcerpt] = useState('');
  const [note, setNote] = useState<{ id: string; title: string; markdown: string } | null>(null);
  useEffect(() => {
    let live = true;
    if (!query.trim()) { setHits([]); return; }
    const timer = setTimeout(() => { write.searchNotes(query).then(r => { if (live) setHits(r.hits); }, e => { if (live) onError(errorText(e)); }); }, 180);
    return () => { live = false; clearTimeout(timer); };
  }, [query, onError]);
  return <aside className="write-research" aria-label="Document research">
    <header><h2>Notes &amp; Research</h2><IconButton label="Close research" onClick={onClose}><X size={15} /></IconButton></header>
    <label className="write-notes-label">Notes<textarea aria-label="Document notes" disabled={busy} value={manuscript.notes} onChange={e => onNotes(e.target.value)} /></label>
    <div className="write-source-heading"><h3>Sources</h3><IconButton label="Add research source" onClick={() => setSource(true)}><Plus size={15} /></IconButton></div>
    <ul className="write-sources">{manuscript.research.map(r => <li key={r.id}>
      <div><button type="button" className="write-source-title" onClick={() => {
        if (r.noteId) { setNote({ id: r.noteId, title: r.title, markdown: r.excerpt }); void write.readNote(r.noteId).then(n => setNote({ id: r.noteId!, ...n }), e => onError(errorText(e))); }
        else if (r.url) openExternal(r.url);
      }}><Link2 size={13} />{r.title}</button><IconButton label={`Remove source ${r.title}`} disabled={busy} onClick={() => void onEdit({ type: 'researchRemove', source: r.id })}><Trash2 size={13} /></IconButton></div>
      {r.url && <small>{r.url}</small>}{r.excerpt && <p>{r.excerpt.slice(0, 180)}</p>}
    </li>)}</ul>
    <label className="write-note-search"><Search size={14} /><input aria-label="Search Learn notes" placeholder="Search Learn notes" value={query} onChange={e => setQuery(e.target.value)} /></label>
    <ul className="write-note-hits">{hits.map(h => <li key={h.id}><button type="button" disabled={busy} onClick={() => { void onEdit({ type: 'linkNote', note: h.id }); setQuery(''); }}><Link2 size={13} /><span>{h.title}</span></button></li>)}</ul>
    {source && <Modal title="Research source" onClose={() => setSource(false)}><form className="write-source-form" onSubmit={e => { e.preventDefault(); void onEdit({ type: 'researchAdd', title, url: url || null, excerpt }).then(ok => { if (ok) { setSource(false); setTitle(''); setUrl(''); setExcerpt(''); } }); }}>
      <label>Title<input autoFocus required maxLength={200} value={title} onChange={e => setTitle(e.target.value)} /></label>
      <label>Address<input type="url" value={url} onChange={e => setUrl(e.target.value)} /></label>
      <label>Excerpt<textarea maxLength={16000} value={excerpt} onChange={e => setExcerpt(e.target.value)} /></label>
      <button className="console-primary" disabled={busy || !title.trim()}><Link2 size={15} />Add source</button>
    </form></Modal>}
    {note && <Modal title={note.title} onClose={() => setNote(null)} wide><div className="write-linked-note"><div><span>Learn note / Read only</span></div><Preview text={note.markdown} mode="prose" /></div></Modal>}
  </aside>;
}
