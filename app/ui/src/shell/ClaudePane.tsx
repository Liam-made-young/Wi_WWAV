// Settings → Claude (docs/SPEC.md 2.11, 2.13, 3.13): how to add Wi_WWAV to
// Claude Desktop and Claude Code, with the helper's real path; how to ask
// for school mail; a switch for each tool (a tool switched off is missing
// from what Claude can see); and Claude's recent changes, each with Undo.
// The app holds no key and calls no model: Claude starts the helper itself.

import { useCallback, useEffect, useState } from 'react';
import { call } from '../bridge';
import { CLAUDE_TOOLS } from './claudeTools';

interface ClaudeData {
  helper: string;
  desktop: string;
  code: string;
  tools: { name: string; on: boolean }[];
  recent: { txnId: string; label: string; reason: string; at: number; undone: boolean }[];
  note?: string;
}

const sentence = (e: unknown) => (e instanceof Error ? e.message : String(e));
const DOES = new Map(CLAUDE_TOOLS.map((t) => [t.name, t.does]));

function when(at: number): string {
  const d = new Date(at);
  const time = d.toLocaleTimeString([], { hour: 'numeric', minute: '2-digit' });
  return d.toDateString() === new Date().toDateString()
    ? time
    : `${d.toLocaleDateString([], { month: 'short', day: 'numeric' })}, ${time}`;
}

/** `ask` is the core's `call`; a test passes its fake core's. */
export function ClaudePane({ ask = call }: { ask?: typeof call }) {
  const [data, setData] = useState<ClaudeData | null>(null);
  const [said, setSaid] = useState<string | null>(null);

  const read = useCallback(
    () => ask<ClaudeData>('heat.claude.get').then(setData, (e) => setSaid(sentence(e))),
    [ask],
  );
  useEffect(() => void read(), [read]);

  const act = (what: Promise<unknown>, done: string | null) =>
    what.then(() => setSaid(done), (e) => setSaid(sentence(e))).then(read);

  return (
    <>
      <div className="setting">
        <p className="setting-label" data-text="secondary">
          How Claude reaches Wi_WWAV
        </p>
        <p>
          Claude connects to Wi_WWAV as an MCP server, from Claude Desktop or Claude Code. The app holds no Anthropic key
          and calls no model. Claude estimates and drafts, and never decides: it can’t mark anything done, enter a
          score, delete, or make anything public. Every change it makes is labelled and can be undone.
        </p>
      </div>
      {data && (
        <>
          <div className="setting">
            <p className="setting-label" data-text="secondary">
              Add Wi_WWAV to Claude
            </p>
            {data.note && <p role="alert">{data.note}</p>}
            <p>Claude Code, in a terminal:</p>
            <pre className="claude-line" aria-label="The line for Claude Code">
              {data.code}
            </pre>
            <p>Claude Desktop, in its config file, then restart it:</p>
            <pre className="claude-line" aria-label="The lines for Claude Desktop">
              {data.desktop}
            </pre>
          </div>
          <div className="setting">
            <p className="setting-label" data-text="secondary">
              Mail
            </p>
            <p>
              Wi_WWAV never reads your mail. Claude reads it with its own Gmail connector, then records each thread in
              Mail under its account, sorted by how pressing it is. Ask Claude “Read my mail”, or pick the prompt{' '}
              <code>read_mail</code> that Wi_WWAV offers: in Claude Code it is <code>/mcp__wi-wwav__read_mail</code>.
              Your accounts are in Settings → Learn.
            </p>
          </div>
          <div className="setting">
            <p className="setting-label" data-text="secondary">
              The tools Claude sees
            </p>
            <ul className="claude-tools">
              {data.tools.map((t) => (
                <li key={t.name}>
                  <label className="check" data-dense>
                    <input
                      type="checkbox"
                      checked={t.on}
                      aria-label={t.name}
                      onChange={(e) => void act(ask('heat.claude.setTool', { name: t.name, on: e.target.checked }), null)}
                    />
                    <code>{t.name}</code>
                    <span data-text="secondary">{DOES.get(t.name) ?? ''}</span>
                  </label>
                </li>
              ))}
            </ul>
          </div>
          <div className="setting">
            <p className="setting-label" data-text="secondary">
              Claude’s recent changes
            </p>
            {data.recent.length === 0 ? (
              <p className="why" data-text="secondary">
                None yet.
              </p>
            ) : (
              <ul className="claude-recent">
                {data.recent.map((r) => (
                  <li key={r.txnId}>
                    <div>
                      <span>{r.label}</span> <span data-text="secondary">{when(r.at)}</span>
                      {r.reason && (
                        <p className="why" data-text="secondary">
                          {r.reason}
                        </p>
                      )}
                    </div>
                    <button
                      type="button"
                      className="gel plain"
                      disabled={r.undone}
                      aria-label={`Undo ${r.label}`}
                      onClick={() => void act(ask('history.undoEntry', { txnId: r.txnId }), `Undid ${r.label}.`)}
                    >
                      {r.undone ? 'Undone' : 'Undo'}
                    </button>
                  </li>
                ))}
              </ul>
            )}
          </div>
        </>
      )}
      {said && <p role="status">{said}</p>}
    </>
  );
}
