/// <reference types="node" />
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Tauri serves the built files; `npm run dev` serves them for the dev bridge
// and Playwright. No remote scripts load in the app (docs/SPEC.md 9.8).
export default defineConfig({
  plugins: [react()],
  // VITE_FAKE_CORE=1 npm run dev runs Heat on its fake core with a seeded
  // sample. The switch is a compile-time constant, so a build without it
  // leaves the fake out of the bundle altogether (src/main.tsx).
  define: { __FAKE_CORE__: JSON.stringify(process.env.VITE_FAKE_CORE === '1') },
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: 'safari16', outDir: 'dist', sourcemap: true },
  test: { environment: 'jsdom', include: ['src/**/*.test.{ts,tsx}'] },
});
