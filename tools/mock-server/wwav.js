// Just enough of .wwav (docs/SPEC.md 6.1, 6.8) for the mock: a writer for
// fixtures and tests, and a reader for the identity a published file
// carries. The server reads a song's id and lineage from the file itself
// (4.13), so the mock does too. (server.md mismatch 9: today the browser
// reads the file and the server trusts the JSON it is sent.)
import { isObject } from './http.js';

const RATE = 44100;
const ALIGN = 512;

function chunk(id, payload) {
  const head = Buffer.alloc(8);
  head.write(id, 0, 'latin1');
  head.writeUInt32LE(payload.length, 4);
  return Buffer.concat([head, payload, Buffer.alloc(payload.length & 1)]);
}

// JSON with ", " and ": " between items, as wwav_pack.py writes it.
function spaced(entries) {
  return Buffer.from(`{${entries.map(([k, v]) => `${JSON.stringify(k)}: ${JSON.stringify(v)}`).join(', ')}}`);
}

// A small, valid song: a silent master and four silent stems. `version`
// writes wmet 0.2's key; `plain` writes a plain WAV with no WWAV chunks.
export function writeWwav({
  songId,
  title = 'Untitled',
  artist = '',
  bpm,
  key,
  frames = 4,
  type = 'original',
  created = '2026-10-07',
  version,
  parentId = null,
  rootId = songId,
  generation = 0,
  creator = '',
  plain = false,
}) {
  const fmt = Buffer.alloc(16);
  fmt.writeUInt16LE(1, 0); // PCM
  fmt.writeUInt16LE(2, 2);
  fmt.writeUInt32LE(RATE, 4);
  fmt.writeUInt32LE(RATE * 4, 8);
  fmt.writeUInt16LE(4, 12);
  fmt.writeUInt16LE(16, 14);
  const parts = [chunk('fmt ', fmt), chunk('data', Buffer.alloc(frames * 4))];
  if (!plain) {
    const meta = [
      ['wwav', version === undefined ? '0.1' : '0.2'],
      ['song_id', songId],
      ['title', title],
      ['artist', artist],
    ];
    if (bpm !== undefined) meta.push(['bpm', bpm]);
    if (key !== undefined) meta.push(['key', key]);
    meta.push(['frames', frames], ['type', type], ['created', created]);
    if (version !== undefined) meta.push(['version', version]);
    parts.push(chunk('wmet', spaced(meta)));

    // The stems start on a 512-byte boundary of the file.
    const at = 12 + parts.reduce((n, p) => n + p.length, 0) + 8 + 16;
    const pad = (ALIGN - (at % ALIGN)) % ALIGN;
    const head = Buffer.alloc(16);
    head.writeUInt16LE(1, 0);
    head.writeUInt8(4, 2);
    head.writeUInt8(2, 3);
    head.writeUInt8(16, 4);
    head.writeUInt16LE(pad, 6);
    head.writeUInt32LE(RATE, 8);
    head.writeUInt32LE(frames, 12);
    parts.push(chunk('wstm', Buffer.concat([head, Buffer.alloc(pad + frames * 16)])));
    parts.push(
      chunk(
        'wlin',
        spaced([
          ['parent_id', parentId],
          ['root_id', rootId],
          ['generation', generation],
          ['creator', creator],
          ['device_id', ''],
        ]),
      ),
    );
  }
  const body = Buffer.concat(parts);
  const riff = Buffer.alloc(12);
  riff.write('RIFF', 0, 'latin1');
  riff.writeUInt32LE(4 + body.length, 4);
  riff.write('WAVE', 8, 'latin1');
  return Buffer.concat([riff, body]);
}

function parseJson(payload) {
  if (!payload) return null;
  try {
    return JSON.parse(payload.toString('utf8'));
  } catch {
    return null;
  }
}

// The identity in a .wwav's wmet and wlin, or null for anything else (a
// plain WAV, an MP3, a file whose wmet isn't readable). The first chunk of
// each id counts, as in wwav_pack.py.
export function readWwav(buffer) {
  if (buffer.length < 12 || buffer.toString('latin1', 0, 4) !== 'RIFF' || buffer.toString('latin1', 8, 12) !== 'WAVE') {
    return null;
  }
  const first = {};
  for (let at = 12; at + 8 <= buffer.length; ) {
    const id = buffer.toString('latin1', at, at + 4);
    const size = buffer.readUInt32LE(at + 4);
    if (!(id in first)) first[id] = buffer.subarray(at + 8, Math.min(at + 8 + size, buffer.length));
    at += 8 + size + (size & 1);
  }
  const wmet = parseJson(first.wmet);
  if (!isObject(wmet) || !/^[0-9a-f]{32}$/.test(wmet.song_id)) return null;
  const wlin = isObject(parseJson(first.wlin)) ? parseJson(first.wlin) : {};
  return {
    songId: wmet.song_id,
    // wmet 0.2's version; absent means 1 (6.8).
    version: Number.isInteger(wmet.version) && wmet.version >= 1 ? wmet.version : 1,
    title: typeof wmet.title === 'string' ? wmet.title : '',
    artist: typeof wmet.artist === 'string' ? wmet.artist : '',
    bpm: typeof wmet.bpm === 'number' ? wmet.bpm : null,
    key: typeof wmet.key === 'string' ? wmet.key : null,
    type: wmet.type,
    parentId: typeof wlin.parent_id === 'string' ? wlin.parent_id : null,
    rootId: typeof wlin.root_id === 'string' ? wlin.root_id : wmet.song_id,
    generation: Number.isInteger(wlin.generation) ? wlin.generation : 0,
    creator: typeof wlin.creator === 'string' ? wlin.creator : '',
  };
}
