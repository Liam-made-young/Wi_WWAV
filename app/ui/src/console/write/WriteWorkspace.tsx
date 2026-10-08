import { useCallback, useEffect, useRef, useState } from 'react';
import { AlignVerticalJustifyCenter, ArrowDown, ArrowUp, ArrowUpRight, Bold, ChevronLeft, ChevronRight, Download, Eye, Focus, GitBranch, GripVertical, History, Italic, ListTree, MessageSquare, NotebookPen, PanelLeft, Pencil, Plus, Redo2, Save, Trash2, Undo2 } from 'lucide-react';
import { api, errorText } from '../client';
import { IconButton, Modal } from '../Panels';
import type { WorkspaceProps } from '../DocumentWorkspace';
import { Editor } from './Editor';
import { Preview } from './Preview';
import { ResearchPanel } from './Research';
import { download, write, type Edit, type Manuscript, type Mode, type Section, type Selection, type Stats, type View, type WriteData } from './client';
import './write.css';

function depths(sections: Section[]) {
  const depth = new Map<string, number>();
  for (const s of sections) depth.set(s.id, s.parent ? (depth.get(s.parent) ?? 0) + 1 : 0);
  return depth;
}
function descendantIds(sections: Section[], id: string) {
  const ids = new Set([id]); for (const s of sections) if (s.parent && ids.has(s.parent)) ids.add(s.id); return ids;
}
export function WriteWorkspace({ document: d, visible, busy, onDirty, onError, onPanel, onVariation, onUndo, onPost, onRefresh }: WorkspaceProps) {
  const [data, setData] = useState<WriteData | null>(null);
  const [manuscript, setManuscript] = useState<Manuscript | null>(null);
  const [title, setTitle] = useState(d.title);
  const [view, setView] = useState<View>({ binder: window.innerWidth > 760 });
  const [selected, setSelected] = useState('');
  const [dirty, setDirty] = useState(false);
  const [working, setWorking] = useState(false);
  const [stats, setStats] = useState<Record<string, Stats>>({});
  const [selection, setSelection] = useState<Selection | null>(null);
  const [adding, setAdding] = useState<{ parent: string | null } | null>(null);
  const [newTitle, setNewTitle] = useState('');
  const [newKind, setNewKind] = useState('section');
  const [deleting, setDeleting] = useState<Section | null>(null);
  const [exporting, setExporting] = useState(false);
  const [format, setFormat] = useState('markdown');
  const [exportPath, setExportPath] = useState('');
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const [jump, setJump] = useState<number | undefined>();
  const changed = useRef(false); changed.current = dirty;
  const operating = useRef(false);
  const current = useRef({ data, manuscript, title, selected }); current.current = { data, manuscript, title, selected };
  const callbacks = useRef({ onError, onDirty, onRefresh }); callbacks.current = { onError, onDirty, onRefresh };
  const loadSerial = useRef(0);
  const load = useCallback(async (prefer?: string) => {
    const n = ++loadSerial.current;
    const r = await write.read(d.id); if (n !== loadSerial.current) return;
    setData(r); setManuscript(r.manuscript); setTitle(r.document.title); setDirty(false); changed.current = false;
    setStats(Object.fromEntries(r.stats.sections.map(s => [s.id, s.stats])));
    const id = prefer || current.current.selected || r.view.section;
    setSelected(r.manuscript.sections.some(s => s.id === id) ? id! : r.manuscript.sections[0].id);
    setView(v => ({ ...v, ...r.view }));
    setSelection(null);
  }, [d.id]);
  useEffect(() => {
    if (changed.current || operating.current) return;
    void load().catch(e => callbacks.current.onError(errorText(e)));
  }, [d.head, load]);
  useEffect(() => { callbacks.current.onDirty(dirty); }, [dirty]);
  useEffect(() => { if (!visible) { setAdding(null); setDeleting(null); setExporting(false); } }, [visible]);
  useEffect(() => {
    if (!dirty) return;
    const leave = (e: BeforeUnloadEvent) => { e.preventDefault(); };
    window.addEventListener('beforeunload', leave); return () => window.removeEventListener('beforeunload', leave);
  }, [dirty]);
  const editDraft = (change: (m: Manuscript) => Manuscript) => { setManuscript(m => m ? change(m) : m); changed.current = true; setDirty(true); };
  const editSection = (patch: Partial<Section>) => editDraft(m => ({ ...m, sections: m.sections.map(s => s.id === selected ? { ...s, ...patch } : s) }));
  const saveDraft = async () => {
    const now = current.current;
    if (!now.data || !now.manuscript) throw new Error('The manuscript is still opening.');
    if (!changed.current) return now.data.document;
    const result = await write.save(d.id, now.data.base, now.title, now.manuscript);
    changed.current = false; setDirty(false); return result.document;
  };
  const run = async (action: () => Promise<void>) => {
    if (operating.current) return false;
    operating.current = true; setWorking(true);
    try { await action(); await callbacks.current.onRefresh(); return true; }
    catch (e) { callbacks.current.onError(errorText(e)); return false; }
    finally { operating.current = false; setWorking(false); }
  };
  const save = () => void run(async () => {
    const now = current.current;
    if (!now.data || !now.manuscript) return;
    await write.save(d.id, now.data.base, now.title, now.manuscript); changed.current = false; await load();
  });
  const applyEdit = async (action: Edit) => {
    return run(async () => { const saved = await saveDraft(); const r = await write.edit(d.id, saved.head, action); await load(r.sectionId || undefined); });
  };
  const setOptions = (patch: View) => {
    if (window.innerWidth <= 760) {
      if (patch.binder) patch.research = false; if (patch.research) patch.binder = false;
    }
    setView(v => ({ ...v, ...patch }));
    void write.view(d.id, patch).catch(e => onError(errorText(e)));
  };
  const selectSection = (id: string) => {
    setSelected(id); setSelection({ section: id, from: 0, to: 0 }); setJump(undefined);
    setOptions({ section: id });
    void api.selection('write', { section: id, from: 0, to: 0 }).catch(e => onError(errorText(e)));
    if (window.innerWidth <= 760) setOptions({ binder: false });
  };
  const recordSelection = useCallback((s: Selection) => {
    setSelection(s); void api.selection('write', s).catch(() => {});
  }, []);
  const renderedStats = useCallback((s: Stats) => { setStats(was => ({ ...was, [selected]: s })); }, [selected]);
  const pending = busy || working || !data;
  if (!manuscript || !data) return <div className="console-document" hidden={!visible} inert={!visible}><p className="write-loading">Opening manuscript...</p></div>;
  const section = manuscript.sections.find(s => s.id === selected) ?? manuscript.sections[0];
  const depth = depths(manuscript.sections);
  const siblings = manuscript.sections.filter(s => s.parent === section.parent);
  const place = siblings.findIndex(s => s.id === section.id);
  const previous = siblings[place - 1]; const next = siblings[place + 1];
  const parent = manuscript.sections.find(s => s.id === section.parent);
  const grandSiblings = parent ? manuscript.sections.filter(s => s.parent === parent.parent) : [];
  const afterParent = parent ? grandSiblings[grandSiblings.findIndex(s => s.id === parent.id) + 1] : null;
  const excluded = descendantIds(manuscript.sections, section.id);
  const total = manuscript.sections.reduce((n, s) => n + (stats[s.id]?.words ?? 0), 0);
  const activeStats = stats[section.id];
  const selectedRange = selection?.section === section.id ? selection : { section: section.id, from: 0, to: 0 };
  const style = (style: string) => { void applyEdit({ type: 'format', section: section.id, from: selectedRange.from, to: selectedRange.to, style }); };
  const newSection = (parent: string | null) => { setNewTitle(''); setNewKind(manuscript.mode === 'lyrics' ? 'verse' : manuscript.mode === 'screenplay' ? 'scene' : 'section'); setAdding({ parent }); };
  return <div className="console-document write-document" hidden={!visible} inert={!visible} data-active={visible}>
    <div className="console-document-bar">
      <input aria-label="Document title" value={title} disabled={pending} maxLength={200} onChange={e => { setTitle(e.target.value); changed.current = true; setDirty(true); }} />
      <span className="console-version-label">v{d.versions}{dirty ? ' / Unsaved' : ''}</span>
      <div className="console-document-actions">
        <IconButton label="Save version" data-save disabled={pending} onClick={save}><Save size={17} /></IconButton>
        <IconButton label="Undo document edit" disabled={pending || dirty || !d.canUndo} onClick={() => onUndo(false)}><Undo2 size={17} /></IconButton>
        <IconButton label="Redo document edit" disabled={pending || dirty || !d.canRedo} onClick={() => onUndo(true)}><Redo2 size={17} /></IconButton>
        <IconButton label="Save as variation" disabled={pending || dirty} onClick={onVariation}><GitBranch size={17} /></IconButton>
        <IconButton label="Version history" onClick={() => onPanel('history')}><History size={17} /></IconButton>
        <IconButton label="Read saved version" onClick={() => onPanel('reader')}><Eye size={17} /></IconButton>
        <IconButton label="Ask Claude" onClick={() => onPanel('claude')}><MessageSquare size={17} /></IconButton>
        <IconButton label="Export document" disabled={pending} onClick={() => { if (manuscript.mode !== 'screenplay' && format === 'fountain') setFormat('markdown'); setExportPath(''); setExporting(true); }}><Download size={17} /></IconButton>
        <IconButton label="Prepare Space post" disabled={pending || dirty} onClick={onPost}><ArrowUpRight size={17} /></IconButton>
      </div>
    </div>
    {data.base !== d.head && dirty && <p className="console-conflict" role="alert">A newer version exists. Your unsaved manuscript is preserved here.</p>}
    <div className="write-options">
      <div><IconButton label="Toggle binder" aria-pressed={!!view.binder} onClick={() => setOptions({ binder: !view.binder })}><PanelLeft size={16} /></IconButton>
        <select aria-label="Writing mode" value={manuscript.mode} disabled={pending} onChange={e => void applyEdit({ type: 'mode', mode: e.target.value as Mode })}><option value="prose">Prose</option><option value="lyrics">Lyrics</option><option value="screenplay">Screenplay</option></select></div>
      <div className="write-view-modes" role="group" aria-label="Writing view"><IconButton label="Edit source" aria-pressed={!view.preview} onClick={() => setOptions({ preview: false })}><Pencil size={15} /></IconButton><IconButton label="Live preview" aria-pressed={!!view.preview} onClick={() => setOptions({ preview: true })}><Eye size={15} /></IconButton></div>
      <div><IconButton label="Typewriter scroll" aria-pressed={!!view.typewriter} onClick={() => setOptions({ typewriter: !view.typewriter })}><AlignVerticalJustifyCenter size={16} /></IconButton><IconButton label="Focus mode" aria-pressed={!!view.focus} onClick={() => setOptions({ focus: !view.focus })}><Focus size={16} /></IconButton><IconButton label="Toggle research" aria-pressed={!!view.research} onClick={() => setOptions({ research: !view.research })}><NotebookPen size={16} /></IconButton></div>
    </div>
    <div className="write-layout" data-binder={!!view.binder} data-research={!!view.research} data-focus={!!view.focus}>
      {view.binder && <aside className="write-binder" aria-label="Section binder">
        <header><h2>{view.outline ? 'Outline' : 'Binder'}</h2><div><IconButton label="Toggle outline" aria-pressed={!!view.outline} onClick={() => setOptions({ outline: !view.outline })}><ListTree size={15} /></IconButton><IconButton label="Add section" disabled={pending} onClick={() => newSection(null)}><Plus size={16} /></IconButton></div></header>
        <ol className="write-section-list" onDragOver={e => e.preventDefault()} onDrop={e => { if (e.target !== e.currentTarget) return; e.preventDefault(); const id = e.dataTransfer.getData('application/x-wi-section'); if (id) void applyEdit({ type: 'move', section: id, parent: null, before: null }); setDropTarget(null); }}>
          {manuscript.sections.map(s => <li key={s.id} draggable={!pending} data-selected={s.id === section.id} data-drop={dropTarget === s.id} data-section-id={s.id} onDragStart={e => { e.dataTransfer.setData('application/x-wi-section', s.id); e.dataTransfer.effectAllowed = 'move'; }} onDragOver={e => { e.preventDefault(); e.stopPropagation(); setDropTarget(s.id); }} onDragEnd={() => setDropTarget(null)} onDrop={e => { e.preventDefault(); e.stopPropagation(); const id = e.dataTransfer.getData('application/x-wi-section'); if (id && id !== s.id) void applyEdit({ type: 'move', section: id, parent: s.parent, before: s.id }); setDropTarget(null); }}>
            <button type="button" className="write-section-button" aria-pressed={s.id === section.id} onClick={() => selectSection(s.id)} style={{ paddingLeft: 12 + (depth.get(s.id) ?? 0) * 14 }}><GripVertical size={12} /><span>{s.title}</span><small>{stats[s.id]?.words ?? 0}</small></button>
            {view.outline && <p className="write-outline-summary">{s.synopsis || s.text.replace(/[#*_`]/g, '').slice(0, 120) || s.kind}</p>}
          </li>)}
        </ol>
        <div className="write-binder-controls">
          <IconButton label="Move section up" disabled={pending || !previous} onClick={() => void applyEdit({ type: 'move', section: section.id, parent: section.parent, before: previous.id })}><ArrowUp size={15} /></IconButton>
          <IconButton label="Move section down" disabled={pending || !next} onClick={() => void applyEdit({ type: 'move', section: section.id, parent: section.parent, before: siblings[place + 2]?.id ?? null })}><ArrowDown size={15} /></IconButton>
          <IconButton label="Nest section" disabled={pending || !previous} onClick={() => void applyEdit({ type: 'move', section: section.id, parent: previous.id, before: null })}><ChevronRight size={15} /></IconButton>
          <IconButton label="Unnest section" disabled={pending || !parent} onClick={() => void applyEdit({ type: 'move', section: section.id, parent: parent!.parent, before: afterParent?.id ?? null })}><ChevronLeft size={15} /></IconButton>
          <IconButton label="Add subsection" disabled={pending} onClick={() => newSection(section.id)}><Plus size={15} /></IconButton>
          <IconButton label="Delete section" disabled={pending || excluded.size === manuscript.sections.length} onClick={() => setDeleting(section)}><Trash2 size={15} /></IconButton>
        </div>
        <footer>{manuscript.sections.length} sections / {total.toLocaleString()} words</footer>
      </aside>}
      <main className="write-page">
        <header className="write-section-heading"><input aria-label="Section title" disabled={pending} value={section.title} maxLength={200} onChange={e => editSection({ title: e.target.value })} />
          {manuscript.mode === 'lyrics' && <select aria-label="Lyrics section type" value={section.kind} disabled={pending} onChange={e => editSection({ kind: e.target.value })}>{['section', 'verse', 'hook', 'bridge', 'intro', 'outro'].map(k => <option key={k} value={k}>{k[0].toUpperCase() + k.slice(1)}</option>)}</select>}
        </header>
        {view.outline && <div className="write-section-outline"><textarea aria-label="Section synopsis" placeholder="Synopsis" disabled={pending} maxLength={4000} value={section.synopsis} onChange={e => editSection({ synopsis: e.target.value })} /><label>Parent<select aria-label="Section parent" value={section.parent ?? ''} disabled={pending} onChange={e => void applyEdit({ type: 'move', section: section.id, parent: e.target.value || null, before: null })}><option value="">Top level</option>{manuscript.sections.filter(s => !excluded.has(s.id)).map(s => <option key={s.id} value={s.id}>{s.title}</option>)}</select></label>
          {!!activeStats?.outline.length && <ul>{activeStats.outline.map((h, i) => <li key={i}><button type="button" onClick={() => { setOptions({ preview: false }); setJump(h.line); }}>{h.title}</button></li>)}</ul>}
        </div>}
        <div className="write-formatting">
          {manuscript.mode === 'screenplay' ? <select aria-label="Screenplay element" defaultValue="" disabled={pending} onChange={e => { if (e.target.value) style(e.target.value); e.target.value = ''; }}><option value="">Element</option><option value="scene">Scene heading</option><option value="action">Action</option><option value="character">Character</option><option value="dialogue">Dialogue</option><option value="parenthetical">Parenthetical</option><option value="transition">Transition</option></select> : <><IconButton label="Bold selection" disabled={pending} onClick={() => style('bold')}><Bold size={15} /></IconButton><IconButton label="Italic selection" disabled={pending} onClick={() => style('italic')}><Italic size={15} /></IconButton><select aria-label="Paragraph format" defaultValue="" disabled={pending} onChange={e => { if (e.target.value) style(e.target.value); e.target.value = ''; }}><option value="">Paragraph</option><option value="heading">Heading</option><option value="quote">Quote</option><option value="bullet">List</option><option value="code">Code</option></select></>}
          {manuscript.mode === 'lyrics' && <span>{activeStats?.lineCount ?? 0} lines / syllables estimated</span>}
        </div>
        <div className="write-sheet">
          <div className="write-source" hidden={!!view.preview} inert={!!view.preview}><Editor section={section.id} text={section.text} mode={manuscript.mode} disabled={pending} typewriter={!!view.typewriter} focus={!!view.focus} onChange={text => editSection({ text })} onSelection={recordSelection} jumpLine={jump} /></div>
          <div className="write-rendered" hidden={!view.preview} inert={!view.preview}><Preview text={section.text} mode={manuscript.mode} /></div>
          <div className="write-stats-worker" hidden aria-hidden><Preview text={section.text} mode={manuscript.mode} onStats={renderedStats} /></div>
        </div>
        <footer className="write-counts"><span>{(activeStats?.words ?? 0).toLocaleString()} words / {total.toLocaleString()} total</span>
          {manuscript.mode === 'lyrics' && <details><summary>Line counts</summary><table aria-label="Lyrics line and syllable counts"><thead><tr><th>Line</th><th>Words</th><th>Syllables (est.)</th></tr></thead><tbody>{activeStats?.lines.map(l => <tr key={l.line}><th title={l.text}>{l.line}</th><td>{l.words}</td><td>{l.syllables}</td></tr>)}</tbody></table></details>}
        </footer>
      </main>
      {view.research && <ResearchPanel manuscript={manuscript} busy={pending} onNotes={text => editDraft(m => ({ ...m, notes: text }))} onEdit={applyEdit} onClose={() => setOptions({ research: false })} onError={onError} />}
    </div>
    <div className="console-document-foot"><button type="button" className="console-provenance" onClick={() => onPanel('provenance')}>{d.marker}</button>{d.parent && <span><GitBranch size={12} />Variation of {d.parent.title}</span>}<span>{manuscript.mode === 'screenplay' ? 'Fountain' : 'Markdown'}</span></div>
    {adding && <Modal title="New section" onClose={() => setAdding(null)}><form className="write-source-form" onSubmit={e => { e.preventDefault(); void applyEdit({ type: 'add', title: newTitle, parent: adding.parent, kind: newKind }).then(ok => { if (ok) setAdding(null); }); }}><label>Title<input autoFocus required maxLength={200} value={newTitle} onChange={e => setNewTitle(e.target.value)} /></label>{manuscript.mode === 'lyrics' && <label>Type<select value={newKind} onChange={e => setNewKind(e.target.value)}>{['verse', 'hook', 'bridge', 'intro', 'outro', 'section'].map(k => <option key={k} value={k}>{k[0].toUpperCase() + k.slice(1)}</option>)}</select></label>}<button className="console-primary" disabled={pending || !newTitle.trim()}><Plus size={15} />Add section</button></form></Modal>}
    {deleting && <Modal title="Delete section" onClose={() => setDeleting(null)}><div className="write-source-form"><p>Delete {deleting.title}{descendantIds(manuscript.sections, deleting.id).size > 1 ? ' and its subsections' : ''}?</p><div className="write-confirm-actions"><button type="button" className="console-primary" disabled={pending} onClick={() => void applyEdit({ type: 'delete', section: deleting.id }).then(ok => { if (ok) setDeleting(null); })}><Trash2 size={15} />Delete section</button><button type="button" className="console-plain" onClick={() => setDeleting(null)}>Cancel</button></div></div></Modal>}
    {exporting && <Modal title="Export document" onClose={() => setExporting(false)}><form className="write-source-form" onSubmit={e => { e.preventDefault(); void run(async () => { const saved = await saveDraft(); const result = await write.export(d.id, format, saved.head); await load(); setExportPath(result.path); download(result); }); }}><label>Format<select aria-label="Export format" value={format} onChange={e => setFormat(e.target.value)}><option value="markdown">Markdown</option><option value="pdf">PDF</option><option value="text">Plain text</option><option value="fountain" disabled={manuscript.mode !== 'screenplay'}>Fountain</option></select></label><button className="console-primary" disabled={pending}><Download size={15} />Export</button>{exportPath && <p className="write-export-path" role="status">Exported on this Mac: {exportPath}</p>}</form></Modal>}
  </div>;
}
