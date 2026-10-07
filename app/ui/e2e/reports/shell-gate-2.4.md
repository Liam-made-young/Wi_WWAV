# Gate 2.4: the shell, measured

Written by `e2e/shell/gate.spec.ts` (Playwright, Chromium, against the real core through the dev bridge).
Bars (docs/GATES.md 2.4, docs/SPEC.md 3.18 and 8.9): body text 7:1, secondary text 4.5:1, buttons, tabs and
orbs 44 × 44 pt, dense rows and grid cells 24 × 24 pt, no text under 11 pt, and nothing moving while idle
(two screenshots 5 s apart, byte for byte). Contrast is the worst over every colour the grounds behind a
text can show. Fonts are Linux stand-ins for Lucida Grande, Menlo, Inter and Jost; sizes and colours are the
tokens’ own, so the ratios hold on a Mac, while widths there will differ slightly.

Run 2026-10-07.

| State | Window | Appearance | Texts | Lowest body | Lowest secondary | Smallest text | Targets | Tightest target | Still 5 s | Fails |
|---|---|---|---|---|---|---|---|---|---|---|
| Learn | 1024 × 680 | light | 10 | 8.86 | 5 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Space | 1024 × 680 | light | 8 | 8.55 | 5 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Console | 1024 × 680 | light | 8 | 9.03 | 5 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Now strip filled, paused | 1024 × 680 | light | 10 | 8.86 | 5 | 11 | 11 | 76 × 44 (needs 44) | yes | 0 |
| ⌘K with results | 1024 × 680 | light | 17 | 8.54 | 5 | 11 | 11 | 76 × 44 (needs 44) | — | 0 |
| ⌘⇧N after a capture | 1024 × 680 | light | 11 | 11.18 | 5 | 11 | 7 | 76 × 44 (needs 44) | — | 0 |
| ⌘L with a selection | 1024 × 680 | light | 35 | 8.54 | 5 | 11 | 38 | 76 × 44 (needs 44) | — | 0 |
| Get Info | 1024 × 680 | light | 78 | 8.54 | 5 | 11 | 47 | 76 × 44 (needs 44) | — | 0 |
| Player, expanded | 1024 × 680 | light | 11 | 11.18 | 5 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Galaxy chip menu | 1024 × 680 | light | 13 | 11.18 | 5 | 11 | 11 | 76 × 44 (needs 44) | — | 0 |
| Settings · Account | 1024 × 680 | light | 24 | 11.18 | 5 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Settings · Library | 1024 × 680 | light | 25 | 11.18 | 5 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Settings · Learn | 1024 × 680 | light | 23 | 11.18 | 5 | 11 | 18 | 76 × 44 (needs 44) | — | 0 |
| Settings · Audio & MIDI · Video | 1024 × 680 | light | 24 | 11.18 | 5 | 11 | 17 | 76 × 44 (needs 44) | — | 0 |
| Settings · Claude | 1024 × 680 | light | 37 | 11.18 | 5 | 11 | 14 | 76 × 44 (needs 44) | — | 0 |
| Settings · Privacy | 1024 × 680 | light | 18 | 11.18 | 5 | 11 | 14 | 76 × 44 (needs 44) | — | 0 |
| Settings · Appearance · Keyboard | 1024 × 680 | light | 76 | 11.18 | 5 | 11 | 18 | 76 × 44 (needs 44) | — | 0 |
| Export everything | 1024 × 680 | light | 13 | 11.18 | 5 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| Undo toast | 1024 × 680 | light | 12 | 8.86 | 5 | 11 | 6 | 76 × 44 (needs 44) | — | 0 |
| First launch 1: Sign in | 1024 × 680 | light | 12 | 11.18 | 5 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| First launch 2: Claim your galaxy | 1024 × 680 | light | 15 | 11.18 | 5 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| First launch 3: Import your folder | 1024 × 680 | light | 14 | 11.18 | 5 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| First launch 4: Add your calendars | 1024 × 680 | light | 16 | 11.18 | 5 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| First launch 5: Connect Claude | 1024 × 680 | light | 31 | 11.18 | 5 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| Learn | 1024 × 680 | dark | 10 | 8.3 | 4.83 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Space | 1024 × 680 | dark | 8 | 8.55 | 4.83 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Console | 1024 × 680 | dark | 8 | 9.03 | 4.83 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Now strip filled, paused | 1024 × 680 | dark | 10 | 8.3 | 4.83 | 11 | 11 | 76 × 44 (needs 44) | yes | 0 |
| ⌘K with results | 1024 × 680 | dark | 17 | 8.54 | 4.83 | 11 | 11 | 76 × 44 (needs 44) | — | 0 |
| ⌘⇧N after a capture | 1024 × 680 | dark | 11 | 9.64 | 4.83 | 11 | 7 | 76 × 44 (needs 44) | — | 0 |
| ⌘L with a selection | 1024 × 680 | dark | 35 | 8.3 | 4.83 | 11 | 38 | 76 × 44 (needs 44) | — | 0 |
| Get Info | 1024 × 680 | dark | 78 | 8.3 | 4.83 | 11 | 47 | 76 × 44 (needs 44) | — | 0 |
| Player, expanded | 1024 × 680 | dark | 11 | 9.64 | 4.83 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Galaxy chip menu | 1024 × 680 | dark | 13 | 9.64 | 4.83 | 11 | 11 | 76 × 44 (needs 44) | — | 0 |
| Settings · Account | 1024 × 680 | dark | 24 | 9.64 | 4.71 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Settings · Library | 1024 × 680 | dark | 25 | 9.64 | 4.71 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Settings · Learn | 1024 × 680 | dark | 23 | 9.64 | 4.71 | 11 | 18 | 76 × 44 (needs 44) | — | 0 |
| Settings · Audio & MIDI · Video | 1024 × 680 | dark | 24 | 9.64 | 4.71 | 11 | 17 | 76 × 44 (needs 44) | — | 0 |
| Settings · Claude | 1024 × 680 | dark | 37 | 9.64 | 4.71 | 11 | 14 | 76 × 44 (needs 44) | — | 0 |
| Settings · Privacy | 1024 × 680 | dark | 18 | 9.64 | 4.71 | 11 | 14 | 76 × 44 (needs 44) | — | 0 |
| Settings · Appearance · Keyboard | 1024 × 680 | dark | 76 | 9.64 | 4.71 | 11 | 18 | 76 × 44 (needs 44) | — | 0 |
| Export everything | 1024 × 680 | dark | 13 | 9.64 | 4.83 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| Undo toast | 1024 × 680 | dark | 12 | 8.3 | 4.83 | 11 | 6 | 76 × 44 (needs 44) | — | 0 |
| First launch 1: Sign in | 1024 × 680 | dark | 12 | 9.64 | 4.83 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| First launch 2: Claim your galaxy | 1024 × 680 | dark | 15 | 9.64 | 4.83 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| First launch 3: Import your folder | 1024 × 680 | dark | 14 | 9.64 | 4.83 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| First launch 4: Add your calendars | 1024 × 680 | dark | 16 | 9.64 | 4.83 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| First launch 5: Connect Claude | 1024 × 680 | dark | 31 | 9.64 | 4.83 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| Learn | 1280 × 800 | light | 12 | 8.86 | 5 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Space | 1280 × 800 | light | 10 | 8.55 | 5 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Console | 1280 × 800 | light | 10 | 9.03 | 5 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Now strip filled, paused | 1280 × 800 | light | 12 | 8.86 | 5 | 11 | 11 | 76 × 44 (needs 44) | yes | 0 |
| ⌘K with results | 1280 × 800 | light | 19 | 8.54 | 5 | 11 | 11 | 76 × 44 (needs 44) | — | 0 |
| ⌘⇧N after a capture | 1280 × 800 | light | 13 | 11.18 | 5 | 11 | 7 | 76 × 44 (needs 44) | — | 0 |
| ⌘L with a selection | 1280 × 800 | light | 37 | 8.54 | 5 | 11 | 38 | 76 × 44 (needs 44) | — | 0 |
| Get Info | 1280 × 800 | light | 80 | 8.54 | 5 | 11 | 47 | 76 × 44 (needs 44) | — | 0 |
| Player, expanded | 1280 × 800 | light | 13 | 11.18 | 5 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Galaxy chip menu | 1280 × 800 | light | 15 | 11.18 | 5 | 11 | 11 | 76 × 44 (needs 44) | — | 0 |
| Settings · Account | 1280 × 800 | light | 26 | 11.18 | 5 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Settings · Library | 1280 × 800 | light | 27 | 11.18 | 5 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Settings · Learn | 1280 × 800 | light | 25 | 11.18 | 5 | 11 | 18 | 76 × 44 (needs 44) | — | 0 |
| Settings · Audio & MIDI · Video | 1280 × 800 | light | 26 | 11.18 | 5 | 11 | 17 | 76 × 44 (needs 44) | — | 0 |
| Settings · Claude | 1280 × 800 | light | 39 | 11.18 | 5 | 11 | 14 | 76 × 44 (needs 44) | — | 0 |
| Settings · Privacy | 1280 × 800 | light | 20 | 11.18 | 5 | 11 | 14 | 76 × 44 (needs 44) | — | 0 |
| Settings · Appearance · Keyboard | 1280 × 800 | light | 78 | 11.18 | 5 | 11 | 18 | 76 × 44 (needs 44) | — | 0 |
| Export everything | 1280 × 800 | light | 15 | 11.18 | 5 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| Undo toast | 1280 × 800 | light | 14 | 8.86 | 5 | 11 | 6 | 76 × 44 (needs 44) | — | 0 |
| First launch 1: Sign in | 1280 × 800 | light | 15 | 11.18 | 5 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| First launch 2: Claim your galaxy | 1280 × 800 | light | 17 | 11.18 | 5 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| First launch 3: Import your folder | 1280 × 800 | light | 16 | 11.18 | 5 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| First launch 4: Add your calendars | 1280 × 800 | light | 18 | 11.18 | 5 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| First launch 5: Connect Claude | 1280 × 800 | light | 33 | 11.18 | 5 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| Learn | 1280 × 800 | dark | 12 | 8.3 | 4.83 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Space | 1280 × 800 | dark | 10 | 8.55 | 4.83 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Console | 1280 × 800 | dark | 10 | 9.03 | 4.83 | 11 | 6 | 76 × 44 (needs 44) | yes | 0 |
| Now strip filled, paused | 1280 × 800 | dark | 12 | 8.3 | 4.83 | 11 | 11 | 76 × 44 (needs 44) | yes | 0 |
| ⌘K with results | 1280 × 800 | dark | 19 | 8.54 | 4.83 | 11 | 11 | 76 × 44 (needs 44) | — | 0 |
| ⌘⇧N after a capture | 1280 × 800 | dark | 13 | 9.64 | 4.83 | 11 | 7 | 76 × 44 (needs 44) | — | 0 |
| ⌘L with a selection | 1280 × 800 | dark | 37 | 8.3 | 4.83 | 11 | 38 | 76 × 44 (needs 44) | — | 0 |
| Get Info | 1280 × 800 | dark | 80 | 8.3 | 4.83 | 11 | 47 | 76 × 44 (needs 44) | — | 0 |
| Player, expanded | 1280 × 800 | dark | 13 | 9.64 | 4.83 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Galaxy chip menu | 1280 × 800 | dark | 15 | 9.64 | 4.83 | 11 | 11 | 76 × 44 (needs 44) | — | 0 |
| Settings · Account | 1280 × 800 | dark | 26 | 9.64 | 4.71 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Settings · Library | 1280 × 800 | dark | 27 | 9.64 | 4.71 | 11 | 16 | 76 × 44 (needs 44) | — | 0 |
| Settings · Learn | 1280 × 800 | dark | 25 | 9.64 | 4.71 | 11 | 18 | 76 × 44 (needs 44) | — | 0 |
| Settings · Audio & MIDI · Video | 1280 × 800 | dark | 26 | 9.64 | 4.71 | 11 | 17 | 76 × 44 (needs 44) | — | 0 |
| Settings · Claude | 1280 × 800 | dark | 39 | 9.64 | 4.71 | 11 | 14 | 76 × 44 (needs 44) | — | 0 |
| Settings · Privacy | 1280 × 800 | dark | 20 | 9.64 | 4.71 | 11 | 14 | 76 × 44 (needs 44) | — | 0 |
| Settings · Appearance · Keyboard | 1280 × 800 | dark | 78 | 9.64 | 4.71 | 11 | 18 | 76 × 44 (needs 44) | — | 0 |
| Export everything | 1280 × 800 | dark | 15 | 9.64 | 4.83 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| Undo toast | 1280 × 800 | dark | 14 | 8.3 | 4.83 | 11 | 6 | 76 × 44 (needs 44) | — | 0 |
| First launch 1: Sign in | 1280 × 800 | dark | 15 | 9.64 | 4.83 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| First launch 2: Claim your galaxy | 1280 × 800 | dark | 17 | 9.64 | 4.83 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |
| First launch 3: Import your folder | 1280 × 800 | dark | 16 | 9.64 | 4.83 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| First launch 4: Add your calendars | 1280 × 800 | dark | 18 | 9.64 | 4.83 | 11 | 9 | 76 × 44 (needs 44) | — | 0 |
| First launch 5: Connect Claude | 1280 × 800 | dark | 33 | 9.64 | 4.83 | 11 | 8 | 76 × 44 (needs 44) | — | 0 |

## Fails

None.
