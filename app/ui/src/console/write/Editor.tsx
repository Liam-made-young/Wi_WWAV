import { useEffect, useRef } from 'react';
import { Compartment, EditorState, Transaction } from '@codemirror/state';
import { Decoration, EditorView, drawSelection, keymap } from '@codemirror/view';
import { defaultKeymap, history, historyKeymap } from '@codemirror/commands';
import { markdown } from '@codemirror/lang-markdown';
import { defaultHighlightStyle, syntaxHighlighting } from '@codemirror/language';
import type { Mode, Selection } from './client';

export function Editor({ section, text, mode, disabled, typewriter, focus, onChange, onSelection, jumpLine }: {
  section: string; text: string; mode: Mode; disabled: boolean; typewriter: boolean; focus: boolean;
  onChange(text: string): void; onSelection(selection: Selection): void; jumpLine?: number;
}) {
  const host = useRef<HTMLDivElement>(null);
  const editor = useRef<EditorView | null>(null);
  const lock = useRef(new Compartment());
  const language = useRef(new Compartment());
  const external = useRef(false);
  const latest = useRef({ onChange, onSelection, typewriter }); latest.current = { onChange, onSelection, typewriter };
  useEffect(() => {
    const view = new EditorView({ parent: host.current!, state: EditorState.create({ doc: text, extensions: [
      history(), drawSelection(), keymap.of([...defaultKeymap, ...historyKeymap]), EditorView.lineWrapping,
      language.current.of(mode === 'screenplay' ? [] : markdown()), syntaxHighlighting(defaultHighlightStyle),
      lock.current.of(EditorState.readOnly.of(disabled)),
      EditorView.contentAttributes.of({ role: 'textbox', 'aria-label': 'Document text', 'aria-multiline': 'true', spellcheck: 'true' }),
      EditorView.decorations.compute(['selection', 'doc'], state => Decoration.set([Decoration.line({ class: 'write-active-line' }).range(state.doc.lineAt(state.selection.main.head).from)])),
      EditorView.updateListener.of(update => {
        if (external.current) return;
        if (update.docChanged) latest.current.onChange(update.state.doc.toString());
        if (update.selectionSet || update.docChanged) {
          const s = update.state.selection.main;
          latest.current.onSelection({ section, from: s.from, to: s.to, text: update.state.sliceDoc(s.from, s.to).slice(0, 4000) });
          if (latest.current.typewriter) requestAnimationFrame(() => { if (editor.current === update.view) update.view.dispatch({ effects: EditorView.scrollIntoView(s.head, { y: 'center' }) }); });
        }
      }),
    ] }) });
    editor.current = view;
    return () => { editor.current = null; view.destroy(); };
    // A section owns an editor session; subsequent updates are transactions.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [section]);
  useEffect(() => {
    const view = editor.current; if (!view || view.state.doc.toString() === text) return;
    external.current = true;
    view.dispatch({ changes: { from: 0, to: view.state.doc.length, insert: text }, annotations: Transaction.addToHistory.of(false) });
    external.current = false;
  }, [text]);
  useEffect(() => { editor.current?.dispatch({ effects: lock.current.reconfigure(EditorState.readOnly.of(disabled)) }); }, [disabled]);
  useEffect(() => { editor.current?.dispatch({ effects: language.current.reconfigure(mode === 'screenplay' ? [] : markdown()) }); }, [mode]);
  useEffect(() => {
    const view = editor.current; if (!view || !jumpLine) return;
    const line = view.state.doc.line(Math.min(jumpLine, view.state.doc.lines));
    view.dispatch({ selection: { anchor: line.from }, effects: EditorView.scrollIntoView(line.from, { y: 'center' }) }); view.focus();
  }, [jumpLine]);
  return <div ref={host} className={`write-editor ${mode === 'screenplay' ? 'write-script-source' : ''} ${typewriter ? 'write-typewriter' : ''} ${focus ? 'write-focus' : ''}`} />;
}
