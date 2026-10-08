import { describe, expect, it } from 'vitest';
import { continueList, indentItem } from './Editor';
import { boxesIn, canon, excerptOf, firstLines, joinBlocks, linksIn, splitBlocks, tagsIn, tailOf } from './text';

// The rules a note's text is read by (docs/NOTES.md). What a fail looks
// like: a note that comes back from the editor with its blank lines moved;
// a fence or a list cut in two; a checkbox's line counted wrong; a link, a
// tag or a checkbox found inside code.

describe('blocks', () => {
  const note = '\n# Title\n\nOne line\nand another.\n\n\n```\ncode\n\nmore code\n```\n\n- a\n- b\n';

  it('splits on blank lines, with a fenced block whole', () => {
    expect(splitBlocks(note).map((b) => b.text)).toEqual(['# Title', 'One line\nand another.', '```\ncode\n\nmore code\n```', '- a\n- b']);
    expect(splitBlocks(note).map((b) => b.line)).toEqual([1, 3, 7, 13]);
  });

  it('joins back to the same text, blank lines and all', () => {
    expect(joinBlocks(splitBlocks(note), tailOf(note))).toBe(note);
    expect(canon(note)).toBe(note);
    expect(canon('')).toBe('');
    expect(canon('one')).toBe('one');
  });

  it('knows the line each block starts on after an edit', () => {
    const blocks = splitBlocks(note);
    expect(firstLines(blocks)).toEqual([1, 3, 7, 13]);
    blocks[1] = { ...blocks[1], text: 'One line' };
    expect(firstLines(blocks)).toEqual([1, 3, 6, 12]);
    // A block with nothing in it takes no line.
    expect(firstLines([{ text: 'a', gap: 0 }, { text: '', gap: 1 }, { text: 'b', gap: 1 }])).toEqual([0, 2, 2]);
    expect(joinBlocks([{ text: 'a', gap: 0 }, { text: '', gap: 1 }, { text: 'b\n', gap: 1 }])).toBe('a\n\nb');
  });

  it('keeps a fence nothing closed as one block to the end', () => {
    expect(splitBlocks('a\n\n```\nb\n\nc').map((b) => b.text)).toEqual(['a', '```\nb\n\nc']);
  });
});

describe('links, tags and checkboxes', () => {
  it('finds links outside code, by the note they name', () => {
    expect(linksIn('[[Verb groups]] and `[[not]]`\n[[JPN 201|the class]] [[Keigo#Humble]]')).toEqual([
      { target: 'Verb groups', shown: null, line: 0 },
      { target: 'JPN 201', shown: 'the class', line: 1 },
      { target: 'Keigo', shown: null, line: 1 },
    ]);
  });

  it('finds tags, but not in code, a heading, a link’s heading or an address', () => {
    expect(tagsIn('# Heading\n#grammar `#code` [[Keigo#Humble]] https://example.com/a#part\n```\n#fenced\n```\n(#te-form)')).toEqual([
      'grammar',
      'te-form',
    ]);
  });

  it('finds checkboxes with their line, their words and the task they became', () => {
    expect(boxesIn('Text\n- [ ] Do worksheet 4 ^t-01JABC\n    - [x] Read 3.2\n```\n- [ ] not one\n```\n- plain')).toEqual([
      { line: 1, text: 'Do worksheet 4', done: false, taskId: '01JABC' },
      { line: 2, text: 'Read 3.2', done: true, taskId: null },
    ]);
  });

  it('gives a note’s first words as its excerpt', () => {
    expect(excerptOf('# Title\n\n![](a.png)\n\n- [ ] Do **this** with [[Verb groups|the verbs]] ^t-1')).toBe('Title Do this with the verbs');
  });
});

describe('Return and Tab in a list', () => {
  it('goes on with a bullet, a number and a checkbox', () => {
    expect(continueList('- a', 3, 3)).toEqual({ value: '- a\n- ', pos: 6 });
    expect(continueList('1. one', 6, 6)).toEqual({ value: '1. one\n2. ', pos: 10 });
    expect(continueList('- [x] done', 10, 10)).toEqual({ value: '- [x] done\n- [ ] ', pos: 17 });
    expect(continueList('    - nested', 12, 12)).toEqual({ value: '    - nested\n    - ', pos: 19 });
    // In the middle of an item, the rest goes to the new one.
    expect(continueList('- ab', 3, 3)).toEqual({ value: '- a\n- b', pos: 6 });
  });

  it('ends the list on an empty item', () => {
    expect(continueList('- a\n- ', 6, 6)).toEqual({ value: '- a\n\n', pos: 5 });
    expect(continueList('- ', 2, 2)).toEqual({ value: '', pos: 0 });
    expect(continueList('- a\n- [ ] \n- c', 10, 10)).toEqual({ value: '- a\n\n- c', pos: 4 });
  });

  it('leaves Return alone outside a list', () => {
    expect(continueList('plain', 5, 5)).toBeNull();
    expect(continueList('- a', 1, 1)).toBeNull();
  });

  it('moves an item in and out', () => {
    expect(indentItem('- a\n- b', 7, false)).toEqual({ value: '- a\n    - b', pos: 11 });
    expect(indentItem('- a\n    - b', 11, true)).toEqual({ value: '- a\n- b', pos: 7 });
    expect(indentItem('- a', 3, true)).toEqual({ value: '- a', pos: 3 });
    expect(indentItem('plain', 2, false)).toBeNull();
  });
});
