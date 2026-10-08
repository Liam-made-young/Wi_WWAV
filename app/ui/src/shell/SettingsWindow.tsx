// The settings window (docs/SPEC.md 2.13): app/src-tauri opens it on ⌘, as a
// window of its own that loads index.html?window=settings, and it shows the
// same Settings as the sheet in the main window, filling the window.
//
// Its capability lists only the commands Settings uses (app.hello,
// app.settings.*, account.*, engine.*, library.cleanup.*,
// library.trash.empty), and only the main window listens to the core's
// events: so it reads what it shows when it opens, and takes each change
// from the answer to the command that made it.

import { useFocusRoot } from '../focus/hooks';
import { useEffect, useState } from 'react';
import { call } from '../bridge';
import { useAppearance } from './hooks';
import { type Account, type Pane, Settings, type SettingsValue } from './Settings';

export function SettingsWindow() {
  const [library, setLibrary] = useState('');
  const [settings, setSettings] = useState<SettingsValue | null>(null);
  const [account, setAccount] = useState<Account>({ signedIn: false });
  const [pane, setPane] = useState<Pane>('account');
  useAppearance(settings);
  useFocusRoot();

  useEffect(() => {
    document.title = 'Settings';
    call<{ library: string }>('app.hello').then(
      (h) => setLibrary(h.library),
      () => {},
    );
    call<SettingsValue>('app.settings.get').then(setSettings, () => {});
    call<Account>('account.status').then(setAccount, () => {});
  }, []);

  return (
    <div className="settings-window register-desk">
      <Settings
        shown
        pane={pane}
        settings={settings}
        account={account}
        library={library}
        onPane={setPane}
        onPatch={(patch) => void call<SettingsValue>('app.settings.set', { patch }).then(setSettings, () => {})}
        onSignIn={() => void call<Account>('account.signIn').then((a) => setAccount((was) => ({ ...was, ...a })), () => {})}
        onSignOut={() => void call<Account>('account.signOut').then(setAccount, () => {})}
      />
    </div>
  );
}
