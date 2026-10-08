// The readout (docs/FOCUS.md): the one ornament in the app, and the link
// between Wi-WWAV and the Mi-WWAV device. It is the device's character LCD
// drawn dot by dot on a canvas: a dark field, a row of 5 × 8 cells a dot
// apart, every unlit dot faintly there, and the lit ones spelling the Now
// task and when it is due, the next commitment, and what is playing, or ALL
// CLEAR. The words are focus/words.ts's; VoiceOver reads them as a
// sentence, not as dots.

import { useEffect, useRef } from 'react';
import { COLS, glyph, lit, ROWS } from './dots';
import { layoutCells, type ReadoutText } from './words';

interface Props {
  text: ReadoutText;
}

/** A cell is the character's five columns and the gap after it; its eighth row is the cursor's, never lit here. */
const CELL = COLS + 1;
const CELL_ROWS = ROWS + 1;

const px = (style: CSSStyleDeclaration, name: string, otherwise: number) => {
  const n = parseFloat(style.getPropertyValue(name));
  return Number.isFinite(n) && n > 0 ? n : otherwise;
};

/**
 * A character the ROM lacks (a kanji, a kana) is set in the system's face,
 * small, and read back as dots: two cells wide, the same eight rows.
 */
const WIDE = COLS * 2 + 1;
const sampled = new Map<string, boolean[]>();
function sample(ch: string, family: string): boolean[] | null {
  const hit = sampled.get(ch);
  if (hit) return hit;
  const scale = 4;
  const canvas = document.createElement('canvas');
  canvas.width = WIDE * scale;
  canvas.height = CELL_ROWS * scale;
  const ctx = canvas.getContext('2d', { willReadFrequently: true });
  if (!ctx) return null;
  ctx.font = `${CELL_ROWS * scale}px ${family}`;
  ctx.textBaseline = 'middle';
  ctx.textAlign = 'center';
  ctx.fillText(ch, canvas.width / 2, canvas.height / 2 + scale / 2, canvas.width);
  const data = ctx.getImageData(0, 0, canvas.width, canvas.height).data;
  const dots: boolean[] = [];
  for (let row = 0; row < CELL_ROWS; row++) {
    for (let col = 0; col < WIDE; col++) {
      let sum = 0;
      for (let y = 0; y < scale; y++) {
        for (let x = 0; x < scale; x++) sum += data[((row * scale + y) * canvas.width + col * scale + x) * 4 + 3];
      }
      dots.push(sum / (scale * scale) > 96);
    }
  }
  sampled.set(ch, dots);
  return dots;
}

function draw(canvas: HTMLCanvasElement, text: ReadoutText) {
  const style = getComputedStyle(canvas);
  const pitch = px(style, '--prism-readout-pitch', 3);
  const dot = px(style, '--prism-readout-dot', 2);
  const width = canvas.clientWidth;
  const height = canvas.clientHeight;
  if (width === 0 || height === 0) return;
  const ratio = window.devicePixelRatio || 1;
  canvas.width = Math.round(width * ratio);
  canvas.height = Math.round(height * ratio);
  const ctx = canvas.getContext('2d');
  if (!ctx) return;
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
  ctx.clearRect(0, 0, width, height);

  const unlit = style.getPropertyValue('--prism-readout-unlit').trim();
  const on = style.getPropertyValue('--prism-readout-lit').trim();
  const family = style.getPropertyValue('--prism-type-mono').trim() || 'monospace';

  const pad = 2;
  const cells = Math.max(0, Math.floor((Math.floor(width / pitch) - pad * 2 + 1) / CELL));
  const left = Math.round((width - (cells * CELL - 1) * pitch) / 2);
  const top = Math.round((height - (CELL_ROWS * pitch - (pitch - dot))) / 2);
  const line = layoutCells(text, cells);

  const cell = (i: number, col: number, row: number, isLit: boolean) => {
    ctx.fillStyle = isLit ? on : unlit;
    ctx.fillRect(left + (i * CELL + col) * pitch, top + row * pitch, dot, dot);
  };

  const chars = [...line];
  for (let i = 0, at = 0; at < cells; i++) {
    const ch = chars[i] ?? ' ';
    const cols = glyph(ch);
    if (cols) {
      for (let row = 0; row < CELL_ROWS; row++) {
        for (let col = 0; col < COLS; col++) cell(at, col, row, row < ROWS && lit(cols, col, row));
      }
      at += 1;
      continue;
    }
    // Two cells, and the gap between them, for a character read from the system's face.
    const wide = at + 1 < cells ? sample(ch, family) : null;
    if (!wide) {
      for (let row = 0; row < CELL_ROWS; row++) for (let col = 0; col < COLS; col++) cell(at, col, row, false);
      at += 1;
      continue;
    }
    for (let row = 0; row < CELL_ROWS; row++) {
      for (let col = 0; col < WIDE; col++) {
        const isLit = wide[row * WIDE + col];
        // The gap column shows only where the character crosses it.
        if (col === COLS && !isLit) continue;
        cell(at, col, row, isLit);
      }
    }
    at += 2;
  }
}

export function Readout({ text }: Props) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const latest = useRef(text);
  latest.current = text;

  useEffect(() => {
    const el = canvas.current;
    if (!el) return;
    const redraw = () => draw(el, latest.current);
    redraw();
    // The dots are sized in CSS pixels, so the field is redrawn when it is resized.
    const seen = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(redraw);
    seen?.observe(el);
    return () => seen?.disconnect();
  }, []);

  useEffect(() => {
    if (canvas.current) draw(canvas.current, text);
  }, [text.left, text.right.join('|')]); // eslint-disable-line react-hooks/exhaustive-deps

  return (
    <div className="readout" role="img" aria-label={`Readout. ${text.label}`} data-tauri-drag-region>
      <canvas ref={canvas} className="readout-dots" aria-hidden="true" data-tauri-drag-region />
    </div>
  );
}
