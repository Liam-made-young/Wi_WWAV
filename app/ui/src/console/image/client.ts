import { call } from '../../bridge';
import type { Asset, Document } from '../client';
export interface Matrix { a: number; b: number; c: number; d: number; e: number; f: number }
export const identity = (): Matrix => ({ a: 1, b: 0, c: 0, d: 1, e: 0, f: 0 });
export interface Adjustments { exposure: number; contrast: number; saturation: number; temperature: number; tint: number; curves: [number, number][] }
export const neutral = (): Adjustments => ({ exposure: 0, contrast: 0, saturation: 0, temperature: 0, tint: 0, curves: [[0, 0], [1, 1]] });
export type Shape = { type: 'rect'; x: number; y: number; width: number; height: number; radius: number } | { type: 'ellipse'; x: number; y: number; width: number; height: number } | { type: 'path'; d: string } | { type: 'text'; x: number; y: number; text: string; size: number; font: string };
export interface Vector { id: string; name: string; shape: Shape; fill: string | null; stroke: string | null; strokeWidth: number; opacity: number; transform: Matrix }
export interface Layer { id: string; parent: string | null; type: 'pixel' | 'vector' | 'group'; name: string; opacity: number; blend: string; visible: boolean; locked: boolean; transform: Matrix; mask: Asset | null; adjustments: Adjustments; objects?: Vector[] }
export interface ImageRecord { format: string; width: number; height: number; background: string | null; layers: Layer[]; palette: string[] }
export type Tool = 'move' | 'brush' | 'eraser' | 'fill' | 'rect' | 'ellipse' | 'pen' | 'text' | 'eyedropper' | 'select' | 'lasso' | 'wand' | 'pan' | 'crop';
export interface View { layer?: string | null; object?: string | null; tool?: Tool; zoom?: number; panX?: number; panY?: number; rotation?: number; size?: number; opacity?: number; color?: string; mask?: boolean; tolerance?: number; inspector?: boolean }
export interface Point { x: number; y: number; pressure: number }
export interface Region { x: number; y: number; width: number; height: number }
export type Selection = ({ type: 'rect' } & Region) | { type: 'lasso'; points: Point[] } | { type: 'wand'; x: number; y: number; tolerance: number; contiguous: boolean };
export interface Preview { base64: string; mime: string; width: number; height: number; region: Region }
export interface ImageData { document: Document; base: string; image: ImageRecord; preview: Preview; view: View; selection: { document: string; layer: string | null; shape?: Selection; mask?: Asset } | null }
export type Action = Record<string, unknown> & { type: string };
export function world(model: ImageRecord, id: string): DOMMatrix {
  const layer = model.layers.find(l => l.id === id)!; const t = layer.transform;
  const local = new DOMMatrix([t.a, t.b, t.c, t.d, t.e, t.f]);
  return layer.parent ? world(model, layer.parent).multiply(local) : local;
}
export function affine(m: DOMMatrix): Matrix { return { a: m.a, b: m.b, c: m.c, d: m.d, e: m.e, f: m.f }; }
export interface ImageProposal { id: string; command: string; summary: string; args: { id: string; base: string; action?: Action; shape?: Selection } }
export const image = {
  read: (id: string, version?: string) => call<ImageData>('console.image.read', { id, version }),
  edit: (id: string, base: string, action: Action) => call<{ document: Document; layerId?: string }>('console.image.edit', { id, base, action }),
  save: (id: string, base: string, title: string) => call('console.image.save', { id, base, title }),
  view: (id: string, view: View) => call('console.image.view', { id, view }),
  select: (id: string, layer: string | null, shape: Selection | null) => call<{ selection: ImageData['selection'] }>('console.image.selection', { id, layer, shape }),
  preview: (id: string, region?: Region, layer?: string, adjustments?: Adjustments) => call<Preview>('console.image.preview', { id, region, layer, adjustments, maxEdge: 1536 }),
  sample: (id: string, x: number, y: number) => call<{ color: string }>('console.image.sample', { id, x, y }),
  export: (id: string, format: string) => call<{ path: string; name: string; base64: string; mime: string }>('console.image.export', { id, format }),
  assist: (id: string, base: string, layer: string | null, intent: string, prompt: string) => call<{ answer: string; proposal: ImageProposal | null }>('console.image.assist', { id, base, layer, intent, prompt }),
};
export async function fileBytes(file: File): Promise<string> {
  if (file.size > 64 * 1024 * 1024) throw new Error('Image imports are limited to 64 MiB.');
  return new Promise((resolve, reject) => { const reader = new FileReader(); reader.onload = () => resolve(String(reader.result).split(',')[1]); reader.onerror = () => reject(new Error('This file could not be read.')); reader.readAsDataURL(file); });
}
export function download(result: { base64: string; mime: string; name: string }) {
  const bytes = Uint8Array.from(atob(result.base64), c => c.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes], { type: result.mime }));
  const a = document.createElement('a'); a.href = url; a.download = result.name; a.click(); setTimeout(() => URL.revokeObjectURL(url), 30000);
}
