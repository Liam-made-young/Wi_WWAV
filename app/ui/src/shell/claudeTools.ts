// The tools Wi_WWAV offers Claude through its MCP server (docs/SPEC.md
// 2.11, 3.13), and what each does in a few words: first launch lists them,
// and Settings → Claude puts a switch on each.

export const CLAUDE_TOOLS: readonly { name: string; does: string }[] = [
  { name: 'list_tasks', does: 'reads open tasks with heat, due date and estimate' },
  { name: 'add_task', does: 'makes a task, such as one found in a school email' },
  { name: 'update_task', does: 'sets difficulty, estimate and a one-line reason' },
  { name: 'plan_day', does: 'runs Plan my day, Learn’s own written rule' },
  { name: 'get_grades', does: 'reads courses and grades' },
  { name: 'add_pending_grade', does: 'records a grade notice with no score, linked to Brightspace' },
  { name: 'log_focus', does: 'logs a focus session on a task' },
  { name: 'record_mail_thread', does: 'records a thread Claude read: its account, priority and state' },
  { name: 'get_schedule', does: 'reads blocks, calendar events, tasks due and waiting drafts' },
  { name: 'draft_block', does: 'drafts one block at a time you asked for, for you to accept' },
  { name: 'list_habits', does: 'reads your habits and whether today is ticked' },
  { name: 'list_projects', does: 'reads projects and their milestones' },
  { name: 'add_project', does: 'makes an empty project, marked as Claude’s' },
  { name: 'add_milestone', does: 'adds a milestone on a day, never done' },
  { name: 'get_notes', does: 'reads your notes and a day’s daily note' },
  { name: 'add_note', does: 'adds a note, marked as Claude’s' },
  { name: 'list_inbox', does: 'reads the captures waiting in the inbox' },
  { name: 'add_capture', does: 'drops a line in the inbox for you to triage' },
  { name: 'list_mail_accounts', does: 'reads your mail accounts and how to find each one’s mail' },
  { name: 'list_mail', does: 'reads the threads already recorded, with how they were sorted' },
  { name: 'save_mail_text', does: 'saves a thread’s text on this Mac, for Mail’s reader' },
  { name: 'list_mail_outbox', does: 'reads what you asked for in Mail: mail to send, threads to archive' },
  { name: 'finish_mail_action', does: 'says an outbox action is done in Gmail, or why it failed' },
];
