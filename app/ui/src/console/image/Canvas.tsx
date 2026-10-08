import { useEffect, useRef, useState, type PointerEvent as ReactPointerEvent } from 'react';
import type { Action, ImageData, Point, Preview, Selection, Shape, View } from './client';
import { world } from './client';

interface Gesture { start: Point; points: Point[]; screen: { x: number; y: number }; view: View; mode: View['tool']; matrix?: DOMMatrix }
export interface CanvasProps { data: ImageData; view: View; preview: Preview; busy: boolean; update(v: View): void; edit(a: Action): Promise<void>; make(shape: Shape): Promise<void>; select(shape: Selection | null): Promise<void>; sample(p: Point): Promise<void>; detail(region: Preview['region'] | null): void; detailPreview: Preview | null }
const path = (points: Point[]) => points.map((p, i) => `${i ? 'L' : 'M'} ${p.x} ${p.y}`).join(' ');
function rect(a: Point, b: Point) { return { x: Math.min(a.x, b.x), y: Math.min(a.y, b.y), width: Math.max(1, Math.abs(b.x - a.x)), height: Math.max(1, Math.abs(b.y - a.y)) }; }
export function ImageCanvas({ data, view, preview, busy, update, edit, make, select, sample, detail, detailPreview }: CanvasProps) {
  const host = useRef<HTMLDivElement>(null);
  const [bounds, setBounds] = useState({ width: 800, height: 600 });
  const [draft, setDraft] = useState<Gesture | null>(null);
  const gesture = useRef<Gesture | null>(null);
  const trackpadPressure = useRef(1);
  useEffect(() => {
    const el = host.current!;
    const force = (e: Event) => { const value = (e as MouseEvent & { webkitForce?: number }).webkitForce; if (value && Number.isFinite(value)) trackpadPressure.current = Math.min(1, Math.max(0.05, value / 3)); };
    const reset = () => { trackpadPressure.current = 1; };
    el.addEventListener('webkitmouseforcechanged', force); el.addEventListener('pointerleave', reset);
    return () => { el.removeEventListener('webkitmouseforcechanged', force); el.removeEventListener('pointerleave', reset); };
  }, []);
  const [pen, setPen] = useState<{ p: Point; dx: number; dy: number }[]>([]);
  const [text, setText] = useState<{ point: Point; value: string } | null>(null);
  const { width, height } = data.image;
  const scale = Math.max(0.001, Math.min((bounds.width - 64) / width, (bounds.height - 64) / height)) * (view.zoom ?? 1);
  const rotation = view.rotation ?? 0;
  const matrix = new DOMMatrix().translate(bounds.width / 2 + (view.panX ?? 0), bounds.height / 2 + (view.panY ?? 0)).rotate(rotation).scale(scale).translate(-width / 2, -height / 2);
  const matrixRef = useRef(matrix); matrixRef.current = matrix;
  const layer = data.image.layers.find(l => l.id === view.layer) ?? data.image.layers[0];
  const object = layer.objects?.find(o => o.id === view.object);
  useEffect(() => { const el = host.current; if (!el) return; const observer = new ResizeObserver(([entry]) => setBounds({ width: entry.contentRect.width, height: entry.contentRect.height })); observer.observe(el); return () => observer.disconnect(); }, []);
  useEffect(() => {
    if (draft || scale <= 1 || rotation !== 0) { detail(null); return; }
    const inverse = matrix.inverse(); const a = inverse.transformPoint({ x: 0, y: 0 }); const b = inverse.transformPoint({ x: bounds.width, y: bounds.height });
    const x = Math.max(0, a.x); const y = Math.max(0, a.y); const w = Math.min(width, b.x) - x; const h = Math.min(height, b.y) - y;
    const timer = setTimeout(() => { detail(w > 0 && h > 0 ? { x, y, width: w, height: h } : null); }, 180);
    return () => clearTimeout(timer);
  // The matrix is derived entirely from these stable primitives.
  // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [bounds.width, bounds.height, scale, rotation, view.panX, view.panY, width, height, data.base, draft !== null]);
  const point = (e: { clientX: number; clientY: number; pressure?: number; pointerType?: string }, transform = matrixRef.current): Point => { const b = host.current!.getBoundingClientRect(); const p = transform.inverse().transformPoint({ x: e.clientX - b.left, y: e.clientY - b.top }); return { x: p.x, y: p.y, pressure: e.pointerType === 'mouse' ? trackpadPressure.current : e.pressure || 0.5 }; };
  const penPath = () => pen.map((n, i) => i ? `C ${pen[i - 1].p.x + pen[i - 1].dx} ${pen[i - 1].p.y + pen[i - 1].dy} ${n.p.x - n.dx} ${n.p.y - n.dy} ${n.p.x} ${n.p.y}` : `M ${n.p.x} ${n.p.y}`).join(' ');
  const finishPen = () => { if (pen.length > 1) { void make({ type: 'path', d: penPath() }); setPen([]); } };
  const down = (e: ReactPointerEvent<HTMLDivElement>) => {
    if (e.button !== 0 && e.button !== 1) return;
    const mode = e.button === 1 || e.altKey ? 'pan' : view.tool ?? 'brush';
    if (busy && mode !== 'pan') return;
    if ((e.target as HTMLElement).closest('input,textarea,button')) return;
    e.preventDefault(); e.currentTarget.focus(); e.currentTarget.setPointerCapture(e.pointerId);
    const p = point(e);
    if (mode !== 'pan' && (p.x < 0 || p.y < 0 || p.x > width || p.y > height)) return;
    if (mode === 'eyedropper') { void sample(p); return; }
    if (mode === 'fill') { void edit({ type: 'fill', layer: layer.id, x: Math.min(width - 1, Math.floor(p.x)), y: Math.min(height - 1, Math.floor(p.y)), color: view.color ?? '#171717', tolerance: view.tolerance ?? 24, contiguous: true, mask: !!view.mask }); return; }
    if (mode === 'wand') { void select({ type: 'wand', x: Math.min(width - 1, Math.floor(p.x)), y: Math.min(height - 1, Math.floor(p.y)), tolerance: view.tolerance ?? 24, contiguous: true }); return; }
    if (mode === 'text') { setText({ point: p, value: '' }); return; }
    const g = { start: p, points: [p], screen: { x: e.clientX, y: e.clientY }, view: { ...view }, mode, matrix: matrixRef.current }; gesture.current = g; setDraft(g);
  };
  const move = (e: ReactPointerEvent<HTMLDivElement>) => {
    const g = gesture.current; if (!g) return;
    if (g.mode === 'pan') { update({ panX: (g.view.panX ?? 0) + e.clientX - g.screen.x, panY: (g.view.panY ?? 0) + e.clientY - g.screen.y }); return; }
    const events = e.nativeEvent.getCoalescedEvents?.() ?? [e.nativeEvent];
    const points = [...g.points, ...events.map(event => point(event, g.matrix))].slice(0, g.mode === 'lasso' ? 4096 : 8192);
    const next = { ...g, points }; gesture.current = next; setDraft(next);
  };
  const up = (e: ReactPointerEvent<HTMLDivElement>) => {
    const g = gesture.current; if (!g) return; gesture.current = null; setDraft(null); const end = point(e, g.matrix); const box = rect(g.start, end);
    if (g.mode === 'pan') return;
    if (g.mode === 'pen') { setPen(previous => [...previous, { p: g.start, dx: end.x - g.start.x, dy: end.y - g.start.y }]); return; }
    if (g.mode === 'brush' || g.mode === 'eraser') { void edit({ type: 'stroke', layer: layer.id, points: [...g.points, end], size: view.size ?? 18, color: view.color ?? '#171717', opacity: view.opacity ?? 1, eraser: g.mode === 'eraser', mask: !!view.mask }); }
    else if (g.mode === 'rect') void make({ type: 'rect', ...box, radius: 0 });
    else if (g.mode === 'ellipse') void make({ type: 'ellipse', ...box });
    else if (g.mode === 'select') void select({ type: 'rect', ...box });
    else if (g.mode === 'lasso') { if (g.points.length >= 3) void select({ type: 'lasso', points: g.points }); }
    else if (g.mode === 'crop') { const x = Math.max(0, Math.floor(box.x)); const y = Math.max(0, Math.floor(box.y)); const w = Math.min(width - x, Math.round(box.width)); const h = Math.min(height - y, Math.round(box.height)); if (w > 0 && h > 0) void edit({ type: 'crop', x, y, width: w, height: h }); }
    else if (g.mode === 'move') {
      const dx = end.x - g.start.x; const dy = end.y - g.start.y;
      const parent = object ? world(data.image, layer.id).inverse() : layer.parent ? world(data.image, layer.parent).inverse() : new DOMMatrix();
      const x = parent.a * dx + parent.c * dy; const y = parent.b * dx + parent.d * dy;
      if (object) void edit({ type: 'vectorUpdate', layer: layer.id, object: { ...object, transform: { ...object.transform, e: object.transform.e + x, f: object.transform.f + y } } });
      else void edit({ type: 'transform', layer: layer.id, matrix: { ...layer.transform, e: layer.transform.e + x, f: layer.transform.f + y } });
    }
  };
  useEffect(() => { setPen([]); setText(null); }, [view.tool, data.document.id]);
  const wheel = (e: WheelEvent) => {
    e.preventDefault();
    const current = latest.current;
    if (e.ctrlKey || e.metaKey) {
      const z = Math.min(32, Math.max(0.1, (current.view.zoom ?? 1) * Math.exp(-e.deltaY * 0.01)));
      const b = host.current!.getBoundingClientRect(); const x = e.clientX - b.left - b.width / 2; const y = e.clientY - b.top - b.height / 2; const ratio = z / (current.view.zoom ?? 1);
      current.update({ zoom: z, panX: x - (x - (current.view.panX ?? 0)) * ratio, panY: y - (y - (current.view.panY ?? 0)) * ratio });
    } else current.update({ panX: (current.view.panX ?? 0) - e.deltaX, panY: (current.view.panY ?? 0) - e.deltaY });
  };
  const latest = useRef({ view, update }); latest.current = { view, update };
  useEffect(() => { const el = host.current!; el.addEventListener('wheel', wheel, { passive: false }); return () => el.removeEventListener('wheel', wheel); }, []);
  const selection = data.selection?.shape;
  const draftEnd = draft?.points.at(-1); const box = draft && draftEnd ? rect(draft.start, draftEnd) : null;
  return <div ref={host} className={`image-viewport image-tool-${view.tool ?? 'brush'}`} tabIndex={0} aria-label="Image canvas" onPointerDown={down} onPointerMove={move} onPointerUp={up} onPointerCancel={() => { gesture.current = null; setDraft(null); }} onDoubleClick={finishPen} onKeyDown={e => { if (e.target !== e.currentTarget) return; if (e.key === 'Escape') { setPen([]); setText(null); void select(null); } if (e.key === 'Enter') finishPen(); }}>
    <div className="image-artboard" style={{ width, height, transform: `matrix(${matrix.a},${matrix.b},${matrix.c},${matrix.d},${matrix.e},${matrix.f})` }}>
      <img draggable={false} src={`data:${preview.mime};base64,${preview.base64}`} alt={data.document.title} style={{ width, height }} />
      {detailPreview && <img className="image-detail" draggable={false} alt="" src={`data:${detailPreview.mime};base64,${detailPreview.base64}`} style={{ left: detailPreview.region.x, top: detailPreview.region.y, width: detailPreview.region.width, height: detailPreview.region.height }} />}
      <svg className="image-feedback" width={width} height={height} viewBox={`0 0 ${width} ${height}`}>
        {selection?.type === 'rect' && <rect {...selection} fill="none" stroke="#222" strokeDasharray={`${5 / scale} ${4 / scale}`} strokeWidth={1.5 / scale} />}
        {selection?.type === 'lasso' && <path d={`${path(selection.points)} Z`} fill="none" stroke="#222" strokeDasharray={`${5 / scale} ${4 / scale}`} strokeWidth={1.5 / scale} />}
        {draft && ['brush', 'eraser'].includes(draft.mode ?? '') && <path d={path(draft.points)} fill="none" stroke={draft.mode === 'eraser' ? '#aaa' : view.color ?? '#171717'} strokeWidth={view.size ?? 18} strokeLinecap="round" strokeLinejoin="round" opacity={view.opacity ?? 1} />}
        {box && ['rect', 'select', 'crop'].includes(draft?.mode ?? '') && <rect {...box} fill="none" stroke={view.color ?? '#171717'} strokeWidth={1.5 / scale} strokeDasharray={draft?.mode === 'rect' ? undefined : `${5 / scale} ${4 / scale}`} />}
        {box && draft?.mode === 'ellipse' && <ellipse cx={box.x + box.width / 2} cy={box.y + box.height / 2} rx={box.width / 2} ry={box.height / 2} fill="none" stroke={view.color ?? '#171717'} strokeWidth={1.5 / scale} />}
        {draft?.mode === 'lasso' && <path d={path(draft.points)} fill="none" stroke="#171717" strokeWidth={1.5 / scale} />}
        {pen.length > 0 && <><path d={penPath()} fill="none" stroke={view.color ?? '#171717'} strokeWidth={2 / scale} />{pen.map((n, i) => <g key={i}><line x1={n.p.x - n.dx} y1={n.p.y - n.dy} x2={n.p.x + n.dx} y2={n.p.y + n.dy} stroke="#407bd0" strokeWidth={1 / scale} /><circle cx={n.p.x} cy={n.p.y} r={3 / scale} fill="#407bd0" /></g>)}</>}
      </svg>
    </div>
    {text && <form className="image-text-entry" onPointerDown={e => e.stopPropagation()} onSubmit={e => { e.preventDefault(); if (text.value.trim()) { void make({ type: 'text', x: text.point.x, y: text.point.y, text: text.value, size: 64, font: 'IBM Plex Serif' }); setText(null); } }}><textarea autoFocus aria-label="Canvas text" value={text.value} onChange={e => setText({ ...text, value: e.target.value })} /><button className="console-primary" disabled={!text.value.trim()}>Place text</button><button type="button" className="console-plain" onClick={() => setText(null)}>Cancel</button></form>}
    {busy && <span className="image-processing" role="status">Saving edit...</span>}
  </div>;
}
