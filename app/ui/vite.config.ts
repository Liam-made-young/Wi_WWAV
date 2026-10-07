import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';

// Tauri serves the built files; `npm run dev` serves them for the dev bridge
// and Playwright. No remote scripts load in the app (docs/SPEC.md 9.8).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { target: 'safari16', outDir: 'dist', sourcemap: true },
  test: { environment: 'jsdom', include: ['src/**/*.test.{ts,tsx}'] },
});
