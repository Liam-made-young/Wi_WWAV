/// <reference types="node" />
import { build } from 'vite';
import { describe, expect, it } from 'vitest';

// The fake core is for `npm run dev` only (docs/HEAT.md): a production build
// must hold none of it. What a fail looks like: its code, its seeded sample
// or its installer anywhere in the files `vite build` writes, or a lazy chunk
// for it sitting in dist even if nothing loads it.

interface Chunk {
  type: string;
  fileName: string;
  code?: string;
  moduleIds?: string[];
}

async function bundle(env: string | undefined): Promise<Chunk[]> {
  const was = process.env.VITE_FAKE_CORE;
  if (env === undefined) delete process.env.VITE_FAKE_CORE;
  else process.env.VITE_FAKE_CORE = env;
  try {
    const out = await build({
      logLevel: 'silent',
      configFile: 'vite.config.ts',
      build: { write: false, sourcemap: false },
    });
    return (Array.isArray(out) ? out : [out as never]).flatMap((o: { output: Chunk[] }) => o.output);
  } finally {
    if (was === undefined) delete process.env.VITE_FAKE_CORE;
    else process.env.VITE_FAKE_CORE = was;
  }
}

const holdsFake = (files: Chunk[]) =>
  files.some(
    (f) =>
      f.type === 'chunk' &&
      ((f.moduleIds ?? []).some((m) => m.includes('/heat/fake/')) ||
        /installFakeCore|The fake core has no|__wiFake/.test(f.code ?? '')),
  );

describe('the production build', () => {
  it('holds none of the fake core, and no lazy chunk for it', async () => {
    const files = await bundle(undefined);
    expect(files.filter((f) => f.type === 'chunk').length).toBeGreaterThan(0);
    expect(holdsFake(files)).toBe(false);
    expect(files.filter((f) => f.type === 'chunk' && f.fileName.includes('install'))).toEqual([]);
  }, 60_000);

  it('does load it, behind a dynamic import, when VITE_FAKE_CORE=1', async () => {
    const files = await bundle('1');
    expect(holdsFake(files)).toBe(true);
    // A lazy chunk: the main bundle only names it.
    const entry = files.find((f) => f.type === 'chunk' && (f as { isEntry?: boolean }).isEntry)!;
    expect(entry.code).toMatch(/import\(["'`]\.\/install-/);
  }, 60_000);
});
