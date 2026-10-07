// tools/mock-server, plus a side door that reports how many times each
// part of each multipart upload was sent, and the works published, without
// file bytes: the mock's own /__mock/state turns every byte of a 251 MB
// file into JSON (Buffer.toJSON runs before its replacer) and runs out of
// memory.
import http from 'node:http';
import { startMockServer } from '../../../../tools/mock-server/server.js';

const { url, state } = await startMockServer({ port: 0 });
const side = http.createServer((req, res) => {
  const uploads = [...state.multipart.values()].map((u) => ({
    uploadId: u.uploadId,
    trackId: u.trackId,
    total: u.total,
    sends: Object.fromEntries([...u.parts].map(([n, p]) => [n, p.sends])),
  }));
  const tracks = state.tracks.map((t) => ({ trackId: t.trackId, settings: t.settings, versions: t.versions.length }));
  res.setHeader('content-type', 'application/json');
  res.end(JSON.stringify({ uploads, tracks }));
});
side.listen(0, '127.0.0.1', () => {
  console.log(`mock mi-wwav.com on ${url}`);
  console.log(`counts on http://127.0.0.1:${side.address().port}`);
});
