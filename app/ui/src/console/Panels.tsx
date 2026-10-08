import { useEffect, useRef, useState, type ReactNode } from 'react';
import { ArrowLeft, Check, Eye, GitBranch, MessageSquare, X } from 'lucide-react';
import { api, errorText, type Document, type Proposal, type ToolId, type Version } from './client';
import { toolById } from './registry';

export function IconButton({ label, children, ...props }: { label: string; children: ReactNode } & React.ButtonHTMLAttributes<HTMLButtonElement>) {
  return <button type="button" className="console-icon" aria-label={label} title={label} {...props}>{children}</button>;
}
export function Modal({ title, onClose, wide, children }: { title: string; onClose(): void; wide?: boolean; children: ReactNode }) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => { const el = dialog.current; if (typeof el?.showModal === 'function') el.showModal(); return () => el?.close?.(); }, []);
  return <dialog ref={dialog} open={typeof HTMLDialogElement === 'undefined' || !HTMLDialogElement.prototype.showModal ? true : undefined} className={`console-dialog ${wide ? 'console-dialog-wide' : ''}`} aria-label={title} onCancel={e => { e.preventDefault(); onClose(); }}><header><h2>{title}</h2><IconButton label={`Close ${title}`} onClick={onClose}><X size={18} /></IconButton></header>{children}</dialog>;
}
export interface NameSheet { tool: ToolId; parent?: Document }
export function NameDialog({ sheet, busy, onClose, onSubmit }: { sheet: NameSheet; busy: boolean; onClose(): void; onSubmit(title: string): void }) {
  const [title, setTitle] = useState(sheet.parent ? `${sheet.parent.title} - variation` : '');
  return <Modal title={sheet.parent ? 'Save as variation' : `New ${toolById(sheet.tool).title} document`} onClose={onClose}><form className="console-name-form" onSubmit={e => { e.preventDefault(); if (title.trim()) onSubmit(title); }}><label>Title<input autoFocus required maxLength={200} value={title} onChange={e => setTitle(e.target.value)} /></label>{sheet.parent && <p>From {sheet.parent.title}, version {sheet.parent.versions}</p>}<button className="console-primary" disabled={busy || !title.trim()}>{sheet.parent ? 'Create variation' : 'Create document'}</button></form></Modal>;
}
export function HistoryPanel({ id, provenance, onRead }: { id: string; provenance: boolean; onRead(version: string): void }) {
  const [data, setData] = useState<{ versions: Version[]; parent: Document['parent']; note: string } | null>(null);
  const [error, setError] = useState('');
  useEffect(() => { let live = true; api.history(id).then(d => { if (live) setData(d); }, e => { if (live) setError(errorText(e)); }); return () => { live = false; }; }, [id]);
  return <div className="console-history">{error && <p role="alert">{error}</p>}{!data && !error && <p>Reading versions...</p>}{data && <>
    {data.parent && <p className="console-parent"><GitBranch size={16} /> From {data.parent.title}<small>{data.parent.documentId} / {data.parent.versionId}</small></p>}
    {provenance && <p className="console-record-note">{data.note}</p>}
    <ol>{[...data.versions].reverse().map((v, i) => <li key={v.id}><div><strong>v{data.versions.length - i} <span>{v.action}</span></strong><span className="console-history-actor">{v.actor === 'hand' ? 'By hand' : v.actor === 'claude' ? 'Claude' : 'Imported / origin unverified'}</span><time>{new Date(v.at).toLocaleString()}</time>{provenance && v.changes.map((c, n) => <p key={n}>{c}</p>)}{v.restoredFrom && <small>Restores {v.restoredFrom}</small>}</div><IconButton label={`Read version ${data.versions.length - i}`} onClick={() => onRead(v.id)}><Eye size={16} /></IconButton></li>)}</ol>
  </>}</div>;
}
export function ClaudePanel({ document: d, dirty, onApplied }: { document?: Document; dirty: boolean; onApplied(): Promise<void> }) {
  const [prompt, setPrompt] = useState('');
  const [answer, setAnswer] = useState('');
  const [proposal, setProposal] = useState<Proposal | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  if (!d) return <p>Open a document first.</p>;
  const ask = async () => { setBusy(true); setError(''); setProposal(null); try { const r = await api.ask(d.tool, d.id, prompt); setAnswer(r.answer); setProposal(r.proposal); } catch (e) { setError(errorText(e)); } finally { setBusy(false); } };
  const apply = async () => { if (!proposal) return; setBusy(true); try { await api.apply(proposal.id); setProposal(null); await onApplied(); setAnswer('Applied as a new version.'); } catch (e) { setError(errorText(e)); } finally { setBusy(false); } };
  return <div className="console-claude"><p className="console-claude-context">{toolById(d.tool).title} / {d.title} / v{d.versions}</p>{dirty && <p>Save your draft before asking Claude to edit it.</p>}
    <form onSubmit={e => { e.preventDefault(); void ask(); }}><textarea autoFocus aria-label="Ask Claude about this document" value={prompt} onChange={e => setPrompt(e.target.value)} placeholder="Ask about this work" maxLength={16000} /><button className="console-primary" disabled={busy || dirty || !prompt.trim()}><MessageSquare size={16} />{busy ? 'Working...' : 'Ask Claude'}</button></form>
    {error && <p role="alert">{error}</p>}{answer && <p className="console-claude-answer">{answer}</p>}
    {proposal && <section className="console-proposal" aria-label="Proposed edit"><h3>Proposed edit</h3>{proposal.args.title && <p>Title: {proposal.args.title}</p>}{proposal.args.text !== null && <pre>{proposal.args.text}</pre>}<div><button type="button" disabled={busy || dirty} className="console-primary" onClick={() => void apply()}><Check size={16} />Apply as new version</button><button type="button" className="console-plain" onClick={() => setProposal(null)}><ArrowLeft size={15} />Discard proposal</button></div></section>}
  </div>;
}
