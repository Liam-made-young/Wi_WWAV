# Notes for later chapters (from the rewrite so far)

- Chapter 3 fixes the MCP tools at exactly eight: list_tasks, add_task, update_task, plan_day, get_grades, add_pending_grade, log_focus, record_mail_thread (3.13). Arguments and results are Proposed there.
- The local MCP helper (`wi-mcp`) writes through the app's own store code, so each tool call is one journal transaction; the app need not be open (3.13). Chapter 8 should confirm this.
- A grade switched public is copied to the server in the clear and removed when switched back; private grades stay on the Mac unless encrypted sync is on (3.15, 3.16). Chapter 8's sync and privacy sections should say so.
- iCal addresses are kept in the Keychain and left out of heat.json (3.11).
- Syllabus import by Claude and the Claude-drafted weekly note are cut (3.18).
- Chapter 4: ⌥-drag on empty sky orbits the camera; ⌥-drag from one planet onto another declares a link. Forking from a family-tree node is ⌘E (Open in Console). A film opened in the Console comes in with its picture on a video track. Open item: galleries are shown, not made, in v1.
- Chapter 5 (new numbers): 5.9 built-ins are PRANA's reverb and delay (returns, fed by sends), distortion and tremolo (inserts), a filter with LPF and HPF amounts (an insert), and a fixed master limiter with no knob. "As settings" remixes hold only what PRANA's wrmx can hold; pitch, speed and time in an opened wrmx are listed as "Not applied". 5.11 video: a plain composite, the top track's clip is the picture; tracks have only "hide"; the frame is the first clip's (no 1:1 or 9:16). 5.14 is opening any file to tear it apart (Open in Console lands here). 5.16 Push to Space. No freeze anywhere, so "Send session" offers none. Browser sources: Library, Plugins, Takes.
- Chapter 6 must drop grade, titles and generators from session.json and freeze from Send session; 6.8 "As settings" is "a level, a mute or a built-in effect's amount".
- Chapter 8 must drop PRANA golden-hash parity for sessions (9.4) and describe the plain composite (9.5).
- Chapters 6-7: 6.12 files in the store is removed, so old 6.13 -> 6.12, old 6.14 -> 6.13; old 8.x -> 7.x one to one. A filter amount counts toward "As settings" only on the master (wrmx holds lpf/hpf on the master only). The plus icon means New. Chapter 7 has two sound exceptions (the Console's click, Heat's chime).
