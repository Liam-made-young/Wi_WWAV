// Claude through the server (docs/SPEC.md 2.11, 3.12, 7.16, 9.8, 10.4): one
// endpoint per job, the prompt held on the server, 50 calls a day per
// account. The mock answers each task the same way every time from its
// input alone, so tests can check what the app does with an answer.
import { error, isObject, json } from '../http.js';
import { requireUser } from '../auth.js';
import { limit } from '../state.js';

const DAILY = 50;
const DAY_MS = 24 * 60 * 60 * 1000;

const need = (ok, message) => {
  if (!ok) throw error(400, message);
};
const text = (v) => typeof v === 'string' && v.trim() !== '';
const clamp = (v, lo, hi) => Math.min(hi, Math.max(lo, v));

// Heat's estimate chain when there's no better guess: the type's average,
// else difficulty × 20 minutes (3.1).
function guess(title, type, averages) {
  const difficulty = clamp(1 + Math.floor(title.length / 12), 1, 5);
  const average = isObject(averages) ? averages[type] : undefined;
  const minutes = Number.isFinite(average) ? average : difficulty * 20;
  return { difficulty, minutes };
}

const TASKS = {
  score({ title, type, averages }) {
    need(text(title), 'Give the task a name first.');
    const { difficulty, minutes } = guess(title, type, averages);
    return { difficulty, minutes, reason: `Your ${type || 'tasks'} like this take about ${minutes} minutes.` };
  },
  'score-batch'({ items }) {
    need(Array.isArray(items) && items.every((i) => isObject(i) && text(i.title)), 'Send a list of tasks.');
    return {
      items: items.map((i, n) => {
        const { difficulty, minutes } = guess(i.title, i.type);
        return { index: Number.isInteger(i.index) ? i.index : n, difficulty, minutes: clamp(minutes, 5, 600) };
      }),
    };
  },
  // At most 8 messages a sync (2.11). A subject with a due date becomes a
  // task; everything else is left alone.
  'read-mail'({ messages }) {
    need(Array.isArray(messages) && messages.length <= 8, 'Send at most 8 messages.');
    const tasks = messages
      .filter((m) => isObject(m) && /\bdue\b/i.test(String(m.subject)))
      .map((m) => ({ title: String(m.subject).replace(/\s*-\s*due\b.*$/i, ''), messageId: m.id ?? null }));
    return { tasks };
  },
  syllabus({ text: syllabus }) {
    need(text(syllabus), 'Paste the syllabus first.');
    const categories = [...syllabus.matchAll(/([A-Za-z][A-Za-z ]*?)\s+(\d{1,3})%/g)].map((m) => ({
      name: m[1].trim(),
      weight: Number(m[2]),
    }));
    return { categories, scale: null, keywords: {} };
  },
  // Restates only the facts Heat passed in: "NEVER invent metrics".
  'review-note'({ facts }) {
    need(Array.isArray(facts) && facts.every(text), "Send the week's facts.");
    return { draft: `What moved: ${facts.join('; ') || 'nothing yet'}.\nWhat slipped: \nNext week's one thing: ` };
  },
  'release-plan'({ title }) {
    need(text(title), 'Name the release first.');
    return {
      phases: {
        pre: [`Finish the master of ${title}`, 'Write the letter that announces it'],
        launch: [`Push ${title} to your galaxy`],
        post: ['Answer the forks that come in'],
      },
    };
  },
  feedback({ question }) {
    need(text(question), 'Ask a question first.');
    return { answer: 'The analysis has no sections yet, so there is nothing to point to at [0:00].' };
  },
  // The clerk answers only from the record and its seller's notes (7.16).
  clerk({ record, question }) {
    need(isObject(record) && text(record.artist) && text(question), 'Hold a record and ask about it.');
    const answer = text(record.notes)
      ? `${record.artist}'s note says: ${record.notes}`
      : `I don't know. Neither the file nor ${record.artist}'s notes say.`;
    return { answer };
  },
};

function assist(ctx) {
  const me = requireUser(ctx);
  const task = Object.hasOwn(TASKS, ctx.params.task) ? TASKS[ctx.params.task] : null;
  if (!task) return error(404, 'No such task');
  const { state } = ctx;
  limit(state, {
    scope: 'assist',
    key: `u:${me.id}`,
    max: 10,
    windowMs: 60 * 1000,
    message: 'Too many requests. Wait a minute, then try again.',
  });
  // The day is the server's, in UTC, and it comes back at midnight.
  const day = Math.floor(state.now() / DAY_MS);
  const used = state.assist.get(me.id);
  if (used?.day === day && used.used >= DAILY) {
    const retryAfterSeconds = Math.ceil(((day + 1) * DAY_MS - state.now()) / 1000);
    return json(
      429,
      {
        error: "Claude's 50 calls for today are used. They come back at midnight.",
        code: 'daily_limit',
        retryAfterSeconds,
      },
      { 'retry-after': String(retryAfterSeconds) },
    );
  }
  const result = task(isObject(ctx.body) ? ctx.body : {});
  state.assist.set(me.id, { day, used: used?.day === day ? used.used + 1 : 1 });
  return json(200, { result });
}

export const routes = [['POST', '/api/assist/:task', assist]];
