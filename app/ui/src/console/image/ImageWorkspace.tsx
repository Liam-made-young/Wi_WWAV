import { useCallback, useEffect, useRef, useState } from 'react';
import { ArrowUpRight, Brush, Circle, Crop, Download, Eraser, Eye, FolderOpen, GitBranch, Hand, History, Layers, Lasso, MessageSquare, MousePointer2, PaintBucket, PenTool, Pipette, Redo2, RotateCcw, RotateCw, Save, Scan, Square, Trash2, Type, Undo2, Wand2, ZoomIn, ZoomOut } from 'lucide-react';
import type { WorkspaceProps } from '../DocumentWorkspace';
import { on } from '../../bridge';
import { errorText } from '../client';
import { IconButton } from '../Panels';
import { ImageCanvas } from './Canvas';
import { ImageInspector } from './Inspector';
import { affine, download, fileBytes, image, world, type Action, type Adjustments, type ImageData, type Point, type Preview, type Selection, type Shape, type Tool, type Vector, type View } from './client';
import './image.css';

const tools = [{ id: 'move', label: 'Move', Icon: MousePointer2 }, { id: 'brush', label: 'Brush', Icon: Brush }, { id: 'eraser', label: 'Eraser', Icon: Eraser }, { id: 'fill', label: 'Fill', Icon: PaintBucket }, { id: 'rect', label: 'Rectangle', Icon: Square }, { id: 'ellipse', label: 'Ellipse', Icon: Circle }, { id: 'pen', label: 'Vector pen', Icon: PenTool }, { id: 'text', label: 'Text', Icon: Type }, { id: 'eyedropper', label: 'Eyedropper', Icon: Pipette }, { id: 'select', label: 'Rectangle selection', Icon: Scan }, { id: 'lasso', label: 'Lasso selection', Icon: Lasso }, { id: 'wand', label: 'Magic wand', Icon: Wand2 }, { id: 'pan', label: 'Pan', Icon: Hand }, { id: 'crop', label: 'Crop', Icon: Crop }] as const;
export function ImageWorkspace({ document: d, visible, busy: outerBusy, onDirty, onError, onPanel, onVariation, onUndo, onPost, onRefresh }: WorkspaceProps) {
  const [data, setData] = useState<ImageData | null>(null);
  const [view, setView] = useState<View>({ tool: 'brush', zoom: 1, color: '#171717', size: 18, opacity: 1, inspector: true });
  const [title, setTitle] = useState(d.title);
  const titleDirty = useRef(false); titleDirty.current = title !== d.title;
  const [adjusting, setAdjusting] = useState(false);
  const [preview, setPreview] = useState<Preview | null>(null);
  const [detailPreview, setDetailPreview] = useState<Preview | null>(null);
  const [detailRegion, setDetailRegion] = useState<Preview['region'] | null>(null);
  const [busy, setBusy] = useState(false);
  const [exportFormat, setExportFormat] = useState('png');
  const [status, setStatus] = useState('');
  const [adjustments, setAdjustments] = useState<Adjustments | null>(null);
  const file = useRef<HTMLInputElement>(null);
  const current = useRef<ImageData | null>(null);
  const currentView = useRef(view); currentView.current = view;
  const operation = useRef(false);
  const readSequence = useRef(0);
  const callbacks = useRef({ onError, onRefresh, onDirty }); callbacks.current = { onError, onRefresh, onDirty };
  const read = useCallback(async (initial = false) => {
    const seq = ++readSequence.current; const r = await image.read(d.id); if (seq !== readSequence.current) return r;
    current.current = r; setData(r); setPreview(r.preview); setDetailPreview(null);
    if (initial) { setView(v => ({ ...v, ...r.view, layer: r.view.layer ?? r.image.layers[0].id })); setTitle(r.document.title); }
    else { if (!titleDirty.current) setTitle(r.document.title); setView(v => ({ ...v, layer: r.image.layers.some(l => l.id === v.layer) ? v.layer : r.image.layers[0].id, object: r.image.layers.some(l => l.objects?.some(o => o.id === v.object)) ? v.object : null })); }
    return r;
  }, [d.id]);
  useEffect(() => { void read(current.current === null).catch(e => callbacks.current.onError(errorText(e))); }, [d.head, read]);
  useEffect(() => on('console', payload => {
    const event = payload as { command?: string; actor?: string; workspace?: { tools?: { image?: { selection?: ImageData['selection']; views?: Record<string, View> } } } };
    if (event.command === 'console.image.selection') { const s = event.workspace?.tools?.image?.selection; if (s?.document === d.id) { setData(data => data ? { ...data, selection: s } : data); if (current.current) current.current = { ...current.current, selection: s }; } }
    if (event.command === 'console.image.view' && event.actor === 'claude') { const prefs = event.workspace?.tools?.image?.views?.[d.id]; if (prefs) setView(v => { const next = { ...v, ...prefs }; return JSON.stringify(next) === JSON.stringify(v) ? v : next; }); }
  }), [d.id]);
  useEffect(() => { onDirty(title !== d.title || adjusting || busy); }, [title, d.title, adjusting, busy, onDirty]);
  useEffect(() => {
    if (current.current === null) return;
    const timer = setTimeout(() => { void image.view(d.id, view).catch(e => callbacks.current.onError(errorText(e))); }, 300);
    return () => clearTimeout(timer);
  }, [d.id, view]);
  useEffect(() => {
    if (!data || !visible) return;
    let live = true;
    if (!adjustments) { setPreview(data.preview); return; }
    const timer = setTimeout(() => { image.preview(d.id, undefined, view.layer ?? data.image.layers[0].id, adjustments).then(p => { if (live) { setPreview(p); setDetailPreview(null); } }, e => { if (live) callbacks.current.onError(errorText(e)); }); }, 180);
    return () => { live = false; clearTimeout(timer); };
  }, [adjustments, data, d.id, visible, view.layer]);
  useEffect(() => {
    if (!detailRegion || !visible || adjustments || busy) { setDetailPreview(null); return; }
    let live = true;
    image.preview(d.id, detailRegion).then(p => { if (live) setDetailPreview(p); }, e => { if (live) callbacks.current.onError(errorText(e)); });
    return () => { live = false; };
  }, [detailRegion, data?.base, visible, d.id, adjustments, busy]);
  const update = (patch: View) => { currentView.current = { ...currentView.current, ...patch }; setView(currentView.current); };
  const run = async (fn: () => Promise<void>) => {
    if (operation.current) return; operation.current = true; setBusy(true); setStatus('');
    try { await fn(); } catch (e) { onError(errorText(e)); } finally { operation.current = false; setBusy(false); }
  };
  const edit = async (action: Action) => run(async () => {
    const r = current.current; if (!r) return;
    const result = await image.edit(d.id, r.base, action);
    if (result.layerId) update({ layer: result.layerId, object: null, mask: false });
    setAdjustments(null); setAdjusting(false); await read(); await onRefresh();
  });
  const make = async (shape: Shape) => run(async () => {
    let r = current.current; if (!r) return;
    let target = r.image.layers.find(l => l.id === currentView.current.layer);
    if (!target || target.type !== 'vector') {
      const added = await image.edit(d.id, r.base, { type: 'add', kind: 'vector', name: 'Vector layer', parent: null });
      r = await image.read(d.id); current.current = r;
      target = r.image.layers.find(l => l.id === added.layerId)!;
    }
    const object: Vector = { id: '', name: shape.type === 'text' ? shape.text.slice(0, 80) : `${shape.type[0].toUpperCase()}${shape.type.slice(1)}`, shape, fill: shape.type === 'path' ? null : currentView.current.color ?? '#171717', stroke: shape.type === 'path' ? currentView.current.color ?? '#171717' : null, strokeWidth: shape.type === 'path' ? 3 : 0, opacity: 1, transform: affine(world(r.image, target.id).inverse()) };
    await image.edit(d.id, r.base, { type: 'vectorAdd', layer: target.id, object }); const refreshed = await read();
    const latest = refreshed.image.layers.find(l => l.id === target!.id)!;
    update({ layer: latest.id, object: latest.objects?.at(-1)?.id ?? null, mask: false }); await onRefresh();
  });
  const select = async (shape: Selection | null) => run(async () => { const r = await image.select(d.id, currentView.current.layer ?? null, shape); setData(data => data ? { ...data, selection: r.selection } : data); if (current.current) current.current = { ...current.current, selection: r.selection }; });
  const sample = async (p: Point) => { try { const r = await image.sample(d.id, Math.floor(p.x), Math.floor(p.y)); update({ color: r.color }); } catch (e) { onError(errorText(e)); } };
  const save = () => run(async () => { if (!current.current) return; await image.save(d.id, current.current.base, title); titleDirty.current = false; await read(); await onRefresh(); });
  const chooseTool = async (tool: Tool) => {
    update({ tool });
    const r = current.current; if (!r || !['brush', 'eraser', 'fill'].includes(tool) || currentView.current.mask) return;
    const layer = r.image.layers.find(l => l.id === currentView.current.layer);
    if (layer?.type === 'pixel') return;
    const pixel = r.image.layers.find(l => l.type === 'pixel' && !l.locked);
    if (pixel) update({ layer: pixel.id, object: null });
    else await edit({ type: 'add', kind: 'pixel', name: 'Pixel layer', parent: null });
  };
  const pending = outerBusy || busy || !data || data.base !== d.head;
  const dirty = title !== d.title || adjusting;
  return <div className="console-document image-document" hidden={!visible} inert={!visible} data-active={visible}>
    <div className="console-document-bar"><input aria-label="Document title" value={title} maxLength={200} onChange={e => setTitle(e.target.value)} /><span className="console-version-label">v{d.versions}{dirty ? ' / Unsaved' : ''}</span><div className="console-document-actions">
      <IconButton label="Save version" data-save disabled={pending || adjusting} onClick={() => void save()}><Save size={17} /></IconButton><IconButton label="Undo document edit" disabled={pending || dirty || !d.canUndo} onClick={() => onUndo(false)}><Undo2 size={17} /></IconButton><IconButton label="Redo document edit" disabled={pending || dirty || !d.canRedo} onClick={() => onUndo(true)}><Redo2 size={17} /></IconButton><IconButton label="Save as variation" disabled={pending || dirty} onClick={onVariation}><GitBranch size={17} /></IconButton><IconButton label="Version history" onClick={() => onPanel('history')}><History size={17} /></IconButton><IconButton label="Read saved version" onClick={() => onPanel('reader')}><Eye size={17} /></IconButton><IconButton label="Ask Claude" disabled={pending || dirty} onClick={() => onPanel('claude')}><MessageSquare size={17} /></IconButton><IconButton label="Prepare Space post" disabled={pending || dirty} onClick={onPost}><ArrowUpRight size={17} /></IconButton>
    </div></div>
    <div className="image-options"><label className="image-color"><input aria-label="Paint color" type="color" value={(view.color ?? '#171717').slice(0, 7)} onChange={e => update({ color: e.target.value })} /></label><input className="image-hex" aria-label="Hex color" value={view.color ?? '#171717'} maxLength={9} onChange={e => { if (/^#[a-f\d]{6}([a-f\d]{2})?$/i.test(e.target.value)) update({ color: e.target.value }); }} /><label>Size<input aria-label="Brush size" type="number" min={0.5} max={1000} value={view.size ?? 18} onChange={e => update({ size: Math.max(0.5, Math.min(1000, Number(e.target.value))) })} /></label><label>Flow<input aria-label="Brush flow" type="range" min={0.01} max={1} step={0.01} value={view.opacity ?? 1} onChange={e => update({ opacity: Number(e.target.value) })} /></label><label>Tolerance<input aria-label="Fill tolerance" type="number" min={0} max={255} value={view.tolerance ?? 24} onChange={e => update({ tolerance: Math.max(0, Math.min(255, Number(e.target.value))) })} /></label><span className="image-options-spacer" /><IconButton label="Import image layer" disabled={pending} onClick={() => file.current?.click()}><FolderOpen size={17} /></IconButton><select aria-label="Image export format" value={exportFormat} onChange={e => setExportFormat(e.target.value)}>{['png', 'jpg', 'svg', 'pdf'].map(s => <option key={s} value={s}>{s.toUpperCase()}</option>)}</select><IconButton label="Export image" disabled={pending || dirty} onClick={() => void run(async () => { const r = await image.export(d.id, exportFormat); download(r); setStatus(r.path); })}><Download size={17} /></IconButton><IconButton label="Toggle image inspector" onClick={() => update({ inspector: view.inspector === false })}><Layers size={17} /></IconButton>
    </div>
    <input ref={file} type="file" accept="image/png,image/jpeg,image/webp,image/svg+xml" aria-label="Image layer file" hidden onChange={e => { const f = e.target.files?.[0]; e.target.value = ''; if (f) void run(async () => { const bytes = await fileBytes(f); const r = current.current; if (!r) return; const result = await image.edit(d.id, r.base, { type: 'import', name: f.name, base64: bytes, fit: false }); update({ layer: result.layerId ?? null, object: null, mask: false }); await read(); await onRefresh(); }); }} />
    <div className="image-work-area"><div className="image-tool-rail" role="toolbar" aria-label="Image tools">{tools.map(({ id, label, Icon }) => <IconButton key={id} label={label} aria-pressed={(view.tool ?? 'brush') === id} disabled={pending || adjusting} onClick={() => void chooseTool(id)}><Icon size={18} /></IconButton>)}<hr /><IconButton label="Clear selection" disabled={pending || !data?.selection?.shape} onClick={() => void select(null)}><Scan size={18} /></IconButton><IconButton label="Clear pixel layer" disabled={pending || adjusting || data?.image.layers.find(l => l.id === view.layer)?.type !== 'pixel'} onClick={() => void edit({ type: 'clear', layer: view.layer })}><Trash2 size={16} /></IconButton></div>
      {data && preview ? <ImageCanvas data={data} view={view} preview={preview} busy={pending || adjusting} update={update} edit={edit} make={make} select={select} sample={sample} detail={setDetailRegion} detailPreview={detailPreview} /> : <div className="image-viewport"><p role="status">Opening image...</p></div>}
      {data && <ImageInspector visible={view.inspector !== false} model={data.image} view={view} busy={pending} edit={edit} update={update} adjustment={setAdjustments} pending={setAdjusting} />}
    </div>
    <div className="image-palette" aria-label="Saved palette">{data?.image.palette.map((color, i) => <button key={`${color}-${i}`} type="button" className="image-swatch" style={{ background: color }} title={color} aria-label={`Use ${color}`} aria-pressed={view.color === color} onClick={() => update({ color })} onContextMenu={e => { e.preventDefault(); void edit({ type: 'palette', colors: data.image.palette.filter((_, n) => n !== i) }); }} />)}<IconButton label="Save palette color" disabled={pending || data?.image.palette.includes(view.color ?? '#171717')} onClick={() => data && void edit({ type: 'palette', colors: [...data.image.palette, view.color ?? '#171717'] })}><Save size={13} /></IconButton><span className="image-options-spacer" /><IconButton label="Zoom out" onClick={() => update({ zoom: Math.max(0.1, (view.zoom ?? 1) / 1.25) })}><ZoomOut size={15} /></IconButton><button className="console-plain image-zoom" title="Fit canvas" onClick={() => update({ zoom: 1, panX: 0, panY: 0, rotation: 0 })}>{Math.round((view.zoom ?? 1) * 100)}%</button><IconButton label="Zoom in" onClick={() => update({ zoom: Math.min(32, (view.zoom ?? 1) * 1.25) })}><ZoomIn size={15} /></IconButton><IconButton label="Rotate canvas view" onClick={() => update({ rotation: ((view.rotation ?? 0) + 15) % 360 })}><RotateCcw size={15} /></IconButton><IconButton label="Rotate image clockwise" disabled={pending || dirty} onClick={() => void edit({ type: 'rotate', quarter_turns: 1 })}><RotateCw size={15} /></IconButton></div>
    <div className="console-document-foot"><button type="button" className="console-provenance" onClick={() => onPanel('provenance')}>{d.marker}</button>{d.parent && <span><GitBranch size={12} />Variation of {d.parent.title}</span>}<span>{data ? `${data.image.width} x ${data.image.height}` : ''}{view.mask ? ' / Mask' : ''}</span>{status && <span className="image-export-path" role="status" title={status}>{status}</span>}</div>
  </div>;
}
