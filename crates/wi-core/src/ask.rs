//! The Claude prompt box, `ask.*` (docs/ASK.md). ⌘K opens it where Search
//! was.
//!
//! - `ask.search` is the box as a search: instant results from the library's
//!   own index, with no Claude and no network (`ask_index.rs`).
//! - `ask.send` asks Claude. The run is the person's own Claude Code, with
//!   one MCP server, the core's (`ask_mcp.rs`), whose tools are the core's
//!   commands (`ask_tools.rs`). The answer comes back as Markdown in which a
//!   Learn record is `[title](learn://table/id)` and a Wikipedia article is
//!   `[Title](wiki://Title)`, for the panel to make into links.
//! - What Claude would change is staged on the run, never made by it.
//!   `ask.apply` makes all of it as one journal entry: one ⌘Z.
//!   `ask.applyOutward` makes one change that leaves this Mac, after its own
//!   click. `ask.discard` lets a run go.
//!
//! The app holds no key and names no model of its own beyond the size to
//! use: with Claude Code missing, signed out or offline, `ask.send` answers
//! one sentence (code `claude`) and the box goes on working as a search.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

use crate::args::Args;
use crate::ask_tools::{self, Run, Staged};
use crate::bus::lock;
use crate::{ask_index, ask_mcp, batch, claude_cli, db, CoreError, Inner};

const TIMEOUT: Duration = Duration::from_secs(240);
/// Runs kept for their staged changes; the oldest goes when there are more.
const KEPT: usize = 12;

#[derive(Default)]
pub(crate) struct State {
    /// The tools' port, once the server has started.
    pub port: Mutex<Option<u16>>,
    /// Runs still asking, by their token.
    asking: Mutex<HashMap<String, Arc<Run>>>,
    /// Runs whose changes wait for the person, newest last.
    waiting: Mutex<Vec<Arc<Run>>>,
}

impl State {
    pub fn run_by_token(&self, token: &str) -> Option<Arc<Run>> {
        if token.is_empty() {
            return None;
        }
        lock(&self.asking).get(token).cloned()
    }

    fn run(&self, id: &str) -> Option<Arc<Run>> {
        lock(&self.waiting)
            .iter()
            .find(|r| r.id == id)
            .cloned()
            .or_else(|| lock(&self.asking).values().find(|r| r.id == id).cloned())
    }
}

fn error(sentence: impl Into<String>) -> CoreError {
    CoreError::new("claude", sentence)
}

fn token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// What Claude is told before the question: what it is, what day it is,
/// what the tables hold, and the rules of the box.
fn system_prompt(i: &Inner) -> Result<String, CoreError> {
    let clock = i.clock();
    let today = clock.today();
    let weekday = wi_formula::parse_date(&today)
        .map(|d| {
            const DAYS: [&str; 7] = [
                "Sunday",
                "Monday",
                "Tuesday",
                "Wednesday",
                "Thursday",
                "Friday",
                "Saturday",
            ];
            DAYS[((d as i64 + 4).rem_euclid(7)) as usize]
        })
        .unwrap_or("");
    Ok(format!(
        r#"You are Claude in the prompt box of Learn, the planner in the desktop app Wi_WWAV. The person typed a request into the box at the top of the window. Learn holds their college work and everything else they plan: tasks, courses and grades, habits, focus time, mail notes, projects, notes, and tables of their own. All of it is on their Mac, and you reach it only through the tools here.

Now: {now} ({weekday}). Today is {today}. Unless they name days, "this week" means today through the next six days, which is how Learn's "Due this week" list reads.

# The data

Every kind of record is a table. Read with list_rows (filter and sort there, and ask only for the columns you need), search when you don't know the table, get_row for one record whole, today for a day, summarize for totals. Never state anything about their data that a tool didn't give you.

{tables}
A column marked read-only can't be changed. A link column (to Courses, to Tasks) takes the other row's name or id, and filters by either.

# Changing things

Tools that change data only stage the change. Nothing is written until the person looks at the preview under your answer and applies it with one click; the whole of it is then a single undo. So:
- Read first, then stage every change the request needs, in as few calls as you can (update_rows takes many rows at once).
- Say what you staged in a sentence or two, as something that will happen when they apply it. Never say it is done.
- If a tool answers with an error, fix what it names and call it again.
- Delete only when asked to delete.
- Some things leave their Mac: sending mail or a reply (mail_send, mail_reply), archiving or marking a thread in their mailbox (mail_file), and making a record public (make_public). Use these only when asked for in so many words. They are never part of the one-click apply: the person is shown each one whole and answers yes or no to it, every time. Say it is waiting for their answer, never that it was sent or done. Write a mail exactly as it should go, with no placeholders; if you don't know an address or a fact it needs, ask instead.
- Flashcards and study notes are rows of Notes: Title "Flashcards: <topic>", and Markdown with one "Q: …" line and one "A: …" line per card, a blank line between cards.

# What is on screen

The request comes with what the person is looking at: the tab, what is selected, the rows in view, and in the Wiki tab the article and section. "These", "this", "here" and "this class" mean that. In the Wiki tab, read the article or section with wiki_get_article before explaining or making cards from it.

# Answering

Call your tools first, without writing anything in between: no "let me check". When they are done, write the whole answer once. It shows in the panel under the box, so keep it brief, in Markdown: short paragraphs, a list or a small table when it helps. No headings.
- Every Learn record you mention is a link: [its title](learn://<table id>/<row id>), such as [Kanji quiz](learn://task/01JABC). The table id is the lower-case word in the tool's answers ("task", "course", "note"), and for the person's own tables the id of the table.
- For a question of general knowledge, call wiki_search for the most relevant article or two, then answer the question yourself, plainly and in full, and end with a line: Wikipedia: [Article title](wiki://Article title). Use the titles exactly as the search gives them. Do this from any tab.
- Dates read as people say them ("Friday, Oct 9"), not as ISO.
"#,
        now = clock.iso(clock.now_ms),
        tables = db::describe(i)?,
    ))
}

/// What goes on stdin: what is on screen, what was said before, the request.
fn user_prompt(prompt: &str, context: &Value, history: &Value) -> String {
    let mut out = String::new();
    if context.is_object() {
        out.push_str("On screen now:\n");
        out.push_str(&serde_json::to_string_pretty(context).unwrap_or_default());
        out.push_str("\n\n");
    }
    let turns: Vec<&Value> = history.as_array().into_iter().flatten().collect();
    if !turns.is_empty() {
        out.push_str("Earlier in this conversation:\n");
        for t in turns.iter().rev().take(4).rev() {
            let cut = |s: &str, n: usize| -> String { s.chars().take(n).collect() };
            out.push_str(&format!(
                "They asked: {}\nYou answered: {}\n{}\n",
                cut(t["prompt"].as_str().unwrap_or(""), 600),
                cut(t["answer"].as_str().unwrap_or(""), 1500),
                match t["applied"].as_bool() {
                    Some(true) => "(They applied the changes you staged.)\n",
                    Some(false) => "(They have not applied the changes you staged.)\n",
                    None => "",
                }
            ));
        }
        out.push('\n');
    }
    out.push_str("Their request:\n");
    out.push_str(prompt.trim());
    out.push('\n');
    out
}

/// One run of the Claude Code command line with the core's tools and no
/// others. The result envelope `--output-format json` prints.
fn run_claude(
    binary: &Path,
    prompt: &str,
    system: &str,
    mcp: &Value,
    model: &str,
    stop: &dyn Fn() -> Option<&'static str>,
) -> Result<Value, CoreError> {
    let allowed: Vec<String> = ask_tools::TOOLS
        .iter()
        .map(|t| format!("mcp__learn__{}", t.name))
        .collect();
    let mut command = Command::new(binary);
    command
        .arg("-p")
        // Every message as it is written, a line of JSON each, so words
        // Claude wrote before calling a tool are not lost with the turn.
        .args(["--output-format", "stream-json"])
        .arg("--verbose")
        .arg("--no-session-persistence")
        .arg("--disable-slash-commands")
        // No built-in tools: no shell, no files, no web. Only the core's.
        .args(["--tools", ""])
        .arg("--strict-mcp-config")
        .args(["--mcp-config", &mcp.to_string()])
        .args(["--allowedTools", &allowed.join(",")])
        .args(["--system-prompt", system]);
    if !model.is_empty() {
        command.args(["--model", model]);
    }
    let mut path = vec![binary.parent().unwrap_or(Path::new("/")).to_path_buf()];
    path.extend(
        ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"]
            .iter()
            .map(std::path::PathBuf::from),
    );
    if let Some(have) = std::env::var_os("PATH") {
        path.extend(std::env::split_paths(&have));
    }
    command
        // A folder of its own, so no project's instructions or settings apply.
        .current_dir(std::env::temp_dir())
        .env("PATH", std::env::join_paths(path).unwrap_or_default())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| error(format!("Claude Code wouldn't start: {e}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        let prompt = prompt.as_bytes().to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&prompt);
        });
    }
    let read = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            text
        })
    };
    let out = read(Box::new(child.stdout.take().expect("stdout is piped")));
    let err = read(Box::new(child.stderr.take().expect("stderr is piped")));
    let deadline = Instant::now() + TIMEOUT;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => {
                let why = stop().or_else(|| {
                    (Instant::now() >= deadline)
                        .then_some("Claude didn't answer within 4 minutes. Try again, or ask for less at once.")
                });
                if let Some(why) = why {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(error(why));
                }
                std::thread::sleep(Duration::from_millis(40));
            }
            Err(e) => return Err(error(format!("Claude Code stopped unexpectedly: {e}"))),
        }
    };
    let (out, err) = (
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default(),
    );
    let mut envelope = read_stream(&out);
    let said = envelope["result"].as_str().unwrap_or_default().to_string();
    let said = said.as_str();
    let all = format!("{said}\n{out}\n{err}").to_lowercase();
    if [
        "/login",
        "not logged in",
        "invalid api key",
        "authentication_error",
        "oauth token",
    ]
    .iter()
    .any(|sign| all.contains(sign))
        && (!status.success() || envelope["is_error"] == true)
    {
        return Err(error(
            "Claude Code is signed out. Open a terminal, run claude, sign in, then try again.",
        ));
    }
    if !status.success() || envelope["is_error"] == true || !envelope.is_object() {
        let offline = [
            "enotfound",
            "econnrefused",
            "network",
            "fetch failed",
            "connection error",
            "timed out",
        ]
        .iter()
        .any(|sign| all.contains(sign));
        if offline {
            return Err(error(
                "Claude can't be reached right now. The box still searches what is on this Mac.",
            ));
        }
        let why = [said, err.trim(), out.trim()]
            .into_iter()
            .find(|s| !s.is_empty())
            .and_then(|s| s.lines().next())
            .unwrap_or("it gave no reason");
        let why: String = why.chars().take(200).collect();
        return Err(error(format!("Claude couldn't finish: {why}")));
    }
    // The answer is everything Claude wrote, not only what came after its last tool call.
    let whole = envelope["said"].as_str().map(String::from);
    if let Some(whole) = whole.filter(|w| w.len() > said.len()) {
        envelope["result"] = json!(whole);
    }
    Ok(envelope)
}

/// The run's output, a line of JSON per event, as its result event with
/// `said` added: every piece of text Claude wrote, in order. A run that
/// printed one object (a test's stand-in) reads the same way.
fn read_stream(out: &str) -> Value {
    let mut envelope = Value::Null;
    let mut texts: Vec<String> = Vec::new();
    for line in out.lines() {
        let Ok(event) = serde_json::from_str::<Value>(line.trim()) else {
            continue;
        };
        match event["type"].as_str() {
            Some("assistant") => {
                for block in event["message"]["content"].as_array().into_iter().flatten() {
                    if block["type"] != "text" {
                        continue;
                    }
                    let text = block["text"].as_str().unwrap_or("").trim();
                    if !text.is_empty() && texts.last().map(String::as_str) != Some(text) {
                        texts.push(text.to_string());
                    }
                }
            }
            Some("result") => envelope = event,
            _ if envelope.is_null() && event.get("result").is_some() => envelope = event,
            _ => {}
        }
    }
    if envelope.is_object() && !texts.is_empty() {
        envelope["said"] = json!(texts.join("\n\n"));
    }
    envelope
}

/// The Wikipedia articles an answer links, in order, each once.
fn wiki_links(answer: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut rest = answer;
    while let Some(at) = rest.find("](wiki://") {
        let after = &rest[at + 9..];
        let Some(end) = after.find(')') else { break };
        let title = wi_wiki::display_title(&wi_wiki::percent_decode(&after[..end]));
        if !title.is_empty() && !out.contains(&title) {
            out.push(title);
        }
        rest = &after[end..];
    }
    out
}

fn change_json(run: &Run) -> Value {
    let staged = lock(&run.staged);
    if staged.is_empty() {
        return Value::Null;
    }
    json!({
        "summary": ask_tools::summary(&staged),
        "lines": staged.iter().map(|s| s.line.clone()).collect::<Vec<_>>(),
        "count": staged.len(),
    })
}

fn outward_json(run: &Run) -> Value {
    Value::Array(
        lock(&run.outward)
            .iter()
            .enumerate()
            .map(|(index, s)| json!({"index": index, "line": s.line}))
            .collect(),
    )
}

/// `ask.send {prompt, context?, history?, model?}`
fn send(inner: &Arc<Inner>, a: &Args) -> Result<Value, CoreError> {
    let i: &Inner = inner;
    let prompt = a.str("prompt")?;
    if prompt.trim().is_empty() {
        return Err(CoreError::new("bad_args", "Type something to ask first."));
    }
    let binary = claude_cli::binary(i)?;
    let port = ask_mcp::port(inner)
        .map_err(|e| error(format!("Learn couldn't open its tools to Claude: {e}")))?;
    let run = Arc::new(Run::new(wwav_ids::ulid(), token()));
    let system = system_prompt(i)?;
    let context = a.get("context").cloned().unwrap_or(Value::Null);
    let history = a.get("history").cloned().unwrap_or(Value::Null);
    let stdin = user_prompt(prompt, &context, &history);
    let mcp = json!({"mcpServers": {"learn": {
        "type": "http",
        "url": format!("http://127.0.0.1:{port}/mcp"),
        "headers": {"Authorization": format!("Bearer {}", run.token)},
    }}});
    let model = a
        .opt_str("model")
        .map(String::from)
        .or_else(|| std::env::var("WI_WWAV_ASK_MODEL").ok())
        .unwrap_or_else(|| "sonnet".to_string());
    lock(&i.ask.asking).insert(run.token.clone(), run.clone());
    i.bus
        .emit("ask", json!({"id": run.id, "doing": "Asking Claude"}));
    let started = Instant::now();
    let stop = || {
        if i.closing() {
            Some("Claude was stopped because Wi_WWAV is closing.")
        } else if run.cancelled.load(Ordering::SeqCst) {
            Some("Stopped.")
        } else {
            None
        }
    };
    let result = run_claude(&binary, &stdin, &system, &mcp, &model, &stop);
    // The answer is in, or never came: the token opens nothing from here on.
    lock(&i.ask.asking).remove(&run.token);
    let envelope = result?;
    let answer = envelope["result"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();
    let mut wiki = wiki_links(&answer);
    for read in lock(&run.wiki).iter() {
        if !wiki.contains(read) {
            wiki.push(read.clone());
        }
    }
    let out = json!({
        "id": run.id,
        "answer": answer,
        "wiki": wiki,
        "opens": lock(&run.opens).clone(),
        "change": change_json(&run),
        "outward": outward_json(&run),
        "tools": lock(&run.calls).clone(),
        "seconds": (started.elapsed().as_secs_f64() * 10.0).round() / 10.0,
    });
    let pending = !lock(&run.staged).is_empty() || !lock(&run.outward).is_empty();
    if pending {
        let mut waiting = lock(&i.ask.waiting);
        waiting.push(run);
        let over = waiting.len().saturating_sub(KEPT);
        waiting.drain(..over);
    }
    Ok(out)
}

/// A value with every stand-in id swapped for the row it became.
fn substitute(v: &Value, made: &HashMap<String, String>) -> Value {
    match v {
        Value::String(s) => made.get(s).map_or_else(|| v.clone(), |real| json!(real)),
        Value::Array(items) => Value::Array(items.iter().map(|x| substitute(x, made)).collect()),
        Value::Object(m) => Value::Object(
            m.iter()
                .map(|(k, x)| (k.clone(), substitute(x, made)))
                .collect(),
        ),
        other => other.clone(),
    }
}

/// Makes one staged change. Ok is what it made, by stand-in id.
fn make(i: &Inner, s: &Staged, made: &mut HashMap<String, String>) -> Result<(), String> {
    let args = substitute(&s.args, made);
    let result = ask_tools::invoke(i, &s.cmd, &args).map_err(|e| e.message)?;
    // The Database's writes say what they couldn't do instead of failing whole.
    if let Some(first) = result["failed"].as_array().and_then(|f| f.first()) {
        let nothing = result["changed"] == 0
            || result["ids"].as_array().is_some_and(Vec::is_empty)
            || result["deleted"] == 0;
        if nothing {
            return Err(first["message"]
                .as_str()
                .unwrap_or("It couldn't be made.")
                .to_string());
        }
    }
    for (stand_in, real) in s
        .makes
        .iter()
        .zip(result["ids"].as_array().into_iter().flatten())
    {
        if let Some(real) = real.as_str() {
            made.insert(stand_in.clone(), real.to_string());
        }
    }
    Ok(())
}

/// `ask.apply {id}`: everything the run staged, as one undo step.
fn apply(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let id = a.str("id")?;
    let run = i.ask.run(id).ok_or_else(|| {
        CoreError::new(
            "refused",
            "Those changes aren't waiting any more. Ask again.",
        )
    })?;
    let staged: Vec<Staged> = std::mem::take(&mut *lock(&run.staged));
    if staged.is_empty() {
        return Err(CoreError::new(
            "refused",
            "Those changes were applied already.",
        ));
    }
    let summary = ask_tools::summary(&staged);
    let label = format!("Claude: {summary}");
    let mut failed: Vec<Value> = Vec::new();
    let mut applied = 0usize;
    let mut made: HashMap<String, String> = HashMap::new();
    let (_, step) = batch::one_step(i, &label, false, || {
        for s in &staged {
            match make(i, s, &mut made) {
                Ok(()) => applied += 1,
                Err(message) => failed.push(json!({"line": s.line, "message": message})),
            }
        }
    })?;
    forget_if_done(i, &run);
    Ok(json!({
        "applied": applied,
        "failed": failed,
        "summary": summary,
        "undo": step.undo,
        "steps": if step.txn.is_some() || step.entries <= 1 { 1 } else { step.entries },
    }))
}

/// `ask.applyOutward {id, index}`: one change that leaves this Mac, made
/// because the person answered yes to that one.
fn apply_outward(i: &Inner, a: &Args) -> Result<Value, CoreError> {
    let id = a.str("id")?;
    let index = a
        .opt_usize("index")?
        .ok_or_else(|| CoreError::new("bad_args", "ask.applyOutward needs index, which one."))?;
    let run = i
        .ask
        .run(id)
        .ok_or_else(|| CoreError::new("refused", "That isn't waiting any more. Ask again."))?;
    let s = {
        let mut outward = lock(&run.outward);
        let Some(s) = outward.get(index).cloned() else {
            return Err(CoreError::new("refused", "That isn't waiting any more."));
        };
        if s.cmd.is_empty() {
            return Err(CoreError::new("refused", "That was done already."));
        }
        // Done once: the same click again finds nothing to do.
        outward[index].cmd.clear();
        s
    };
    let result = ask_tools::invoke(i, &s.cmd, &s.args)?;
    forget_if_done(i, &run);
    Ok(
        json!({"done": true, "line": s.line, "undo": result["undo"], "sentence": result["sentence"]}),
    )
}

/// A run with nothing left waiting is let go.
fn forget_if_done(i: &Inner, run: &Arc<Run>) {
    let left =
        !lock(&run.staged).is_empty() || lock(&run.outward).iter().any(|s| !s.cmd.is_empty());
    if !left {
        lock(&i.ask.waiting).retain(|r| r.id != run.id);
    }
}

pub(crate) fn invoke(inner: &Arc<Inner>, cmd: &str, a: &Args) -> Result<Value, CoreError> {
    let i: &Inner = inner;
    match cmd {
        "ask.search" => {
            let kinds = a.opt_strings("kinds")?;
            ask_index::search(
                i,
                a.opt_str("q").unwrap_or(""),
                a.opt_usize("limit")?.unwrap_or(12),
                &kinds,
            )
        }
        "ask.status" => Ok(match claude_cli::binary(i) {
            Ok(_) => json!({"available": true}),
            Err(e) => json!({"available": false, "reason": e.message}),
        }),
        "ask.tools" => Ok(json!({
            "tools": ask_tools::TOOLS.iter().map(|t| json!({
                "name": t.name,
                "effect": match t.effect {
                    ask_tools::Effect::Reads => "reads",
                    ask_tools::Effect::Changes => "changes",
                    ask_tools::Effect::LeavesThisMac => "leaves this Mac",
                },
                "description": t.description,
            })).collect::<Vec<_>>(),
        })),
        "ask.send" => send(inner, a),
        "ask.apply" => apply(i, a),
        "ask.applyOutward" => apply_outward(i, a),
        "ask.cancel" => {
            let id = a.str("id")?;
            if let Some(run) = lock(&i.ask.asking).values().find(|r| r.id == id) {
                run.cancelled.store(true, Ordering::SeqCst);
            }
            Ok(json!({}))
        }
        "ask.discard" => {
            let id = a.str("id")?;
            lock(&i.ask.waiting).retain(|r| r.id != id);
            Ok(json!({}))
        }
        _ => Err(CoreError::new(
            "unknown_command",
            format!("There is no command called '{cmd}'."),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_answers_wikipedia_links_are_found_once_each() {
        let answer = "A transform. Wikipedia: [Fourier transform](wiki://Fourier_transform), [Erdős](wiki://Erd%C5%91s%E2%80%93R%C3%A9nyi_model) and [again](wiki://Fourier transform). [Not this](learn://task/1)";
        assert_eq!(
            wiki_links(answer),
            ["Fourier transform", "Erdős–Rényi model"]
        );
        assert!(wiki_links("No links here ](wiki://").is_empty());
    }

    #[test]
    fn what_claude_wrote_before_a_tool_call_is_part_of_the_answer() {
        let out = [
            r#"{"type":"system","subtype":"init"}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hm"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"A Fourier transform splits a signal into frequencies."},{"type":"tool_use","name":"mcp__learn__wiki_search","input":{}}]}}"#,
            r#"{"type":"user","message":{"content":[{"type":"tool_result","content":"x"}]}}"#,
            r#"{"type":"assistant","message":{"content":[{"type":"text","text":"Wikipedia: [Fourier transform](wiki://Fourier transform)"}]}}"#,
            r#"{"type":"result","subtype":"success","is_error":false,"result":"Wikipedia: [Fourier transform](wiki://Fourier transform)"}"#,
            "not json at all",
        ]
        .join("\n");
        let e = read_stream(&out);
        assert_eq!(e["is_error"], false);
        assert_eq!(
            e["said"],
            "A Fourier transform splits a signal into frequencies.\n\nWikipedia: [Fourier transform](wiki://Fourier transform)"
        );
        // One object and nothing else reads as that object.
        let one = read_stream(r#"{"type":"result","is_error":true,"result":"Please run /login"}"#);
        assert_eq!(
            (one["is_error"].clone(), one["said"].clone()),
            (json!(true), Value::Null)
        );
        assert!(read_stream("").is_null());
    }

    #[test]
    fn stand_in_ids_are_swapped_wherever_they_are() {
        let made: HashMap<String, String> = [("new:1".to_string(), "01REAL".to_string())].into();
        let args =
            json!({"taskId": "new:1", "edits": [{"row": "new:1", "value": "new:12"}], "n": 1});
        assert_eq!(
            substitute(&args, &made),
            json!({"taskId": "01REAL", "edits": [{"row": "01REAL", "value": "new:12"}], "n": 1})
        );
    }

    #[test]
    fn the_prompt_carries_the_screen_and_the_last_turns() {
        let p = user_prompt(
            " move these to Friday ",
            &json!({"tab": "tasks", "selection": [{"table": "task", "id": "t1"}]}),
            &json!([{"prompt": "what's due", "answer": "Two things.", "applied": false}]),
        );
        assert!(p.contains("\"tab\": \"tasks\""));
        assert!(
            p.contains("They asked: what's due\nYou answered: Two things.\n(They have not applied")
        );
        assert!(p.ends_with("Their request:\nmove these to Friday\n"));
        assert_eq!(
            user_prompt("hi", &Value::Null, &Value::Null),
            "Their request:\nhi\n"
        );
    }
}
