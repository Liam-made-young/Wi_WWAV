//! Homes and types in the core (docs/HEAT.md): the commands that let a
//! person, and later Claude from inside the app, do what Grades and Get Info
//! do with a course, a project and a task's type; and the two jobs Learn
//! gives Claude itself, each one run of `claude_cli`:
//!
//! - a syllabus PDF is read into text here, in Rust, and Claude turns the
//!   text into one JSON object for the course. It becomes a draft to preview:
//!   nothing of the course changes until the person accepts it, and one ⌘Z
//!   takes the whole import back;
//! - open tasks no type matched are scored in one batch, as one entry,
//!   "Claude's estimates".
//!
//! Both run on a worker, so a command never waits on Claude: the command
//! leaves the job in the library (a draft that is `reading`, a flag that
//! scoring is wanted) and the worker picks it up, so a job outlives a
//! relaunch.

use std::path::Path;
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Map, Value};
use wi_heat::homes as rules;
use wi_heat_store::homes;

use crate::args::Args;
use crate::claude_cli::{self, Ask};
use crate::heat_cmd::{core_error, open_heat, write, wrote_outside};
use crate::{CoreError, Inner};

/// The `heat` event's kind for a syllabus draft that changed.
const DRAFT: &str = "syllabusDraft";

/// Set while a person has asked for scoring and the worker hasn't run it.
const SCORE_WANTED: &str = "heat.score.wanted";

/// A syllabus is a few pages of text: the larger model reads it once.
const SYLLABUS_MODEL: &str = "sonnet";
const SYLLABUS_TIMEOUT: Duration = Duration::from_secs(240);
/// Titles to minutes is the smallest model's job.
const SCORE_MODEL: &str = "haiku";
const SCORE_TIMEOUT: Duration = Duration::from_secs(180);

/// The largest PDF Learn reads as a syllabus.
const PDF_MAX_BYTES: u64 = 40 * 1024 * 1024;

const NOT_A_PDF: &str = "Drop the syllabus as a PDF.";
const NO_TEXT: &str =
    "That PDF has no text to read. If it is a scan, export it from the course site as text, or type the weights in.";

fn object<'a>(a: &'a Args, key: &str) -> Result<&'a Map<String, Value>, CoreError> {
    a.get(key)
        .and_then(Value::as_object)
        .ok_or_else(|| CoreError::new("bad_args", format!("This command needs {key}, an object.")))
}

pub(crate) fn invoke(i: &Inner, cmd: &str, a: &Args) -> Result<Value, CoreError> {
    match cmd {
        // `heat.course.importSyllabus {path, courseId?}`: a PDF to read, as a
        // draft that is `reading`. With `{json, courseId?}` the course's JSON
        // is given, and the draft is ready at once: the same door for a
        // caller that has read the syllabus itself.
        "heat.course.importSyllabus" => {
            open_heat(i)?;
            let course = a.opt_str("courseId");
            let clock = i.clock();
            let draft = match (a.get("json"), a.opt_str("path")) {
                (Some(answer), _) => {
                    let name = a.opt_str("fileName").unwrap_or("syllabus");
                    homes::draft_from_json(&mut i.store(), &clock, course, name, answer)
                        .map_err(core_error)?
                }
                (None, Some(path)) => {
                    let file = Path::new(path);
                    let is_pdf = file
                        .extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
                    if !is_pdf || !file.is_file() {
                        return Err(CoreError::new("refused", NOT_A_PDF));
                    }
                    // Said now, not after the file is read: there is no Claude to ask.
                    claude_cli::binary(i)?;
                    let name = file
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_else(|| "syllabus.pdf".into());
                    homes::draft_begin(&mut i.store(), &clock, course, &name, Some(path))
                        .map_err(core_error)?
                }
                (None, None) => {
                    return Err(CoreError::new(
                        "bad_args",
                        "heat.course.importSyllabus needs path, a PDF, or json.",
                    ))
                }
            };
            wrote_outside(i, &[DRAFT]);
            i.poke();
            let id = draft["id"].as_str().unwrap_or_default();
            let shown = homes::draft(&i.store(), &clock, id).map_err(core_error)?;
            Ok(json!({ "draft": shown.unwrap_or(draft) }))
        }
        "heat.syllabus.accept" => {
            let id = a.str("draftId")?;
            write(i, |s, c| homes::draft_accept(s, c, id))
        }
        "heat.syllabus.discard" => {
            let id = a.str("draftId")?;
            write(i, |s, _| homes::draft_discard(s, id))
        }
        "heat.course.update" => {
            let (id, set) = (a.str("id")?, object(a, "set")?);
            write(i, |s, c| homes::course_update(s, c, id, set))
        }
        "heat.project.update" => {
            let (id, set) = (a.str("id")?, object(a, "set")?);
            write(i, |s, c| homes::project_update(s, c, id, set))
        }
        // `type: null`, or none, gives the choice back to the title.
        "heat.task.setType" => {
            let (id, kind_of) = (a.str("taskId")?, a.opt_str("type"));
            write(i, |s, c| homes::set_type(s, c, id, kind_of))
        }
        "heat.task.reapplyDefaults" => {
            let scope = a.object();
            let force = scope.get("force") == Some(&json!(true));
            write(i, |s, c| homes::reapply(s, c, &scope, force))
        }
        // `heat.tasks.score {}`: asks Claude once, on the worker, to estimate
        // the tasks no type matched.
        "heat.tasks.score" => {
            open_heat(i)?;
            let waiting = homes::unscored(&i.store(), &i.clock(), true).map_err(core_error)?;
            if waiting.is_empty() {
                return Ok(json!({"asked": 0, "line": "Every task has a type or an estimate."}));
            }
            claude_cli::binary(i)?;
            i.kv.set(SCORE_WANTED, &json!(true))?;
            i.poke();
            let n = waiting.len();
            let line = format!(
                "Asking Claude to estimate {n} {}.",
                if n == 1 { "task" } else { "tasks" }
            );
            Ok(json!({"asked": n, "line": line}))
        }
        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}

/// Courses and types brought up to date (`homes::tidy`), as one entry, and
/// the views told. Run once when a library first opens under this version,
/// and after every calendar sync.
pub(crate) fn tidy(i: &Inner) -> Result<(), CoreError> {
    write(i, |s, c| homes::tidy(s, c))?;
    i.poke();
    Ok(())
}

/// The first time a library opens with homes and types: its courses and
/// tasks are brought over. After that, a sync does it.
pub(crate) fn tidy_once(i: &Inner) -> Result<(), CoreError> {
    if homes::tidied(&i.store()).map_err(core_error)? {
        return Ok(());
    }
    tidy(i)
}

/// A PDF's text, page by page. A PDF this crate can't walk is an error, not a
/// crash: it has been known to panic on odd files.
fn pdf_text(path: &Path) -> Result<(String, usize), String> {
    let size = std::fs::metadata(path)
        .map_err(|_| NOT_A_PDF.to_string())?
        .len();
    if size > PDF_MAX_BYTES {
        return Err("That PDF is too large to be a syllabus. Drop the syllabus by itself.".into());
    }
    let bytes = std::fs::read(path).map_err(|_| "Learn couldn't open that PDF.".to_string())?;
    let pages = std::panic::catch_unwind(|| pdf_extract::extract_text_from_mem_by_pages(&bytes))
        .map_err(|_| "Learn couldn't read that PDF.".to_string())?
        .map_err(|_| "Learn couldn't read that PDF.".to_string())?;
    let text = pages.join("\n\n");
    if text.split_whitespace().count() < 20 {
        return Err(NO_TEXT.into());
    }
    Ok((text, pages.len()))
}

/// Reads one draft's file and asks Claude for the course. Whatever happens,
/// the draft says so: ready with its preview, or failed with one sentence.
fn read_draft(i: &Inner, draft: &Value) {
    let id = draft["id"].as_str().unwrap_or_default().to_string();
    let fail = |sentence: &str| {
        let _ = homes::draft_fail(&mut i.store(), &id, sentence);
        wrote_outside(i, &[DRAFT]);
    };
    let path = draft["path"].as_str().unwrap_or_default().to_string();
    let (text, pages) = match pdf_text(Path::new(&path)) {
        Ok(read) => read,
        Err(sentence) => return fail(&sentence),
    };
    let _ = homes::draft_pages(&mut i.store(), &id, pages);
    wrote_outside(i, &[DRAFT]);
    let clock = i.clock();
    let hint = homes::draft_course_label(&i.store(), draft).ok().flatten();
    let prompt = rules::syllabus_prompt(&text, hint.as_deref(), &clock.today());
    let schema = rules::syllabus_schema();
    let answer = claude_cli::run_json(
        i,
        &Ask {
            prompt: &prompt,
            allowed_tools: &[],
            json_schema: Some(&schema),
            model: Some(SYLLABUS_MODEL),
            timeout: SYLLABUS_TIMEOUT,
        },
    );
    let ready = match answer {
        Ok(answer) => {
            homes::draft_ready(&mut i.store(), &clock, &id, &answer).map_err(|e| e.to_string())
        }
        Err(e) => Err(e.message),
    };
    match ready {
        Ok(_) => wrote_outside(i, &[DRAFT]),
        Err(sentence) => fail(&sentence),
    }
}

/// One batch to Claude, and its scores written as one entry. `all` is the
/// person's own ask: it takes in tasks asked about in the last day too.
fn score(i: &Inner, all: bool) -> Result<usize, CoreError> {
    let clock = i.clock();
    let tasks = homes::unscored(&i.store(), &clock, all).map_err(core_error)?;
    if tasks.is_empty() {
        return Ok(0);
    }
    let ids: Vec<String> = tasks.iter().map(|t| t.id.clone()).collect();
    homes::mark_asked(&mut i.store(), &clock, &ids).map_err(core_error)?;
    let prompt = rules::score_prompt(&tasks);
    let schema = rules::score_schema();
    let answer = claude_cli::run_json(
        i,
        &Ask {
            prompt: &prompt,
            allowed_tools: &[],
            json_schema: Some(&schema),
            model: Some(SCORE_MODEL),
            timeout: SCORE_TIMEOUT,
        },
    )?;
    let scores = rules::parse_scores(&answer, &tasks);
    let done = write(i, |s, c| homes::apply_scores(s, c, &scores))?;
    Ok(done["scored"].as_u64().unwrap_or(0) as usize)
}

/// What waits for the worker, done: each syllabus still to read, then a
/// scoring the person asked for, or (when the app runs with `auto_score`)
/// one for tasks a sync just brought. True if anything was done.
fn work(i: &Inner) -> bool {
    let mut did = false;
    // Read first, then let go of the library: reading a draft takes it again.
    let waiting = homes::drafts_to_read(&i.store()).unwrap_or_default();
    for draft in waiting {
        if i.closing() {
            return did;
        }
        read_draft(i, &draft);
        did = true;
    }
    let wanted = i.kv.get(SCORE_WANTED).ok().flatten() == Some(json!(true));
    if wanted {
        let _ = i.kv.set(SCORE_WANTED, &json!(false));
    }
    if (wanted || i.auto_score) && !i.closing() {
        match score(i, wanted) {
            Ok(0) => {}
            Ok(n) => {
                did = true;
                i.bus.status(
                    "claude",
                    &format!(
                        "Claude estimated {n} {}.",
                        if n == 1 { "task" } else { "tasks" }
                    ),
                );
            }
            // Said only when the person asked: a sync's own try is quiet,
            // and the tasks keep the estimate they have.
            Err(e) if wanted => i.bus.status("claude", &e.message),
            Err(_) => {}
        }
    }
    did
}

pub(crate) fn start(inner: &Arc<Inner>) -> JoinHandle<()> {
    let i = inner.clone();
    std::thread::Builder::new()
        .name("heat homes".into())
        .spawn(move || {
            // Once as the app opens (a syllabus left half read), then
            // whenever something pokes: a drop, a sync, a request to score.
            let mut poked = true;
            while !i.closing() {
                // A job left while another ran is found by looking again.
                while poked && work(&i) && !i.closing() {}
                poked = i.nap(Duration::from_secs(300));
            }
        })
        .expect("a thread for Learn's syllabus and scoring jobs")
}
