#!/usr/bin/env python3
"""Join docs/spec2/*.md into docs/SPEC.md and resolve {{old:X.Y}} references.

Used once for the scope cut of 7 Oct 2026. The map gives each section of the
previous SPEC.md its new number; None means the section was removed, and the
script lists every reference to one so it can be rewritten by hand.
"""
import re, sys, pathlib

ROOT = pathlib.Path(__file__).resolve().parents[2]
PARTS = ['front.md', 'ch01-02.md', 'ch03.md', 'ch04.md', 'ch05.md', 'ch06-07.md', 'ch08.md', 'ch09-10.md']

M = {}
for n in range(1, 8): M[f'1.{n}'] = f'1.{n}'
for n in range(1, 15): M[f'2.{n}'] = f'2.{n}'
for n in range(1, 13): M[f'3.{n}'] = f'3.{n}'
M.update({'3.13': '3.14', '3.14': '3.15', '3.15': '3.16', '3.16': '3.17', '3.17': '3.18', '3.18': '3.19'})
for n in range(1, 12): M[f'4.{n}'] = f'4.{n}'
M.update({'4.12': None, '4.13': '4.12', '4.14': None, '4.15': '4.14'})
for n in range(1, 15): M[f'5.{n}'] = f'5.{n}'
M.update({'5.15': None, '5.16': '5.15', '5.17': '5.16', '5.18': '5.17', '5.19': '5.18'})
for n in range(1, 12): M[f'6.{n}'] = f'6.{n}'
M.update({'6.12': None, '6.13': '6.12', '6.14': '6.13'})
for n in range(1, 22): M[f'7.{n}'] = None          # Unquantized: cut
for n in range(1, 13): M[f'8.{n}'] = f'7.{n}'
for n in range(1, 8): M[f'9.{n}'] = f'8.{n}'
M.update({'9.8': '8.9', '9.9': '8.10', '9.10': '8.11', '9.11': '8.12', '9.12': '8.13', '9.13': '8.14'})
# chapters 10 and 11 become 9 and 10; filled in from the last rewrite's report
EXTRA = ROOT / 'tools/spec/map_9_10.txt'
if EXTRA.exists():
    for line in EXTRA.read_text().split('\n'):
        if line.strip():
            old, new = line.split()
            M[old] = None if new == '-' else new

text = '\n\n'.join((ROOT / 'docs/spec2' / p).read_text().rstrip('\n') for p in PARTS) + '\n'
unresolved = []
def sub(m):
    old = m.group(1)
    if old not in M: unresolved.append(f'unknown {old}'); return m.group(0)
    if M[old] is None: unresolved.append(f'removed {old}: …{text[max(0,m.start()-80):m.end()+20]}…'.replace('\n', ' ')); return m.group(0)
    return M[old]
text = re.sub(r'\{\{old:(\d+\.\d+)\}\}', sub, text)
if '--check' in sys.argv:
    print('\n'.join(unresolved) or 'all references resolve'); sys.exit(1 if unresolved else 0)
(ROOT / 'docs/SPEC.md').write_text(text)
print(f'wrote docs/SPEC.md ({text.count(chr(10))} lines)'); print('\n'.join(unresolved) or 'all references resolve')
