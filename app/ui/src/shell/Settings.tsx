// Settings (docs/SPEC.md 2.13): a classic preferences window with an icon
// toolbar of seven panes. Settings apply at once and are not undo steps.
// What this build can't do yet is still shown, and says why in one sentence
// (8.10), so nothing is hidden and nothing pretends.

import { type ReactNode, useEffect, useState } from 'react';
import { call } from '../bridge';
import { ClaudePane } from './ClaudePane';
import { SHORTCUTS } from './keys';
import { MailAccounts } from './MailAccounts';
import { NotYet } from './NotYet';
import { SchoolSheet } from './SchoolSheet';
import { keys } from './platform';

export interface SettingsValue {
  appearance: 'light' | 'dark' | 'system';
  textSize: number;
  reduceMotion: boolean;
  library: { watchedFolders: string[]; leaveInPlace: boolean };
  audio: { device: string | null; buffer: number; pluginFolders: string[] };
  video: { hardwareEncode: boolean; proxyMedia: boolean };
  heat: { timeZone: string | null; school: string | null };
}

export const PANES = [
  { id: 'account', label: 'Account' },
  { id: 'library', label: 'Library' },
  { id: 'heat', label: 'Learn' },
  { id: 'audio', label: 'Audio & MIDI · Video' },
  { id: 'claude', label: 'Claude' },
  { id: 'privacy', label: 'Privacy' },
  { id: 'appearance', label: 'Appearance · Keyboard' },
] as const;

export type Pane = (typeof PANES)[number]['id'];

export interface Account {
  signedIn: boolean;
  username?: string;
  galaxy?: string;
}

interface Props {
  shown: boolean;
  pane: Pane;
  settings: SettingsValue | null;
  account: Account;
  library: string;
  onPane(pane: Pane): void;
  onPatch(patch: Record<string, unknown>): void;
  onSignIn(): void;
  onSignOut(): void;
}

export function Settings({ shown, pane, settings, account, library, onPane, onPatch, onSignIn, onSignOut }: Props) {
  return (
    <div className="sheet settings register-desk" role="dialog" aria-label="Settings" hidden={!shown}>
      <div className="settings-toolbar" role="tablist" aria-label="Settings panes">
        {PANES.map((p) => (
          <button
            key={p.id}
            type="button"
            role="tab"
            className="settings-tab"
            aria-selected={p.id === pane}
            onClick={() => onPane(p.id)}
          >
            <span className="settings-icon" data-pane={p.id} aria-hidden="true" />
            {p.label}
          </button>
        ))}
      </div>
      <div className="settings-pane" role="tabpanel" aria-label={PANES.find((p) => p.id === pane)?.label}>
        {settings && pane === 'account' && <Account account={account} onSignIn={onSignIn} onSignOut={onSignOut} />}
        {settings && pane === 'library' && <LibraryPane settings={settings} library={library} onPatch={onPatch} />}
        {settings && pane === 'heat' && <HeatPane settings={settings} onPatch={onPatch} />}
        {settings && pane === 'audio' && <AudioPane settings={settings} onPatch={onPatch} />}
        {settings && pane === 'claude' && <ClaudePane />}
        {settings && pane === 'privacy' && <PrivacyPane />}
        {settings && pane === 'appearance' && <AppearancePane settings={settings} onPatch={onPatch} />}
      </div>
    </div>
  );
}

function Row({ label, children }: { label: string; children: ReactNode }) {
  return (
    <div className="setting">
      <p className="setting-label" data-text="secondary">
        {label}
      </p>
      {children}
    </div>
  );
}

function Check({ label, checked, onChange }: { label: string; checked: boolean; onChange(v: boolean): void }) {
  return (
    <label className="check" data-dense>
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} />
      {label}
    </label>
  );
}

function Account({ account, onSignIn, onSignOut }: Pick<Props, 'account' | 'onSignIn' | 'onSignOut'>) {
  return (
    <>
      <Row label="Account">
        <p>
          {account.signedIn
            ? `Signed in as ${account.username ?? 'you'}.`
            : 'You’re not signed in. Learn, the library and the Console work fully without an account.'}
        </p>
        <button type="button" className="gel" onClick={account.signedIn ? onSignOut : onSignIn}>
          {account.signedIn ? 'Sign out' : 'Sign in or create an account'}
        </button>
      </Row>
      <Row label="Galaxy address">
        <p>{account.galaxy ?? 'Your galaxy’s address shows here once you have one.'}</p>
      </Row>
      <NotYet label="Stem player skin…" why="Skins come with Space, which isn’t in this build yet." />
      <NotYet
        label="Delete account…"
        why="Deleting an account needs mi-wwav.com’s delete step, which this build doesn’t have yet."
      />
    </>
  );
}

function LibraryPane({
  settings,
  library,
  onPatch,
}: {
  settings: SettingsValue;
  library: string;
  onPatch: Props['onPatch'];
}) {
  const [said, setSaid] = useState<string | null>(null);
  const [cleanup, setCleanup] = useState<string | null>(null);
  return (
    <>
      <Row label="Location">
        <p className="path">{library}</p>
      </Row>
      <Row label="Watched folders">
        <p>{settings.library.watchedFolders.length ? settings.library.watchedFolders.join(', ') : 'None.'}</p>
        <p className="why" data-text="secondary">
          Watching folders isn’t in this build yet; drop a folder on the library instead.
        </p>
      </Row>
      <Row label="Importing">
        <Check
          label="Leave files where they are instead of copying them"
          checked={settings.library.leaveInPlace}
          onChange={(v) => onPatch({ library: { leaveInPlace: v } })}
        />
      </Row>
      <Row label="Media">
        <div className="setting-actions">
          <button
            type="button"
            className="gel"
            onClick={() =>
              call<{ sentence: string; files: string[] }>('library.cleanup.preview').then(
                (r) => {
                  setCleanup(r.files.length ? r.sentence : null);
                  setSaid(r.files.length ? null : 'Nothing to clean up.');
                },
                (e: Error) => setSaid(e.message),
              )
            }
          >
            Clean up media…
          </button>
          <button
            type="button"
            className="gel"
            onClick={() =>
              call<{ deleted: string[] }>('library.trash.empty').then(
                (r) => setSaid(`Emptied the trash: ${r.deleted.length} ${r.deleted.length === 1 ? 'file' : 'files'}.`),
                (e: Error) => setSaid(e.message),
              )
            }
          >
            Empty trash
          </button>
        </div>
        {cleanup && (
          <div className="setting-actions">
            <p>{cleanup} that nothing in the library uses.</p>
            <button
              type="button"
              className="gel"
              onClick={() =>
                call<{ moved: string[] }>('library.cleanup.run').then(
                  (r) => {
                    setCleanup(null);
                    setSaid(`Moved ${r.moved.length} ${r.moved.length === 1 ? 'file' : 'files'} to the trash.`);
                  },
                  (e: Error) => setSaid(e.message),
                )
              }
            >
              Move to the trash
            </button>
          </div>
        )}
        {said && <p role="status">{said}</p>}
      </Row>
    </>
  );
}

function HeatPane({ settings, onPatch }: { settings: SettingsValue; onPatch: Props['onPatch'] }) {
  const here = Intl.DateTimeFormat().resolvedOptions().timeZone;
  const zones = typeof Intl.supportedValuesOf === 'function' ? Intl.supportedValuesOf('timeZone') : [here];
  return (
    <>
      <NotYet label="Spaces…" why="Spaces are set in Learn’s room, which isn’t in this build yet." />
      <Row label="Time zone">
        <select
          aria-label="Time zone"
          value={settings.heat.timeZone ?? ''}
          onChange={(e) => onPatch({ heat: { timeZone: e.target.value || null } })}
        >
          <option value="">This Mac’s ({here})</option>
          {zones.map((z) => (
            <option key={z} value={z}>
              {z}
            </option>
          ))}
        </select>
      </Row>
      <SchoolSheet />
      <MailAccounts />
    </>
  );
}

function AudioPane({ settings, onPatch }: { settings: SettingsValue; onPatch: Props['onPatch'] }) {
  const [device, setDevice] = useState<string | null>(null);
  useEffect(() => {
    call<{ device?: string | null }>('engine.status').then(
      (s) => setDevice(s.device ?? null),
      () => {},
    );
  }, []);
  return (
    <>
      <Row label="Output device">
        <p>{device ?? 'The default output.'}</p>
        <p className="why" data-text="secondary">
          Choosing a device isn’t in this build yet.
        </p>
      </Row>
      <Row label="Buffer">
        <select
          aria-label="Buffer"
          value={settings.audio.buffer}
          onChange={(e) => onPatch({ audio: { buffer: Number(e.target.value) } })}
        >
          {[64, 128, 256, 512, 1024].map((n) => (
            <option key={n} value={n}>
              {n} samples
            </option>
          ))}
        </select>
      </Row>
      <NotYet label="Plugin folders…" why="Plugin folders come with the Console, which isn’t in this build yet." />
      <Row label="Video">
        <Check
          label="Encode with the hardware"
          checked={settings.video.hardwareEncode}
          onChange={(v) => onPatch({ video: { hardwareEncode: v } })}
        />
        <Check
          label="Make proxy media"
          checked={settings.video.proxyMedia}
          onChange={(v) => onPatch({ video: { proxyMedia: v } })}
        />
      </Row>
    </>
  );
}

function PrivacyPane() {
  return (
    <>
      <NotYet
        label="Everything public…"
        why="The table of what is public, and your public Learn view as others see it, come with Space, which isn’t in this build yet."
      />
      <p className="why" data-text="secondary">
        Until then everything in Learn stays private.
      </p>
    </>
  );
}

function AppearancePane({ settings, onPatch }: { settings: SettingsValue; onPatch: Props['onPatch'] }) {
  return (
    <>
      <Row label="Appearance">
        <div className="radios" role="radiogroup" aria-label="Appearance">
          {(
            [
              ['light', 'Light'],
              ['dark', 'Dark'],
              ['system', 'Match system'],
            ] as const
          ).map(([value, label]) => (
            <label key={value} className="check" data-dense>
              <input
                type="radio"
                name="appearance"
                checked={settings.appearance === value}
                onChange={() => onPatch({ appearance: value })}
              />
              {label}
            </label>
          ))}
        </div>
        <p className="why" data-text="secondary">
          Space is always night.
        </p>
      </Row>
      <Row label={`Text size: ${settings.textSize} pt`}>
        <input
          type="range"
          aria-label="Text size"
          min={13}
          max={20}
          step={1}
          value={settings.textSize}
          onChange={(e) => onPatch({ textSize: Number(e.target.value) })}
        />
      </Row>
      <Row label="Motion">
        <Check label="Reduce motion" checked={settings.reduceMotion} onChange={(v) => onPatch({ reduceMotion: v })} />
        <p className="why" data-text="secondary">
          This Mac’s own Reduce Motion setting counts too.
        </p>
      </Row>
      <Row label="Keyboard">
        <table className="shortcuts">
          <tbody>
            {SHORTCUTS.map((s) => (
              <tr key={s.keys}>
                <td className="shortcut-keys">{keys(s.keys)}</td>
                <td>{keys(s.does)}</td>
                <td data-text="secondary">{s.where}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </Row>
    </>
  );
}
