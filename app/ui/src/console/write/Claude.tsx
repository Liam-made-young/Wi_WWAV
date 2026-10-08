import { useEffect, useState } from 'react';
import { ArrowLeft, Check, MessageSquare } from 'lucide-react';
import { api, errorText, type Document } from '../client';
import { write, type Intent, type Selection, type WriteData, type WriteProposal } from './client';

const INTENTS: { id: Intent; title: string }[] = [
  { id: 'rewrite', title: 'Rewrite selection' }, { id: 'tighten', title: 'Tighten selection' },
  { id: 'rhymes', title: 'Suggest rhymes' }, { id: 'alternatives', title: 'Suggest alternatives' },
  { id: 'summarize', title: 'Summarize piece' }, { id: 'reorder', title: 'Reorder sections' },
  { id: 'continue', title: 'Continue in my style' },
];
export function WriteClaude({ document: d, dirty, onApplied }: { document: Document; dirty: boolean; onApplied(): Promise<void> }) {
  const [data, setData] = useState<WriteData | null>(null);
  const [selection, setSelection] = useState<Selection | null>(null);
  const [intent, setIntent] = useState<Intent>('rewrite');
  const [prompt, setPrompt] = useState('');
  const [answer, setAnswer] = useState('');
  const [proposal, setProposal] = useState<WriteProposal | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState('');
  useEffect(() => {
    let live = true;
    Promise.all([write.read(d.id), api.workspace()]).then(([r, w]) => { if (!live) return; setData(r); const s = w.workspace.tools.write.selection as Selection | null; setSelection(s?.section ? s : { section: r.view.section || r.manuscript.sections[0].id, from: 0, to: 0 }); }, e => { if (live) setError(errorText(e)); });
    return () => { live = false; };
  }, [d.id, d.head]);
  const ask = async () => {
    if (!data || !selection || dirty || busy) return;
    setBusy(true); setError(''); setProposal(null);
    try { const r = await write.assist(d.id, data.base, selection.section, selection, intent, prompt); setAnswer(r.answer); setProposal(r.proposal); }
    catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  const apply = async () => {
    if (!proposal || dirty) return;
    setBusy(true);
    try { await api.apply(proposal.id); setProposal(null); await onApplied(); setAnswer('Applied as a new version.'); }
    catch (e) { setError(errorText(e)); } finally { setBusy(false); }
  };
  const section = data?.manuscript.sections.find(s => s.id === selection?.section);
  return <div className="console-claude write-claude">
    <p className="console-claude-context">Write / {d.title} / {section?.title ?? 'Loading section...'}</p>
    {selection && selection.from !== selection.to && <blockquote className="write-selected-excerpt">{section?.text.slice(selection.from, selection.to).slice(0, 600)}</blockquote>}
    {dirty && <p>Save your draft before asking Claude to edit it.</p>}
    <form onSubmit={e => { e.preventDefault(); void ask(); }}>
      <label>Action<select aria-label="Write Claude action" value={intent} onChange={e => { setIntent(e.target.value as Intent); setProposal(null); }}>{INTENTS.map(i => <option key={i.id} value={i.id}>{i.title}</option>)}</select></label>
      <textarea autoFocus aria-label="Ask Claude about this document" value={prompt} onChange={e => setPrompt(e.target.value)} maxLength={8000} placeholder="What would you like to change?" />
      <button className="console-primary" disabled={busy || dirty || !data}><MessageSquare size={16} />{busy ? 'Working...' : 'Ask Claude'}</button>
    </form>
    {error && <p role="alert">{error}</p>}{answer && <p className="console-claude-answer">{answer}</p>}
    {proposal && <section className="console-proposal" aria-label="Proposed edit"><h3>Proposed edit</h3>
      {proposal.args.action.type === 'replace' ? <><div className="write-proposal-comparison"><div><h4>Before</h4><pre>{proposal.before || '(Insertion)'}</pre></div><div><h4>After</h4><pre>{proposal.args.action.text}</pre></div></div></> : <ol>{proposal.args.action.order.map(id => <li key={id}>{data?.manuscript.sections.find(s => s.id === id)?.title ?? id}</li>)}</ol>}
      <div><button type="button" className="console-primary" disabled={busy || dirty} onClick={() => void apply()}><Check size={16} />Apply as new version</button><button type="button" className="console-plain" disabled={busy} onClick={() => setProposal(null)}><ArrowLeft size={15} />Discard proposal</button></div>
    </section>}
  </div>;
}
