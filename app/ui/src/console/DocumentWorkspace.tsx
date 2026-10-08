import { useEffect, useRef, useState } from 'react';
import { ArrowUpRight, Eye, GitBranch, History, MessageSquare, Redo2, Save, Undo2 } from 'lucide-react';
import { api, errorText, type Document, type ReadResult } from './client';
import { toolById } from './registry';
import { Media } from './Reader';
import { IconButton } from './Panels';

export function DocumentWorkspace({ document: d, visible, busy, onDirty, onSave, onError, onPanel, onVariation, onUndo, onPost }: {
  document: Document; visible: boolean; busy: boolean; onDirty(v: boolean): void;
  onSave(base: string, title: string, text?: string): Promise<void>; onError(s: string): void;
  onPanel(kind: 'history' | 'provenance' | 'reader' | 'claude'): void; onVariation(): void; onUndo(redo: boolean): void; onPost(): void;
}) {
  const [data, setData] = useState<ReadResult | null>(null);
  const [title, setTitle] = useState(d.title);
  const [text, setText] = useState<string | undefined>();
  const [base, setBase] = useState(d.head);
  const [saving, setSaving] = useState(false);
  const dirty = !!data && (title !== data.version.title || (text !== undefined && text !== data.text));
  const dirtyRef = useRef(false); dirtyRef.current = dirty;
  const callbacks = useRef({ onError, onDirty }); callbacks.current = { onError, onDirty };
  useEffect(() => {
    if (dirtyRef.current) return;
    let live = true;
    api.read(d.id).then(r => { if (live) { setData(r); setTitle(r.version.title); setText(r.version.asset.mime.startsWith('text/') && d.tool === 'write' ? r.text ?? '' : undefined); setBase(r.version.id); } }, e => { if (live) callbacks.current.onError(errorText(e)); });
    return () => { live = false; };
  }, [d.id, d.head, d.tool]);
  useEffect(() => { callbacks.current.onDirty(dirty); }, [dirty]);
  useEffect(() => {
    if (!dirty) return;
    const leave = (e: BeforeUnloadEvent) => { e.preventDefault(); };
    window.addEventListener('beforeunload', leave); return () => window.removeEventListener('beforeunload', leave);
  }, [dirty]);
  const save = async () => {
    if (!data || saving) return;
    setSaving(true);
    try { await onSave(base, title, text); dirtyRef.current = false; const r = await api.read(d.id); setData(r); setBase(r.version.id); setTitle(r.version.title); setText(text === undefined ? undefined : r.text ?? ''); }
    catch (e) { onError(errorText(e)); }
    finally { setSaving(false); }
  };
  const pending = busy || saving || !data;
  const { Icon } = toolById(d.tool);
  return <div className="console-document" hidden={!visible} inert={!visible} data-active={visible}>
    <div className="console-document-bar">
      <input aria-label="Document title" value={title} maxLength={200} onChange={e => setTitle(e.target.value)} />
      <span className="console-version-label">v{d.versions}{dirty ? ' / Unsaved' : ''}</span>
      <div className="console-document-actions">
        <IconButton label="Save version" data-save disabled={pending} onClick={() => void save()}><Save size={17} /></IconButton>
        <IconButton label="Undo document edit" disabled={pending || dirty || !d.canUndo} onClick={() => onUndo(false)}><Undo2 size={17} /></IconButton>
        <IconButton label="Redo document edit" disabled={pending || dirty || !d.canRedo} onClick={() => onUndo(true)}><Redo2 size={17} /></IconButton>
        <IconButton label="Save as variation" disabled={pending || dirty} onClick={onVariation}><GitBranch size={17} /></IconButton>
        <IconButton label="Version history" onClick={() => onPanel('history')}><History size={17} /></IconButton>
        <IconButton label="Read saved version" onClick={() => onPanel('reader')}><Eye size={17} /></IconButton>
        <IconButton label="Ask Claude" onClick={() => onPanel('claude')}><MessageSquare size={17} /></IconButton>
        <IconButton label="Prepare Space post" disabled={pending || dirty} onClick={onPost}><ArrowUpRight size={18} /></IconButton>
      </div>
    </div>
    {base !== d.head && dirty && <p className="console-conflict" role="alert">A newer version exists. Your unsaved draft is preserved here.</p>}
    <div className="console-canvas">
      {!data ? <p>Opening document...</p> : text !== undefined ? <textarea className="console-draft" aria-label="Document text" placeholder="" spellCheck value={text} onChange={e => setText(e.target.value)} onSelect={e => { const el = e.currentTarget; void api.selection(d.tool, { start: el.selectionStart, end: el.selectionEnd, text: el.value.slice(el.selectionStart, el.selectionEnd).slice(0, 4000) }).catch(() => {}); }} /> : d.asset.mime === 'application/json' ? <div className="console-empty-canvas"><Icon size={40} strokeWidth={1} aria-hidden /><h2>{d.title}</h2><span>{toolById(d.tool).title}</span></div> : visible && <Media data={data} />}
    </div>
    <div className="console-document-foot"><button type="button" className="console-provenance" onClick={() => onPanel('provenance')}>{d.marker}</button>{d.parent && <span><GitBranch size={12} /> Variation of {d.parent.title}</span>}<span>{data?.version.asset.name}</span></div>
  </div>;
}
