// The readout's characters (docs/FOCUS.md): the 5 × 7 dot patterns a
// character LCD holds in its ROM, as the Mi-WWAV device's screen shows them.
// Each character is five columns; a column is seven dots, top first, as the
// bits of one byte (bit 0 is the top dot). The table runs from space to
// underscore, so the readout's text is capitals.

export const COLS = 5;
export const ROWS = 7;

// prettier-ignore
const ROM =
  '0000000000' + '00005F0000' + '0007000700' + '147F147F14' + '242A7F2A12' + '2313086462' + '3649552250' + '0005030000' +
  '001C224100' + '0041221C00' + '14083E0814' + '08083E0808' + '0050300000' + '0808080808' + '0060600000' + '2010080402' +
  '3E5149453E' + '00427F4000' + '4261514946' + '2141454B31' + '1814127F10' + '2745454539' + '3C4A494930' + '0171090503' +
  '3649494936' + '064949291E' + '0036360000' + '0056360000' + '0814224100' + '1414141414' + '0041221408' + '0201510906' +
  '324979413E' + '7E1111117E' + '7F49494936' + '3E41414122' + '7F4141221C' + '7F49494941' + '7F09090901' + '3E4149497A' +
  '7F0808087F' + '00417F4100' + '2040413F01' + '7F08142241' + '7F40404040' + '7F020C027F' + '7F0408107F' + '3E4141413E' +
  '7F09090906' + '3E4151215E' + '7F09192946' + '4649494931' + '01017F0101' + '3F4040403F' + '1F2040201F' + '3F4038403F' +
  '6314081463' + '0708700807' + '6151494543' + '007F414100' + '0204081020' + '0041417F00' + '0402010204' + '4040404040';

/** Marks the ROM has no place for, drawn the same way. */
const EXTRA: Record<string, string> = {
  '·': '0000080000',
  '•': '0000080000',
  '…': '4000400040',
  '♪': '20703F0204',
  '→': '08082A1C08',
  '×': '2214081422',
  '°': '0609090600',
};

/** Characters said with one the ROM has. */
const ALIAS: Record<string, string> = {
  '’': "'",
  '‘': "'",
  '“': '"',
  '”': '"',
  '–': '-',
  '—': '-',
  '‐': '-',
  '{': '(',
  '}': ')',
  '|': '!',
  '~': '-',
  '`': "'",
};

const columns = (hex: string): number[] => [0, 2, 4, 6, 8].map((i) => parseInt(hex.slice(i, i + 2), 16));

/** One character's five columns, or null when the table has no such character (a kanji, say). */
export function glyph(ch: string): number[] | null {
  const c = ALIAS[ch] ?? ch;
  if (EXTRA[c]) return columns(EXTRA[c]);
  const code = c.toUpperCase().charCodeAt(0);
  if (c.length !== 1 || code < 32 || code > 95) return null;
  return columns(ROM.slice((code - 32) * 10, (code - 32) * 10 + 10));
}

/** Whether the dot at `col`, `row` of a character is lit. */
export function lit(cols: readonly number[], col: number, row: number): boolean {
  return ((cols[col] >> row) & 1) === 1;
}
