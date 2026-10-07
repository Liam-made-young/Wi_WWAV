// Wi's JavaScript readers on files, for tools/parity/check.py (F3): one
// JSON line per file, {"file": ..., "verdict": ...}.
//
//   node wi_verdicts.mjs <formats/wi/src/formats> FILE...
//
// A .wwav or .wav gets readWwav's verdict, which is already the reference
// tool's sentence. A .mp4 or .swav gets readSwav's result in swav_pack.py's
// words, by the mapping wi/test/formats.swav.test.js uses for the same
// comparison. A file the reader refuses gets "error: " and its message.
import { openAsBlob } from 'node:fs';
import { join } from 'node:path';
import { pathToFileURL } from 'node:url';

const [formats, ...files] = process.argv.slice(2);
const { readWwav } = await import(pathToFileURL(join(formats, 'wwav.js')).href);
const { readSwav } = await import(pathToFileURL(join(formats, 'swav.js')).href);

const swavWords = (i) =>
  !(i.wmet && i.wlin)
    ? 'a plain MP4: no wmet and wlin'
    : i.isSwav
      ? 'a .swav: the film, its wmet and wlin'
      : i.version === null
        ? 'the film only: wmet has no version'
        : `the film only: version ${i.version} is newer than this reader (0.x)`;

for (const file of files) {
  let verdict;
  try {
    const blob = await openAsBlob(file);
    verdict = /\.(mp4|m4v|mov|swav)$/i.test(file) ? swavWords(await readSwav(blob)) : (await readWwav(blob)).verdict;
  } catch (e) {
    verdict = `error: ${e.message}`;
  }
  console.log(JSON.stringify({ file, verdict }));
}
