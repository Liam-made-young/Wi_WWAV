import { StrictMode } from 'react';
import { createRoot } from 'react-dom/client';
import './styles/tokens.css';
import './shell/shell.css';
import { Shell } from './shell/Shell';
import { SettingsWindow } from './shell/SettingsWindow';

// The app's settings window loads this same page as index.html?window=settings
// (app/src-tauri/tauri.conf.json); every other window is the shell.
const settings = new URLSearchParams(window.location.search).get('window') === 'settings';

createRoot(document.getElementById('root')!).render(<StrictMode>{settings ? <SettingsWindow /> : <Shell />}</StrictMode>);
