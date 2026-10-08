import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import { HeatProvider } from './heat/store';
import './styles/tokens.css';
import './shell/shell.css';
import './focus/prism.css';
import './focus/focus.css';
import { Shell } from './shell/Shell';
import { SettingsWindow } from './shell/SettingsWindow';

// The app's settings window loads this same page as index.html?window=settings
// (app/src-tauri/tauri.conf.json); every other window is the shell.
const settings = new URLSearchParams(window.location.search).get('window') === 'settings';

// `npm run dev` with VITE_FAKE_CORE=1 runs the whole UI on Heat's fake core
// with a seeded sample. __FAKE_CORE__ is a compile-time constant, so a
// production build drops the branch and the fake with it (vite.config.ts).
async function start() {
  if (__FAKE_CORE__) {
    const { installFakeCore } = await import('./heat/fake/install');
    installFakeCore();
  }
  createRoot(document.getElementById('root')!).render(
    <StrictMode>
      <HeatProvider>{settings ? <SettingsWindow /> : <Shell />}</HeatProvider>
    </StrictMode>,
  );
}

void start();
