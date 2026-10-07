// The update manifest (docs/SPEC.md 9.9): Tauri's updater reads
// mi-wwav.com/desktop/latest.json. Each platform's signature is the
// minisign (Ed25519) signature of its bundle; the mock's are empty unless a
// test sets state.latest (or PUT /__mock/latest) with real ones.
import { json } from '../http.js';

export function defaultManifest(base) {
  const file = (name) => ({ signature: '', url: `${base}/desktop/files/${name}` });
  return {
    version: '0.1.0',
    notes: 'The first build.',
    pub_date: '2026-10-07T00:00:00Z',
    platforms: {
      'darwin-aarch64': file('Wi_WWAV_0.1.0_aarch64.app.tar.gz'),
      'darwin-x86_64': file('Wi_WWAV_0.1.0_x64.app.tar.gz'),
      'linux-x86_64': file('Wi_WWAV_0.1.0_amd64.AppImage.tar.gz'),
    },
  };
}

function latest(ctx) {
  return json(200, ctx.state.latest ?? defaultManifest(ctx.state.base));
}

export const routes = [['GET', '/desktop/latest.json', latest]];
