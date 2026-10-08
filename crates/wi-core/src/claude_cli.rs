//! Asking Claude from the core: one run of the Claude Code command line
//! (`claude -p`), with the prompt on stdin and nothing it can use but the
//! tools the caller names. The app holds no key: the run is the person's
//! own Claude Code, signed in as them, with the connectors they gave it.
//!
//! Two kinds of job use it. A text job (a syllabus, a batch of tasks to
//! score) names no tools, so the run can only answer. A mail job names a
//! handful of MCP tools, and the built-in ones (shell, files, the web) are
//! switched off for every run, so a prompt can never reach past its list.
//!
//! The binary is `Config::claude`, else `WI_WWAV_CLAUDE`, else `claude` on the
//! PATH, else the
//! places an installer puts it: an app opened from the Finder has a bare
//! PATH. Tests point `WI_WWAV_CLAUDE` at a script.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use crate::{CoreError, Inner};

/// One question for Claude.
pub(crate) struct Ask<'a> {
    pub prompt: &'a str,
    /// The only tools the run may call, by their full names
    /// (`mcp__wi-wwav__list_mail`). Empty: no tools at all.
    pub allowed_tools: &'a [&'a str],
    /// A JSON Schema the answer must fit; `run_json` returns that object.
    pub json_schema: Option<&'a Value>,
    /// `haiku`, `sonnet` or `opus`; None for the person's own default. A
    /// job run often, such as reading mail, names the smallest that does it.
    pub model: Option<&'a str>,
    pub timeout: Duration,
}

fn error(sentence: impl Into<String>) -> CoreError {
    CoreError::new("claude", sentence)
}

/// Where the command line is, or the sentence for why it can't be found.
pub(crate) fn binary(i: &Inner) -> Result<PathBuf, CoreError> {
    let named = i.claude.clone().or_else(|| {
        std::env::var_os("WI_WWAV_CLAUDE")
            .filter(|p| !p.is_empty())
            .map(PathBuf::from)
    });
    if let Some(path) = named {
        return if path.is_file() {
            Ok(path)
        } else {
            Err(error(format!(
                "WI_WWAV_CLAUDE points at {}, which isn't there.",
                path.display()
            )))
        };
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_default();
    let on_path = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect::<Vec<_>>())
        .unwrap_or_default();
    on_path
        .into_iter()
        .chain([
            home.join(".local/bin"),
            home.join(".claude/local"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
        ])
        .map(|dir| dir.join("claude"))
        .find(|p| p.is_file())
        .ok_or_else(|| {
            error("Claude Code isn't installed where Wi_WWAV can find it. Install it from claude.com/claude-code, then try again.")
        })
}

/// Runs `ask` and returns Claude's final text.
#[allow(dead_code)]
pub(crate) fn run(i: &Inner, ask: &Ask) -> Result<String, CoreError> {
    let envelope = run_at(&binary(i)?, ask, &|| i.closing())?;
    Ok(envelope["result"].as_str().unwrap_or_default().to_string())
}

/// Runs `ask` and returns the one JSON object it answered: the structured
/// output when `json_schema` was given, else the object its text holds.
pub(crate) fn run_json(i: &Inner, ask: &Ask) -> Result<Value, CoreError> {
    let envelope = run_at(&binary(i)?, ask, &|| i.closing())?;
    answer_json(&envelope)
}

fn answer_json(envelope: &Value) -> Result<Value, CoreError> {
    if envelope["structured_output"].is_object() {
        return Ok(envelope["structured_output"].clone());
    }
    // Without a schema the object is in the text, sometimes inside a fence.
    let text = envelope["result"].as_str().unwrap_or_default();
    let inside = match (text.find('{'), text.rfind('}')) {
        (Some(a), Some(b)) if a < b => &text[a..=b],
        _ => text,
    };
    match serde_json::from_str::<Value>(inside) {
        Ok(v) if v.is_object() => Ok(v),
        _ => Err(error(
            "Claude answered, but not with the JSON that was asked for. Try again.",
        )),
    }
}

/// Runs `ask` in `folder`, where Claude may read files and do nothing else:
/// a photo of a schedule, a notebook page Vision couldn't read. The folder
/// is one the caller made for the run and holds only what is to be read;
/// reading is the one built-in tool switched on, and a run can't read
/// outside the folder it stands in.
pub(crate) fn run_json_looking(i: &Inner, ask: &Ask, folder: &Path) -> Result<Value, CoreError> {
    let envelope = run_in(&binary(i)?, ask, &|| i.closing(), Some(folder))?;
    answer_json(&envelope)
}

/// The run itself: the result envelope `--output-format json` prints.
/// `stop` is asked while waiting, so closing the app ends the run.
fn run_at(binary: &Path, ask: &Ask, stop: &dyn Fn() -> bool) -> Result<Value, CoreError> {
    run_in(binary, ask, stop, None)
}

/// [`run_at`], in a folder of its own when there is something to look at.
fn run_in(
    binary: &Path,
    ask: &Ask,
    stop: &dyn Fn() -> bool,
    look: Option<&Path>,
) -> Result<Value, CoreError> {
    let mut command = Command::new(binary);
    command
        .arg("-p")
        .args(["--output-format", "json"])
        .arg("--no-session-persistence")
        // No skills either: their list alone is most of a run's cost.
        .arg("--disable-slash-commands")
        // No built-in tools, ever: no shell, no files, no web. A run that
        // has something to look at may read, and only that.
        .args(["--tools", if look.is_some() { "Read" } else { "" }]);
    if !ask.allowed_tools.is_empty() {
        command.args(["--allowedTools", &ask.allowed_tools.join(",")]);
    }
    if let Some(model) = ask.model {
        command.args(["--model", model]);
    }
    if let Some(schema) = ask.json_schema {
        command.args(["--json-schema", &schema.to_string()]);
    }
    // A folder of its own, so no project's instructions or settings apply.
    let cwd = look.map_or_else(std::env::temp_dir, Path::to_path_buf);
    let mut path = vec![binary.parent().unwrap_or(Path::new("/")).to_path_buf()];
    path.extend(
        ["/opt/homebrew/bin", "/usr/local/bin", "/usr/bin", "/bin"]
            .iter()
            .map(PathBuf::from),
    );
    if let Some(have) = std::env::var_os("PATH") {
        path.extend(std::env::split_paths(&have));
    }
    command
        .current_dir(cwd)
        .env("PATH", std::env::join_paths(path).unwrap_or_default())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|e| error(format!("Claude Code wouldn't start: {e}")))?;
    // On a thread, so a prompt longer than the pipe holds never stalls the
    // wait below. A run that exits before reading is reported by its status.
    if let Some(mut stdin) = child.stdin.take() {
        let prompt = ask.prompt.as_bytes().to_vec();
        std::thread::spawn(move || {
            let _ = stdin.write_all(&prompt);
        });
    }
    // Read both pipes on threads, so a long answer never fills one and stalls.
    let read = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut text = String::new();
            let _ = pipe.read_to_string(&mut text);
            text
        })
    };
    let out = read(Box::new(child.stdout.take().expect("stdout is piped")));
    let err = read(Box::new(child.stderr.take().expect("stderr is piped")));
    let deadline = Instant::now() + ask.timeout;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if stop() || Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(if stop() {
                    error("Claude was stopped because Wi_WWAV is closing.")
                } else {
                    let minutes = (ask.timeout.as_secs() / 60).max(1);
                    error(format!(
                        "Claude didn't answer within {minutes} {}. Try again.",
                        if minutes == 1 { "minute" } else { "minutes" }
                    ))
                });
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => return Err(error(format!("Claude Code stopped unexpectedly: {e}"))),
        }
    };
    let (out, err) = (
        out.join().unwrap_or_default(),
        err.join().unwrap_or_default(),
    );
    let envelope: Value = serde_json::from_str(out.trim()).unwrap_or(Value::Null);
    let said = envelope["result"].as_str().unwrap_or_default();
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
    {
        return Err(error(
            "Claude Code is signed out. Open a terminal, run claude, sign in, then try again.",
        ));
    }
    if !status.success() || envelope["is_error"] == true || !envelope.is_object() {
        let why = [said, err.trim(), out.trim()]
            .into_iter()
            .find(|s| !s.is_empty())
            .and_then(|s| s.lines().next())
            .unwrap_or("it gave no reason");
        let why: String = why.chars().take(200).collect();
        return Err(error(format!("Claude couldn't finish: {why}")));
    }
    Ok(envelope)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::os::unix::fs::PermissionsExt;

    /// A stand-in for the command line: a shell script.
    fn script(dir: &Path, body: &str) -> PathBuf {
        let path = dir.join("claude");
        std::fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        path
    }

    fn ask<'a>(tools: &'a [&'a str], schema: Option<&'a Value>, timeout: Duration) -> Ask<'a> {
        Ask {
            prompt: "What is asked.",
            allowed_tools: tools,
            json_schema: schema,
            model: None,
            timeout,
        }
    }

    const NEVER: &dyn Fn() -> bool = &|| false;

    #[test]
    fn the_prompt_goes_in_on_stdin_and_the_final_text_comes_back() {
        let dir = tempfile::tempdir().unwrap();
        let seen = dir.path().join("seen");
        let bin = script(
            dir.path(),
            &format!(
                "printf '%s\\n' \"$@\" > {seen}; cat >> {seen}; echo '{{\"type\":\"result\",\"is_error\":false,\"result\":\"Done: 3 threads.\"}}'",
                seen = seen.display()
            ),
        );
        let envelope = run_at(&bin, &ask(&[], None, Duration::from_secs(20)), NEVER).unwrap();
        assert_eq!(envelope["result"], "Done: 3 threads.");
        let seen = std::fs::read_to_string(&seen).unwrap();
        let lines: Vec<&str> = seen.lines().collect();
        // No tools named: the built-in ones are off and nothing is allowed.
        assert_eq!(
            lines,
            [
                "-p",
                "--output-format",
                "json",
                "--no-session-persistence",
                "--disable-slash-commands",
                "--tools",
                "",
                "What is asked."
            ]
        );
    }

    #[test]
    fn only_the_named_tools_are_allowed_and_a_schema_is_passed_on() {
        let dir = tempfile::tempdir().unwrap();
        let seen = dir.path().join("seen");
        let bin = script(
            dir.path(),
            &format!(
                "printf '%s\\n' \"$@\" > {}; cat > /dev/null; echo '{{\"is_error\":false,\"result\":\"\",\"structured_output\":{{\"minutes\":45}}}}'",
                seen.display()
            ),
        );
        let schema = json!({"type": "object"});
        let tools = [
            "mcp__wi-wwav__list_mail",
            "mcp__claude_ai_Gmail__search_threads",
        ];
        let envelope = run_at(
            &bin,
            &ask(&tools, Some(&schema), Duration::from_secs(20)),
            NEVER,
        )
        .unwrap();
        assert_eq!(answer_json(&envelope).unwrap(), json!({"minutes": 45}));
        let seen = std::fs::read_to_string(&seen).unwrap();
        assert!(seen.contains("--tools\n\n--allowedTools\nmcp__wi-wwav__list_mail,mcp__claude_ai_Gmail__search_threads\n"), "{seen}");
        assert!(
            seen.contains("--json-schema\n{\"type\":\"object\"}"),
            "{seen}"
        );
    }

    #[test]
    fn a_run_with_something_to_look_at_may_read_and_stands_in_that_folder() {
        let dir = tempfile::tempdir().unwrap();
        let folder = tempfile::tempdir().unwrap();
        let seen = dir.path().join("seen");
        let bin = script(
            dir.path(),
            &format!(
                "printf '%s\\n' \"$@\" > {seen}; pwd -P >> {seen}; cat > /dev/null; echo '{{\"is_error\":false,\"result\":\"\"}}'",
                seen = seen.display()
            ),
        );
        run_in(
            &bin,
            &ask(&[], None, Duration::from_secs(20)),
            NEVER,
            Some(folder.path()),
        )
        .unwrap();
        let seen = std::fs::read_to_string(&seen).unwrap();
        assert!(seen.contains("--tools\nRead\n"), "{seen}");
        let stood = seen.lines().last().unwrap();
        assert_eq!(
            Path::new(stood).canonicalize().unwrap(),
            folder.path().canonicalize().unwrap()
        );
    }

    #[test]
    fn json_in_the_text_is_read_fenced_or_not_and_anything_else_is_refused() {
        let text = |t: &str| json!({"is_error": false, "result": t});
        assert_eq!(answer_json(&text("{\"a\": 1}")).unwrap(), json!({"a": 1}));
        assert_eq!(
            answer_json(&text("Here it is:\n```json\n{\"a\": 1}\n```")).unwrap(),
            json!({"a": 1})
        );
        for bad in ["I couldn't read the syllabus.", "[1, 2]", ""] {
            assert_eq!(
                answer_json(&text(bad)).unwrap_err().message,
                "Claude answered, but not with the JSON that was asked for. Try again."
            );
        }
    }

    #[test]
    fn a_failure_is_one_plain_sentence() {
        let dir = tempfile::tempdir().unwrap();
        let run = |body: &str, timeout: u64| {
            let bin = script(dir.path(), body);
            let e =
                run_at(&bin, &ask(&[], None, Duration::from_millis(timeout)), NEVER).unwrap_err();
            assert_eq!(e.code, "claude");
            e.message
        };
        assert_eq!(
            run("cat > /dev/null; sleep 30", 300),
            "Claude didn't answer within 1 minute. Try again."
        );
        assert_eq!(
            run(
                "cat > /dev/null; echo 'Invalid API key · Please run /login'; exit 1",
                20_000
            ),
            "Claude Code is signed out. Open a terminal, run claude, sign in, then try again."
        );
        assert_eq!(
            run(
                "cat > /dev/null; echo 'the network is down' >&2; exit 2",
                20_000
            ),
            "Claude couldn't finish: the network is down"
        );
        assert_eq!(
            run("cat > /dev/null; echo '{\"is_error\":true,\"result\":\"Credit balance is too low\"}'", 20_000),
            "Claude couldn't finish: Credit balance is too low"
        );
        assert_eq!(
            run("cat > /dev/null; echo 'not json at all'", 20_000),
            "Claude couldn't finish: not json at all"
        );
    }

    #[test]
    fn closing_the_app_stops_the_run() {
        let dir = tempfile::tempdir().unwrap();
        let bin = script(dir.path(), "cat > /dev/null; sleep 30");
        let started = Instant::now();
        let e = run_at(&bin, &ask(&[], None, Duration::from_secs(60)), &|| true).unwrap_err();
        assert_eq!(e.message, "Claude was stopped because Wi_WWAV is closing.");
        assert!(started.elapsed() < Duration::from_secs(5));
    }
}
