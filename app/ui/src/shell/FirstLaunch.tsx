// First launch (docs/SPEC.md 2.14): five steps (sign in, claim your galaxy,
// import your folder, add your calendars, connect Claude), each with exactly
// one secondary action, "Skip for now", ending on Heat → Today with the strip
// reading "All clear" and "Nothing playing". The app is fully usable signed
// out (Open #4, at its recommendation): Heat, the library and the Console
// are local. Steps whose parts aren't in this build yet say so.

import { forwardRef, useImperativeHandle, useState } from 'react';
import { call } from '../bridge';
import { CLAUDE_TOOLS } from './claudeTools';
import { useCoreEvent } from './hooks';
import { NotYet } from './NotYet';

interface Props {
  shown: boolean;
  signedIn: boolean;
  onSignIn(): Promise<void>;
  onSaid(sentence: string): void;
  onDone(): void;
}

export interface FirstLaunchHandle {
  drop(paths: string[]): void;
}

const STEPS = ['Sign in', 'Claim your galaxy', 'Import your folder', 'Add your calendars', 'Connect Claude'];

export const FirstLaunch = forwardRef<FirstLaunchHandle, Props>(function FirstLaunch(
  { shown, signedIn, onSignIn, onSaid, onDone },
  ref,
) {
  const [step, setStep] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const [path, setPath] = useState('');
  const [summary, setSummary] = useState<string | null>(null);
  const [progress, setProgress] = useState<string | null>(null);

  useCoreEvent<{ done: number; total: number }>('library.import', ({ done, total }) =>
    setProgress(`Bringing in ${done} of ${total}…`),
  );

  const next = () => {
    setError(null);
    if (step === STEPS.length - 1) onDone();
    else setStep(step + 1);
  };

  const inspect = (p: string) =>
    call<{ summary: string }>('library.inspect', { path: p }).then(
      (r) => {
        setPath(p);
        setSummary(r.summary);
        setError(null);
      },
      (e: Error) => setError(e.message),
    );

  useImperativeHandle(ref, () => ({
    drop(paths) {
      if (step === 2 && paths.length) void inspect(paths[0]);
    },
  }));

  const bringIn = () => {
    // It runs in the background; pressing again later picks up where it stopped.
    call<{ clips: unknown[] }>('library.import', { paths: [path], label: 'import' }).then(
      (r) => onSaid(`Brought in ${r.clips.length} ${r.clips.length === 1 ? 'file' : 'files'}.`),
      (e: Error) => onSaid(e.message),
    );
    next();
  };

  return (
    <div className="first register-desk" role="dialog" aria-label="Welcome to Wi_WWAV" hidden={!shown}>
      <div className="first-card">
        <p className="first-step" data-text="secondary">
          Step {step + 1} of {STEPS.length}
        </p>
        <h1 className="sheet-title">{STEPS[step]}</h1>
        {step === 0 && (
          <>
            <p>
              {signedIn
                ? 'You’re signed in.'
                : 'Your account puts your work in Space, in a galaxy of your own. Learn, the library and the Console work fully without one.'}
            </p>
            <button
              type="button"
              className="gel"
              onClick={() => (signedIn ? next() : onSignIn().then(next, (e: Error) => setError(e.message)))}
            >
              {signedIn ? 'Continue' : 'Sign in or create an account'}
            </button>
          </>
        )}
        {step === 1 && (
          <>
            <p>
              You have no galaxy yet. A galaxy is yours. Projects orbit it as solar systems, and each song or film is a
              world inside one. The sun at the centre is where you say who you are.
            </p>
            <p data-text="secondary">
              Behind your sun, people can see a simple version of Learn, once you choose what goes there. Everything in
              Learn stays private until then.
            </p>
            <NotYet label="Make my galaxy" why="Making a galaxy comes with Space, which isn’t in this build yet." />
          </>
        )}
        {step === 2 && (
          <div data-drop="import">
            <p>Drop your music folder here, or type where it is. Nothing is copied until you press.</p>
            <form
              onSubmit={(e) => {
                e.preventDefault();
                if (path.trim()) void inspect(path.trim());
              }}
            >
              <label className="field">
                <span data-text="secondary">Folder</span>
                <input
                  value={path}
                  placeholder="/Users/you/Music"
                  onChange={(e) => {
                    setPath(e.target.value);
                    setSummary(null);
                  }}
                />
              </label>
              {summary ? (
                <>
                  <p className="first-summary">{summary}</p>
                  <button type="button" className="gel" onClick={bringIn}>
                    Bring them in
                  </button>
                </>
              ) : (
                <button type="submit" className="gel">
                  Look inside
                </button>
              )}
            </form>
            {progress && <p role="status">{progress}</p>}
          </div>
        )}
        {step === 3 && (
          <>
            <p>Each calendar comes in as a private iCal address, so the app needs no Google sign-in.</p>
            <p>
              Paste your Brightspace calendar link, and any other calendar’s, in Settings → Learn ({'⌘'},). Each is
              kept in the keychain on this Mac and nowhere else.
            </p>
          </>
        )}
        {step === 4 && (
          <>
            <p>
              Claude reaches Wi_WWAV through its MCP server, from Claude Desktop or Claude Code. These are the tools it
              will see, and Claude asks before it calls one you haven’t allowed.
            </p>
            <ul className="tools">
              {CLAUDE_TOOLS.map((t) => (
                <li key={t.name}>
                  <code>{t.name}</code> {t.does}
                </li>
              ))}
            </ul>
            <p>The lines that add Wi_WWAV to Claude are in Settings → Claude ({'⌘'},), with a switch for each tool.</p>
            <p data-text="secondary">
              Skipping leaves Learn whole: estimates use your averages, and Plan my day is Learn’s own rule.
            </p>
          </>
        )}
        {error && (
          <p className="first-error" role="alert">
            {error}
          </p>
        )}
        <button type="button" className="gel plain" onClick={next}>
          Skip for now
        </button>
      </div>
    </div>
  );
});
