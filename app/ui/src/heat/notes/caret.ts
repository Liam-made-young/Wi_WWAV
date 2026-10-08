// Where the caret is in a textarea, in px from the field's top left, so a
// list can open under it. A textarea won't say, so the text before the caret
// is laid out again in a hidden copy of the field and a mark at its end is
// measured.

const COPIED = [
  'boxSizing', 'width', 'paddingTop', 'paddingRight', 'paddingBottom', 'paddingLeft',
  'borderTopWidth', 'borderRightWidth', 'borderBottomWidth', 'borderLeftWidth',
  'fontFamily', 'fontSize', 'fontWeight', 'fontStyle', 'letterSpacing', 'lineHeight', 'tabSize', 'textIndent',
] as const;

export function caretPoint(field: HTMLTextAreaElement, at: number): { left: number; top: number } {
  const copy = document.createElement('div');
  const style = getComputedStyle(field);
  for (const p of COPIED) copy.style[p] = style[p];
  Object.assign(copy.style, { position: 'absolute', visibility: 'hidden', whiteSpace: 'pre-wrap', overflowWrap: 'break-word', top: '0', left: '-9999px' });
  copy.textContent = field.value.slice(0, at);
  const mark = document.createElement('span');
  mark.textContent = '​';
  copy.append(mark);
  document.body.append(copy);
  const point = { left: mark.offsetLeft, top: mark.offsetTop + mark.offsetHeight - field.scrollTop };
  copy.remove();
  return point;
}
