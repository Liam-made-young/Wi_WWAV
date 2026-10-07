// First launch (docs/SPEC.md 2.14): five steps, each with exactly one
// secondary action, "Skip for now", ending on Heat → Today with the strip
// reading "All clear" and "Nothing playing". The app is fully usable signed
// out (Open #4, at its recommendation): Heat, the library and the Console
// are local. Steps whose parts aren't in this build yet say so.

import { forwardRef, useImperativeHandle, useState } from 'react';
import { call } from '../bridge';
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

const STEPS = [
  'Sign in',
  'Make your astronaut',
  'Claim your galaxy',
  'Import your folder',
  'Connect your school calendar',
];

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
      if (step === 3 && paths.length) void inspect(paths[0]);
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
                : 'Your account puts your work in Space and on your shelf. Heat, the library and the Console work fully without one.'}
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
            <p>Race · body · hair & marks · rocket · ready. Skipping gives you the starter astronaut.</p>
            <NotYet
              label="Make my astronaut"
              why="The astronaut maker comes with Space, which isn’t in this build yet."
            />
          </>
        )}
        {step === 2 && (
          <>
            <p>
              You have no galaxy yet. A galaxy is yours. Projects orbit it as solar systems, and each song or film is a
              world inside one. The sun at the centre is where you say who you are.
            </p>
            <NotYet label="Make my galaxy" why="Making a galaxy comes with Space, which isn’t in this build yet." />
          </>
        )}
        {step === 3 && (
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
        {step === 4 && (
          <>
            <NotYet
              label="Paste your Brightspace calendar link"
              why="The link is kept in the keychain, and this build can’t store it there yet."
            />
            <NotYet label="Connect Google" why="Connecting Google isn’t in this build yet." />
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
