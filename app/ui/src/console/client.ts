import { call } from '../bridge';

export type ToolId = 'write' | 'image' | 'audiovisual' | 'three';
export interface Asset { name: string; file: string; mime: string; bytes: number; sha256: string; preview: string }
export interface Parent { documentId: string; versionId: string; title: string; claudeAssisted: boolean; originUnverified: boolean }
export interface Document { id: string; tool: ToolId; title: string; head: string; updatedAt: number; versions: number; parent: Parent | null; marker: string; asset: Asset; canUndo: boolean; canRedo: boolean }
export interface Version { id: string; previous: string | null; at: number; actor: 'hand' | 'claude' | 'import'; action: string; title: string; asset: Asset; restoredFrom: string | null; changes: string[] }
export interface ReadResult { document: Document; version: Version; text: string | null; base64: string | null; path: string; readOnly: true; preview?: { mime: string; base64: string } | null }
export interface ToolState { open: string[]; active: string | null; selection: unknown }
export interface Workspace { tool: ToolId; tools: Record<ToolId, ToolState> }
export interface Listing { root: string; documents: Document[]; issues: { file: string; message: string }[] }
export interface Proposal { id: string; command: string; summary: string; args: { id: string; base: string; title: string | null; text: string | null } }

export const api = {
  workspace: () => call<{ workspace: Workspace; root: string }>('console.workspace'),
  library: (tool?: ToolId, query = '') => call<Listing>('console.library', { tool, query }),
  select: (tool: ToolId) => call('console.selectTool', { tool }),
  create: (tool: ToolId, title: string) => call<{ document: Document }>('console.create', { tool, title }),
  open: (id: string) => call('console.open', { id }),
  close: (id: string) => call('console.close', { id }),
  read: (id: string, version?: string) => call<ReadResult>('console.read', { id, version }),
  save: (id: string, base: string, title: string, text?: string) => call<{ document: Document }>('console.save', { id, base, title, text }),
  variation: (id: string, base: string, title: string) => call('console.variation', { id, base, title }),
  restore: (d: Document, redo = false) => call('console.' + (redo ? 'redo' : 'undo'), { id: d.id, base: d.head }),
  history: (id: string) => call<{ versions: Version[]; parent: Parent | null; note: string }>('console.history', { id }),
  selection: (tool: ToolId, selection: unknown) => call('console.selection', { tool, selection }),
  ask: (tool: ToolId, id: string, prompt: string) => call<{ answer: string; proposal: Proposal | null }>('console.claude.ask', { tool, id, prompt }),
  apply: (proposalId: string) => call('console.claude.apply', { proposalId }),
  post: (d: Document) => call<{ path: string; message: string }>('console.post.prepare', { id: d.id, base: d.head }),
  import: async (file: File) => {
    if (file.size > 24 * 1024 * 1024) throw new Error('Phase 0 imports are limited to 24 MiB.');
    const base64 = await new Promise<string>((resolve, reject) => {
      const reader = new FileReader();
      reader.onload = () => resolve(String(reader.result).split(',')[1]);
      reader.onerror = () => reject(new Error('The file could not be read.'));
      reader.readAsDataURL(file);
    });
    return call('console.import', { name: file.name, base64 });
  },
};
export function errorText(e: unknown): string { return e instanceof Error ? e.message : String(e); }
