//! One token, one value, in every process (docs/SPEC.md 8.11; F7).
//!
//! Fails if: the Rust module, the C++ header (compiled and run) or the CSS
//! gives any token a value other than design/tokens.json's, in either
//! appearance; any of them has a token the file doesn't, or lacks one it
//! has; an output doesn't say it was generated from the file.

mod common;

use common::{leaves, parse_color, tokens, Leaf, Token, Val};
use std::collections::BTreeMap;
use std::process::Command;
use wwav_tokens::{Color, Themed};

/// (path with any [index], appearance) -> leaf
type Leaves = BTreeMap<(String, String), Leaf>;

fn expected() -> Leaves {
    let mut out = Leaves::new();
    for t in tokens() {
        for appearance in common::APPEARANCES {
            for (index, leaf) in leaves(t.get(appearance)) {
                out.insert((t.dotted() + &index, appearance.into()), leaf);
            }
        }
    }
    out
}

fn compare(what: &str, got: &Leaves) {
    let want = expected();
    let mut diffs = Vec::new();
    for (key, w) in &want {
        match got.get(key) {
            None => diffs.push(format!("{} ({}): missing", key.0, key.1)),
            Some(g) if !g.same(w) => {
                diffs.push(format!("{} ({}): {g:?}, the file says {w:?}", key.0, key.1))
            }
            _ => {}
        }
    }
    for key in got.keys().filter(|k| !want.contains_key(*k)) {
        diffs.push(format!("{} ({}): not in the file", key.0, key.1));
    }
    assert!(
        diffs.is_empty(),
        "{what} disagrees with design/tokens.json:\n{}",
        diffs.join("\n")
    );
}

// ---- Rust -------------------------------------------------------------------

fn themed<T: Clone>(t: &Themed<T>, f: impl Fn(T) -> Val) -> (Val, Val) {
    (f(t.light.clone()), f(t.dark.clone()))
}

#[test]
fn the_rust_module_matches_the_file() {
    use wwav_tokens::Token as T;
    let colors = |cs: &[Color]| Val::Colors(cs.to_vec());
    let mut got = Leaves::new();
    for (path, token) in wwav_tokens::ALL {
        let (light, dark) = match *token {
            T::Color(c) => (Val::Color(c), Val::Color(c)),
            T::ThemedColor(t) => themed(&t, Val::Color),
            T::Colors(cs) => (colors(cs), colors(cs)),
            T::ThemedColors(t) => themed(&t, colors),
            T::Number(n) => (Val::Number(n.into()), Val::Number(n.into())),
            T::Numbers(ns) => {
                let v = Val::Numbers(ns.iter().map(|&n| n.into()).collect());
                (v.clone(), v)
            }
            T::Names(ns) => {
                let v = Val::Names(ns.iter().map(|n| n.to_string()).collect());
                (v.clone(), v)
            }
            T::Curve(c) => {
                let v = Val::Curve([c.x1, c.y1, c.x2, c.y2].map(f64::from));
                (v.clone(), v)
            }
        };
        for (appearance, val) in [("light", light), ("dark", dark)] {
            for (index, leaf) in leaves(&val) {
                let key = (format!("{path}{index}"), appearance.to_string());
                assert!(got.insert(key, leaf).is_none(), "{path} listed twice");
            }
        }
    }
    compare("generated.rs", &got);
}

#[test]
fn the_rust_names_carry_their_units() {
    // A bare 140 could be seconds or frames; the name says which.
    assert_eq!(wwav_tokens::motion::BEAT_MS, 140.0);
    assert_eq!(wwav_tokens::space::S4_PX, 16.0);
    assert_eq!(wwav_tokens::r#type::text::LABEL_TRACKING_EM, 0.08);
    assert_eq!(wwav_tokens::r#type::desk::LINE_HEIGHT, 1.35);
    assert_eq!(
        wwav_tokens::desk::lcd::DIM.light,
        Color::rgb(0x4b, 0x50, 0x34)
    );
}

// ---- C++ --------------------------------------------------------------------

fn cpp_name(t: &Token) -> String {
    let mut parts: Vec<String> = t.path[..t.path.len() - 1].to_vec();
    let last = &t.path[t.path.len() - 1];
    let unit = match t.unit.as_deref() {
        Some("px") => "Px",
        Some("ms") => "Ms",
        Some("em") => "Em",
        Some(u) => panic!("unit {u}"),
        None => "",
    };
    parts.push(format!("k{}{}{unit}", last[..1].to_uppercase(), &last[1..]));
    format!("wwav::tokens::{}", parts.join("::"))
}

const PRINTER: &str = r#"
#include <cstdio>
#include "wwav_tokens.h"
namespace t = wwav::tokens;
static void color(const char* path, const char* app, int i, const t::Color& c) {
  std::printf("%s\t%s\t%d\tc\t%u %u %u %.9g\n", path, app, i, (unsigned)c.r, (unsigned)c.g,
              (unsigned)c.b, c.a);
}
static void colors(const char* path, const char* app, const t::List<t::Color>& l) {
  for (std::size_t i = 0; i < l.count; ++i) color(path, app, (int)i, l.items[i]);
}
static void number(const char* path, const char* app, int i, float n) {
  std::printf("%s\t%s\t%d\tn\t%.9g\n", path, app, i, n);
}
static void p(const char* path, const t::Color& c) {
  color(path, "light", -1, c); color(path, "dark", -1, c);
}
static void p(const char* path, const t::Themed<t::Color>& c) {
  color(path, "light", -1, c.light); color(path, "dark", -1, c.dark);
}
static void p(const char* path, const t::List<t::Color>& l) {
  colors(path, "light", l); colors(path, "dark", l);
}
static void p(const char* path, const t::Themed<t::List<t::Color>>& l) {
  colors(path, "light", l.light); colors(path, "dark", l.dark);
}
static void p(const char* path, float n) {
  number(path, "light", -1, n); number(path, "dark", -1, n);
}
static void p(const char* path, const t::List<float>& l) {
  for (std::size_t i = 0; i < l.count; ++i) {
    number(path, "light", (int)i, l.items[i]); number(path, "dark", (int)i, l.items[i]);
  }
}
static void p(const char* path, const t::List<const char*>& l) {
  for (std::size_t i = 0; i < l.count; ++i) {
    std::printf("%s\tlight\t%d\ts\t%s\n", path, (int)i, l.items[i]);
    std::printf("%s\tdark\t%d\ts\t%s\n", path, (int)i, l.items[i]);
  }
}
static void p(const char* path, const t::Curve& c) {
  const float n[4] = {c.x1, c.y1, c.x2, c.y2};
  for (int i = 0; i < 4; ++i) { number(path, "light", i, n[i]); number(path, "dark", i, n[i]); }
}
"#;

fn cpp_leaves(compiler: &str) -> Leaves {
    let dir = tempfile::tempdir().unwrap();
    let mut src = String::from(PRINTER);
    src += "int main() {\n";
    for t in tokens() {
        src += &format!("  p(\"{}\", {});\n", t.dotted(), cpp_name(&t));
    }
    src += "  return 0;\n}\n";
    let file = dir.path().join("print.cpp");
    std::fs::write(&file, src).unwrap();
    let exe = dir.path().join("print");
    let out = Command::new(compiler)
        .args(["-std=c++17", "-Wall", "-Wextra", "-Werror", "-I"])
        .arg(common::repo().join("engine/include"))
        .arg(&file)
        .arg("-o")
        .arg(&exe)
        .output()
        .unwrap_or_else(|e| panic!("can't run {compiler}: {e}"));
    assert!(
        out.status.success(),
        "{compiler} failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let run = Command::new(&exe).output().unwrap();
    assert!(run.status.success());
    let mut got = Leaves::new();
    for line in String::from_utf8(run.stdout).unwrap().lines() {
        let f: Vec<&str> = line.split('\t').collect();
        let index = if f[2] == "-1" {
            String::new()
        } else {
            format!("[{}]", f[2])
        };
        let leaf = match f[3] {
            "c" => {
                let n: Vec<f64> = f[4].split(' ').map(|n| n.parse().unwrap()).collect();
                Leaf::Color(Color::rgba(n[0] as u8, n[1] as u8, n[2] as u8, n[3] as f32))
            }
            "n" => Leaf::Number(f[4].parse().unwrap()),
            _ => Leaf::Name(f[4].to_string()),
        };
        got.insert((format!("{}{index}", f[0]), f[1].to_string()), leaf);
    }
    got
}

#[test]
fn the_cpp_header_matches_the_file_under_gcc() {
    compare("wwav_tokens.h (g++)", &cpp_leaves("g++"));
}

#[test]
fn the_cpp_header_matches_the_file_under_clang() {
    // The engine builds with Apple clang on the Mac.
    compare("wwav_tokens.h (clang++)", &cpp_leaves("clang++"));
}

// ---- CSS --------------------------------------------------------------------

fn kebab(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_uppercase() {
            out.push('-');
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn css_name(path: &[String]) -> String {
    let parts: Vec<String> = path.iter().map(|p| kebab(p)).collect();
    format!("--{}", parts.join("-"))
}

/// The custom properties of each rule in tokens.css, by selector.
fn css_blocks() -> BTreeMap<String, BTreeMap<String, String>> {
    let css = std::fs::read_to_string(common::repo().join("app/ui/src/styles/tokens.css")).unwrap();
    parse_css(&css)
}

/// Reads CSS as a browser does: a comment ends at its first `*/`, and a
/// declaration runs to the next `;` or `}`, whatever lines it spans. Anything
/// inside a rule that isn't `--name: value` fails, so text a browser would
/// read as a broken declaration can't hide.
fn parse_css(css: &str) -> BTreeMap<String, BTreeMap<String, String>> {
    let mut text = String::new();
    let mut rest = css;
    while let Some(open) = rest.find("/*") {
        text += &rest[..open];
        let close = rest[open + 2..]
            .find("*/")
            .unwrap_or_else(|| panic!("a comment never closes: {}", &rest[open..]));
        rest = &rest[open + 2 + close + 2..];
    }
    text += rest;

    let mut blocks: BTreeMap<String, BTreeMap<String, String>> = BTreeMap::new();
    let mut open: Vec<String> = Vec::new(); // the selectors of the rules we're in
    let mut held = String::new();
    for ch in text.chars() {
        match ch {
            '{' => {
                open.push(held.trim().to_string());
                held.clear();
            }
            ';' | '}' => {
                let decl = held.trim();
                if !decl.is_empty() {
                    let sel = open
                        .last()
                        .unwrap_or_else(|| panic!("outside any rule: {decl:?}"));
                    let (name, value) = decl
                        .split_once(':')
                        .unwrap_or_else(|| panic!("not a declaration in {sel}: {decl:?}"));
                    let name = name.trim();
                    let custom = name.strip_prefix("--").is_some_and(|n| {
                        !n.is_empty()
                            && n.chars()
                                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
                    });
                    assert!(custom, "not a custom property in {sel}: {decl:?}");
                    let block = blocks.entry(sel.clone()).or_default();
                    let old = block.insert(name.to_string(), value.trim().to_string());
                    assert!(old.is_none(), "{name} twice in {sel}");
                }
                held.clear();
                if ch == '}' {
                    open.pop().expect("a } with no rule to close");
                }
            }
            _ => held.push(ch),
        }
    }
    assert!(
        open.is_empty() && held.trim().is_empty(),
        "tokens.css ends inside a rule"
    );
    blocks
}

fn css_color(s: &str) -> Color {
    if let Some(c) = parse_color(s) {
        return c;
    }
    let inner = s
        .strip_prefix("rgb(")
        .and_then(|s| s.strip_suffix(')'))
        .unwrap_or_else(|| panic!("not a colour: {s}"));
    let (rgb, a) = inner.split_once(" / ").unwrap();
    let n: Vec<u8> = rgb.split(' ').map(|n| n.parse().unwrap()).collect();
    Color::rgba(n[0], n[1], n[2], a.parse().unwrap())
}

fn css_number(s: &str, unit: Option<&str>) -> f64 {
    let n = match unit {
        Some(u) => s.strip_suffix(u).unwrap_or_else(|| panic!("{s} lacks {u}")),
        None => s,
    };
    n.parse().unwrap_or_else(|_| panic!("not a number: {s}"))
}

/// A CSS value read back as the kind the file says the token is.
fn css_val(like: &Val, s: &str, unit: Option<&str>) -> Val {
    let items = || s.split(", ");
    match like {
        Val::Color(_) => Val::Color(css_color(s)),
        Val::Colors(_) => Val::Colors(items().map(css_color).collect()),
        Val::Number(_) => Val::Number(css_number(s, unit)),
        Val::Numbers(_) => Val::Numbers(items().map(|n| css_number(n, None)).collect()),
        Val::Names(_) => Val::Names(items().map(|n| n.trim_matches('"').to_string()).collect()),
        Val::Curve(_) => {
            let inner = s
                .strip_prefix("cubic-bezier(")
                .and_then(|s| s.strip_suffix(')'))
                .unwrap();
            let n: Vec<f64> = inner.split(", ").map(|n| n.parse().unwrap()).collect();
            Val::Curve(n.try_into().unwrap())
        }
    }
}

#[test]
fn the_css_is_read_as_a_browser_reads_it() {
    // A note holding "*/" closes its comment early. A browser then reads the
    // comment's tail as CSS and drops the declaration after it, so the reader
    // must not skip it as a comment line would.
    let broken = ":root {\n  /* heat: see heat/*/ for the tubes. */\n  --heat-cool: #4f9be6;\n}\n";
    assert!(
        std::panic::catch_unwind(|| parse_css(broken)).is_err(),
        "read a declaration a browser drops"
    );
    let fine = ":root {\n  /* heat: see heat/* / for the tubes. */\n  --heat-cool: #4f9be6;\n}\n";
    assert_eq!(parse_css(fine)[":root"]["--heat-cool"], "#4f9be6");
}

const LIGHT: &str = ":root";
const DARK: [&str; 2] = [
    ":root:not([data-appearance=\"light\"])",
    ":root[data-appearance=\"dark\"]",
];

#[test]
fn the_css_matches_the_file_in_both_appearances() {
    let blocks = css_blocks();
    let tokens = tokens();
    for dark in DARK {
        assert_eq!(blocks[dark], blocks[DARK[0]], "the two dark rules differ");
    }
    let mut got = Leaves::new();
    for t in &tokens {
        let name = css_name(&t.path);
        for (appearance, selector) in [("light", LIGHT), ("dark", DARK[1])] {
            let selector = if appearance == "dark" && !t.themed {
                LIGHT // the same in both: only :root sets it
            } else {
                selector
            };
            let value = blocks[selector]
                .get(&name)
                .unwrap_or_else(|| panic!("{name} missing from {selector}"))
                .clone();
            let val = css_val(t.get(appearance), &value, t.unit.as_deref());
            for (index, leaf) in leaves(&val) {
                got.insert((t.dotted() + &index, appearance.into()), leaf);
            }
        }
    }
    compare("tokens.css", &got);

    // The night's ink at each of its steps, as v4 wrote them (--ink-70).
    let ink = common::colors(&tokens, "night.ink", "light")[0];
    let steps = match &common::find(&tokens, "night.inkSteps").light {
        Val::Numbers(ns) => ns.clone(),
        other => panic!("{other:?}"),
    };
    for step in &steps {
        let name = format!("--night-ink-{:02}", (step * 100.0).round());
        let got = css_color(&blocks[LIGHT][&name]);
        assert_eq!((got.r, got.g, got.b), (ink.r, ink.g, ink.b), "{name}");
        assert!((f64::from(got.a) - step).abs() < 1e-6, "{name}");
    }

    // Nothing else: every property is a token or an ink step. (A gradient
    // is one list, never stop by stop: deck metal has three stops in light and
    // two in dark, and a third stop set only in light would leak into dark.)
    let mut known: Vec<String> = tokens.iter().map(|t| css_name(&t.path)).collect();
    known.extend(
        steps
            .iter()
            .map(|s| format!("--night-ink-{:02}", (s * 100.0).round())),
    );
    for (selector, block) in &blocks {
        for name in block.keys() {
            assert!(known.contains(name), "{name} in {selector} isn't a token");
        }
    }
    let themed = tokens.iter().filter(|t| t.themed).count();
    assert_eq!(
        blocks[DARK[0]].len(),
        themed,
        "only themed tokens in the dark rules"
    );
}

// ---- All three --------------------------------------------------------------

#[test]
fn every_output_says_where_it_comes_from() {
    for path in [
        "app/ui/src/styles/tokens.css",
        "crates/wwav-tokens/src/generated.rs",
        "engine/include/wwav_tokens.h",
    ] {
        let text = std::fs::read_to_string(common::repo().join(path)).unwrap();
        let head: String = text.lines().take(3).collect::<Vec<_>>().join("\n");
        assert!(
            head.contains("Generated from design/tokens.json by tools/tokens/compile.mjs"),
            "{path} doesn't say it's generated:\n{head}"
        );
    }
}
