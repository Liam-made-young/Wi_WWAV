// `node --test tools/mock-server` runs this package's main, which loads
// every test file here, so a new file is picked up without a list to keep.
import { readdirSync } from 'node:fs';

const here = new URL('.', import.meta.url);
for (const name of readdirSync(here).sort()) {
  if (name.endsWith('.test.js')) await import(new URL(name, here));
}
