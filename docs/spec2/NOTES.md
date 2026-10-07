# Notes for later chapters (from the rewrite so far)

- Chapter 3 fixes the MCP tools at exactly eight: list_tasks, add_task, update_task, plan_day, get_grades, add_pending_grade, log_focus, record_mail_thread (3.13). Arguments and results are Proposed there.
- The local MCP helper (`wi-mcp`) writes through the app's own store code, so each tool call is one journal transaction; the app need not be open (3.13). Chapter 8 should confirm this.
- A grade switched public is copied to the server in the clear and removed when switched back; private grades stay on the Mac unless encrypted sync is on (3.15, 3.16). Chapter 8's sync and privacy sections should say so.
- iCal addresses are kept in the Keychain and left out of heat.json (3.11).
- Syllabus import by Claude and the Claude-drafted weekly note are cut (3.18).
