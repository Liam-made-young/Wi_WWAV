import { call } from '../../bridge';
import type { Document } from '../client';

export type Mode = 'prose' | 'lyrics' | 'screenplay';
export interface Section { id: string; parent: string | null; title: string; kind: string; synopsis: string; text: string }
export interface Research { id: string; title: string; url: string | null; noteId: string | null; excerpt: string }
export interface Manuscript { format: 'wi-write/1'; mode: Mode; sections: Section[]; notes: string; research: Research[] }
export interface Stats { words: number; characters: number; lineCount: number; syllablesEstimated: boolean; lines: { line: number; text: string; syllables: number; words: number }[]; outline: { level: number; line: number; title: string }[] }
export interface View { section?: string | null; focus?: boolean; typewriter?: boolean; preview?: boolean; binder?: boolean; research?: boolean; outline?: boolean }
export interface WriteData { document: Document; base: string; manuscript: Manuscript; stats: { words: number; sections: { id: string; stats: Stats }[] }; view: View }
export interface Selection { section: string; from: number; to: number; text?: string }
export type Edit =
  | { type: 'add'; title: string; parent: string | null; kind: string }
  | { type: 'update'; section: string; title?: string; text?: string; kind?: string; synopsis?: string }
  | { type: 'move'; section: string; parent: string | null; before: string | null }
  | { type: 'delete'; section: string }
  | { type: 'mode'; mode: Mode }
  | { type: 'notes'; text: string }
  | { type: 'reorder'; order: string[] }
  | { type: 'replace'; section: string; from: number; to: number; text: string }
  | { type: 'format'; section: string; from: number; to: number; style: string }
  | { type: 'linkNote'; note: string }
  | { type: 'researchAdd'; title: string; url: string | null; excerpt: string }
  | { type: 'researchRemove'; source: string };
export interface WriteProposal { id: string; summary: string; before: string; args: { action: Extract<Edit, { type: 'replace' | 'reorder' }> } }
export type Intent = 'rewrite' | 'tighten' | 'rhymes' | 'alternatives' | 'summarize' | 'continue' | 'reorder';
export const write = {
  read: (id: string, version?: string) => call<WriteData>('console.write.read', { id, version }),
  save: (id: string, base: string, title: string, manuscript: Manuscript) => call<{ document: Document }>('console.write.save', { id, base, title, manuscript }),
  edit: (id: string, base: string, action: Edit) => call<{ document: Document; sectionId: string | null }>('console.write.edit', { id, base, action }),
  render: (text: string, mode: Mode) => call<{ html: string; stats: Stats }>('console.write.render', { text, mode }),
  view: (id: string, view: View) => call('console.write.view', { id, view }),
  transform: (text: string, from: number, to: number, style: string) => call<{ text: string }>('console.write.transform', { text, from, to, style }),
  export: (id: string, format: string, version?: string) => call<{ path: string; name: string; base64: string; mime: string; version: string }>('console.write.export', { id, format, version }),
  assist: (id: string, base: string, section: string, selection: Selection, intent: Intent, prompt: string) => call<{ answer: string; proposal: WriteProposal | null }>('console.write.assist', { id, base, section, selection, intent, prompt }),
  searchNotes: (query: string) => call<{ hits: { id: string; title: string; snippet: string }[] }>('console.write.research.search', { query }),
  readNote: (note: string) => call<{ title: string; markdown: string }>('console.write.research.read', { note }),
};

export function download(data: { name: string; base64: string; mime: string }) {
  const bytes = Uint8Array.from(atob(data.base64), c => c.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes], { type: data.mime }));
  const a = document.createElement('a'); a.href = url; a.download = data.name; a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}
