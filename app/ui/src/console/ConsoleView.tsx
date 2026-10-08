import { useCallback, useEffect, useRef, useState } from 'react';
import { Check, CircleHelp, Eye, Library, Plus, Search, Upload, X } from 'lucide-react';
import { on } from '../bridge';
import { useCurrentView } from '../shell/useCurrentView';
import { api, errorText, type Listing, type ToolId, type Workspace } from './client';
import { CONSOLE_CONTEXT_EVENT, CONSOLE_TOOLS, toolById } from './registry';
import { Reader } from './Reader';
import { ClaudePanel, HistoryPanel, IconButton, Modal, NameDialog, type NameSheet } from './Panels';
import { DocumentWorkspace } from './DocumentWorkspace';
import './console.css';

type Panel = { kind: 'history' | 'provenance' | 'reader' | 'claude'; id: string; version?: string } | null;

export function ConsoleView({ active: forced }: { active?: boolean }) {
  const root = useRef<HTMLDivElement>(null);
  const current = useCurrentView(root);
  const active = forced ?? current;
  const [workspace, setWorkspace] = useState<Workspace | null>(null);
  const [library, setLibrary] = useState<Listing | null>(null);
  const [filtered, setFiltered] = useState<Listing | null>(null);
  const [query, setQuery] = useState('');
  const [filter, setFilter] = useState<ToolId | ''>('');
  const [libraryOpen, setLibraryOpen] = useState(() => typeof matchMedia === 'undefined' || !matchMedia('(max-width: 760px)').matches);
  const [error, setError] = useState('');
  const [message, setMessage] = useState('');
  const [panel, setPanel] = useState<Panel>(null);
  const [nameSheet, setNameSheet] = useState<NameSheet | null>(null);
  const [busy, setBusy] = useState(false);
  const [dirty, setDirty] = useState<Record<string, boolean>>({});
  const importInput = useRef<HTMLInputElement>(null);
  const serial = useRef(0);
  const refresh = useCallback(async () => {
    const n = ++serial.current;
    try {
      const [w, l] = await Promise.all([api.workspace(), api.library()]);
      if (n !== serial.current) return;
      setWorkspace(w.workspace); setLibrary(l); setError('');
    } catch (e) { if (n === serial.current) setError(errorText(e)); }
  }, []);
  useEffect(() => { if (active) void refresh(); }, [active, refresh]);
  useEffect(() => on('console', payload => {
    const event = payload as { command?: string; workspace?: Workspace };
    if (['console.selection', 'console.write.view'].includes(event.command ?? '') && event.workspace) setWorkspace(event.workspace);
    else void refresh();
  }), [refresh]);
  useEffect(() => { if (!active) { setPanel(null); setNameSheet(null); } }, [active]);
  useEffect(() => {
    let live = true;
    const timer = setTimeout(() => {
      api.library(filter || undefined, query).then(l => { if (live) setFiltered(l); }, e => { if (live) setError(errorText(e)); });
    }, 120);
    return () => { live = false; clearTimeout(timer); };
  }, [filter, query, library]);
  const act = async (action: () => Promise<unknown>, said?: string) => {
    setBusy(true); setError('');
    try { await action(); await refresh(); if (said) setMessage(said); }
    catch (e) { setError(errorText(e)); }
    finally { setBusy(false); }
  };
  const selectedTool = workspace?.tool ?? 'write';
  const selectedId = workspace?.tools[selectedTool]?.active;
  const selected = library?.documents.find(d => d.id === selectedId);
  const openClaude = () => { if (selected) setPanel({ kind: 'claude', id: selected.id }); else setMessage('Open a document first.'); };
  useEffect(() => {
    if (!active || !workspace) return;
    window.dispatchEvent(new CustomEvent(CONSOLE_CONTEXT_EVENT, { detail: { tool: selectedTool, documentId: selectedId ?? null, selection: workspace.tools[selectedTool].selection } }));
  }, [active, workspace, selectedTool, selectedId]);

  // Capture only Console's additions, and leave the enclosing shell's keys
  // alone whenever another view or overlay owns focus.
  useEffect(() => {
    if (!active) return;
    const key = (e: KeyboardEvent) => {
      if (e.defaultPrevented || e.repeat || busy) return;
      const target = e.target as HTMLElement;
      const elsewhere = target !== document.body && target !== document.documentElement && !root.current?.contains(target);
      if (elsewhere || document.querySelector('.backdrop[data-overlay], .first-launch')) return;
      const field = target instanceof HTMLInputElement || target instanceof HTMLTextAreaElement || target instanceof HTMLSelectElement || target.isContentEditable;
      const command = e.metaKey || e.ctrlKey;
      if (e.key === 'Escape' && (panel || nameSheet)) { setPanel(null); setNameSheet(null); e.preventDefault(); e.stopImmediatePropagation(); return; }
      if (target.closest('dialog')) { e.stopImmediatePropagation(); return; }
      if (panel || nameSheet) return;
      if (command && e.code === 'KeyT' && !e.shiftKey && !e.altKey) { openClaude(); }
      else if (command && e.code === 'KeyS' && !e.altKey) { root.current?.querySelector<HTMLButtonElement>('[data-active="true"] [data-save]')?.click(); }
      else if (command && e.code === 'KeyZ' && !field && selected && !dirty[selected.id]) { void act(() => api.restore(selected, e.shiftKey)); }
      else if (!command && !field && !e.altKey && !e.shiftKey && /^[1-4]$/.test(e.key)) { void act(() => api.select(CONSOLE_TOOLS[Number(e.key) - 1].id)); }
      else return;
      e.preventDefault(); e.stopImmediatePropagation();
    };
    window.addEventListener('keydown', key, true);
    return () => window.removeEventListener('keydown', key, true);
  });

  const docs = library?.documents ?? [];
  return <div className="console-studio" ref={root} data-library={libraryOpen} aria-label="Console">
    <header className="console-topbar">
      <div className="console-name">Console <span>Studio</span></div>
      <div className="console-tool-switch" role="tablist" aria-label="Creative tools">
        {CONSOLE_TOOLS.map(({ id, title, key, Icon }) => <button type="button" role="tab" key={id} aria-selected={selectedTool === id} aria-controls={`console-tool-${id}`} title={`${title} (${key})`} disabled={busy || !workspace} onClick={() => void act(() => api.select(id))}><Icon size={18} /><span>{title}</span></button>)}
      </div>
      <div className="console-top-actions">
        <IconButton label="New document" disabled={!workspace || busy} onClick={() => setNameSheet({ tool: selectedTool })}><Plus size={19} /></IconButton>
        <IconButton label="Import file" disabled={!workspace || busy} onClick={() => importInput.current?.click()}><Upload size={18} /></IconButton>
        <IconButton label="Toggle Console Library" aria-pressed={libraryOpen} onClick={() => setLibraryOpen(v => !v)}><Library size={19} /></IconButton>
      </div>
      <input ref={importInput} className="console-file-input" type="file" aria-label="Import into Console" accept=".md,.txt,.fountain,.json,.gltf,.obj,.png,.jpg,.jpeg,.webp,.gif,.wav,.wwav,.mp3,.m4a,.ogg,.flac,.mp4,.swav,.webm" onChange={e => { const f = e.target.files?.[0]; if (f) void act(() => api.import(f), 'Imported on this Mac'); e.target.value = ''; }} />
    </header>
    <main className="console-workbench">
      {CONSOLE_TOOLS.map(tool => {
        const state = workspace?.tools[tool.id];
        const openDocs = (state?.open ?? []).flatMap(id => docs.find(d => d.id === id) ?? []);
        return <section key={tool.id} id={`console-tool-${tool.id}`} role="tabpanel" aria-label={tool.title} hidden={selectedTool !== tool.id} inert={selectedTool !== tool.id} className="console-tool">
          <div className="console-document-tabs" aria-label={`${tool.title} documents`}>
            {openDocs.map(d => <div key={d.id} className="console-document-tab" data-selected={state?.active === d.id}>
              <button type="button" aria-pressed={state?.active === d.id} onClick={() => void act(() => api.open(d.id))}>{d.title}{dirty[d.id] ? ' *' : ''}</button>
              <IconButton label={`Close ${d.title}`} disabled={dirty[d.id] || busy} onClick={() => void act(() => api.close(d.id))}><X size={13} /></IconButton>
            </div>)}
            <span className="console-tab-spacer" /><span className="console-tool-label">{tool.title}</span>
          </div>
          {!state?.active && <div className="console-empty-canvas"><tool.Icon size={40} strokeWidth={1} aria-hidden /><h1>{tool.title}</h1><button type="button" className="console-primary" disabled={!workspace || busy} onClick={() => setNameSheet({ tool: tool.id })}><Plus size={16} /> New document</button></div>}
          {openDocs.map(d => <DocumentWorkspace key={d.id} document={d} visible={state?.active === d.id && selectedTool === tool.id && active} busy={busy} onDirty={v => setDirty(was => was[d.id] === v ? was : { ...was, [d.id]: v })} onRefresh={refresh} onSave={async (base, title, text) => { await api.save(d.id, base, title, text); await refresh(); setMessage('Version saved'); }} onError={setError} onPanel={kind => setPanel({ kind, id: d.id })} onVariation={() => setNameSheet({ tool: d.tool, parent: d })} onUndo={redo => void act(() => api.restore(d, redo))} onPost={() => void act(async () => { const r = await api.post(d); setMessage(`${r.message} ${r.path}`); })} />)}
        </section>;
      })}
    </main>
    {libraryOpen && <aside className="console-library" aria-label="Console Library">
      <header><h2>Library</h2><IconButton label="Close Library" onClick={() => setLibraryOpen(false)}><X size={17} /></IconButton></header>
      <label className="console-search"><Search size={16} /><input aria-label="Search Console Library" placeholder="Search your work" value={query} onChange={e => setQuery(e.target.value)} /></label>
      <select aria-label="Filter Library by tool" value={filter} onChange={e => setFilter(e.target.value as ToolId | '')}><option value="">All tools</option>{CONSOLE_TOOLS.map(t => <option key={t.id} value={t.id}>{t.title}</option>)}</select>
      <div className="console-library-list">
        {(filtered ?? library)?.documents.map(d => { const t = toolById(d.tool); return <article key={d.id} className="console-library-item" data-selected={selectedId === d.id}>
          <button className="console-library-open" type="button" aria-label={`Open ${d.title}`} onClick={() => { void act(() => api.open(d.id)); if (window.innerWidth <= 760) setLibraryOpen(false); }}>
            <div className="console-file-preview"><t.Icon size={20} strokeWidth={1.3} /><span>{d.asset.preview.replace(/\s+/g, ' ').slice(0, 120) || (d.asset.mime === 'application/json' ? t.title : d.asset.name)}</span></div>
            <strong>{d.title}</strong><span className="console-file-facts">{t.title} <span>v{d.versions}</span></span>
          </button>
          <div className="console-library-bottom"><button type="button" className="console-provenance" onClick={() => setPanel({ kind: 'provenance', id: d.id })}>{d.marker === 'Made by hand' ? <Check size={12} /> : <CircleHelp size={12} />}{d.marker}</button><IconButton label={`Preview ${d.title}`} onClick={() => setPanel({ kind: 'reader', id: d.id })}><Eye size={15} /></IconButton></div>
        </article>; })}
        {(filtered ?? library)?.documents.length === 0 && <p className="console-empty-list">{query || filter ? 'No matching files' : 'No files yet'}</p>}
      </div>
      {!!library?.issues.length && <details className="console-issues"><summary>{library.issues.length} unreadable bundle(s)</summary>{library.issues.map(i => <p key={i.file}>{i.file}: {i.message}</p>)}</details>}
      <footer title={library?.root}>Saved on this Mac</footer>
    </aside>}
    <footer className="console-status" role="status"><span>{error || message || (workspace ? 'Saved on this Mac' : 'Connecting to Console...')}</span>{error && <button type="button" onClick={() => void refresh()}>Retry</button>}</footer>
    {nameSheet && <NameDialog sheet={nameSheet} busy={busy} onClose={() => setNameSheet(null)} onSubmit={title => void act(async () => { if (nameSheet.parent) await api.variation(nameSheet.parent.id, nameSheet.parent.head, title); else await api.create(nameSheet.tool, title); setNameSheet(null); })} />}
    {panel && <Modal title={panel.kind === 'reader' ? 'Reader' : panel.kind === 'claude' ? 'Claude' : panel.kind === 'history' ? 'Versions' : 'Provenance'} onClose={() => setPanel(null)} wide={panel.kind === 'reader'}>
      {panel.kind === 'reader' ? <Reader id={panel.id} version={panel.version} /> : panel.kind === 'claude' ? <ClaudePanel document={docs.find(d => d.id === panel.id)} dirty={!!dirty[panel.id]} onApplied={async () => { await refresh(); setMessage('Claude edit saved as a new version'); }} /> : <HistoryPanel id={panel.id} provenance={panel.kind === 'provenance'} onRead={version => setPanel({ kind: 'reader', id: panel.id, version })} />}
    </Modal>}
  </div>;
}
