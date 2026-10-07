// The eight tools Wi_WWAV offers Claude through its MCP server (docs/SPEC.md
// 2.11, 3.13). The server isn't in this build yet; first launch and Settings
// say what Claude will see, and why they can't connect it yet.

export const CLAUDE_TOOLS: readonly { name: string; does: string }[] = [
  { name: 'list_tasks', does: 'reads open tasks with heat, due date and estimate' },
  { name: 'add_task', does: 'makes a task, such as one found in a school email' },
  { name: 'update_task', does: 'sets difficulty, estimate and a one-line reason' },
  { name: 'plan_day', does: 'runs Plan my day, Heat’s own written rule' },
  { name: 'get_grades', does: 'reads courses and grades' },
  { name: 'add_pending_grade', does: 'records a grade notice with no score, linked to Brightspace' },
  { name: 'log_focus', does: 'logs a focus session on a task' },
  { name: 'record_mail_thread', does: 'records a school thread Claude read, with its state' },
];
