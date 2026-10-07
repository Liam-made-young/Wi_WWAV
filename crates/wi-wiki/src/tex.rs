//! A formula's TeX as a line of plain text. Wikipedia serves each formula as
//! MathML with its TeX beside it; the Wiki tab shows text only, so `\alpha`
//! becomes α, `x^{2}` becomes x², and `\frac{a}{b}` becomes a/b. What has no
//! plain form is left as it was written, so nothing is lost.

const SYMBOLS: &[(&str, &str)] = &[
    ("alpha", "α"),
    ("beta", "β"),
    ("gamma", "γ"),
    ("delta", "δ"),
    ("epsilon", "ε"),
    ("varepsilon", "ε"),
    ("zeta", "ζ"),
    ("eta", "η"),
    ("theta", "θ"),
    ("vartheta", "ϑ"),
    ("iota", "ι"),
    ("kappa", "κ"),
    ("lambda", "λ"),
    ("mu", "μ"),
    ("nu", "ν"),
    ("xi", "ξ"),
    ("pi", "π"),
    ("varpi", "ϖ"),
    ("rho", "ρ"),
    ("varrho", "ϱ"),
    ("sigma", "σ"),
    ("varsigma", "ς"),
    ("tau", "τ"),
    ("upsilon", "υ"),
    ("phi", "φ"),
    ("varphi", "φ"),
    ("chi", "χ"),
    ("psi", "ψ"),
    ("omega", "ω"),
    ("Gamma", "Γ"),
    ("Delta", "Δ"),
    ("Theta", "Θ"),
    ("Lambda", "Λ"),
    ("Xi", "Ξ"),
    ("Pi", "Π"),
    ("Sigma", "Σ"),
    ("Upsilon", "Υ"),
    ("Phi", "Φ"),
    ("Psi", "Ψ"),
    ("Omega", "Ω"),
    ("infty", "∞"),
    ("partial", "∂"),
    ("nabla", "∇"),
    ("int", "∫"),
    ("iint", "∬"),
    ("iiint", "∭"),
    ("oint", "∮"),
    ("sum", "∑"),
    ("prod", "∏"),
    ("cdot", "·"),
    ("cdots", "⋯"),
    ("ldots", "…"),
    ("dots", "…"),
    ("dotsb", "⋯"),
    ("vdots", "⋮"),
    ("ddots", "⋱"),
    ("times", "×"),
    ("div", "÷"),
    ("pm", "±"),
    ("mp", "∓"),
    ("leq", "≤"),
    ("le", "≤"),
    ("geq", "≥"),
    ("ge", "≥"),
    ("neq", "≠"),
    ("ne", "≠"),
    ("approx", "≈"),
    ("equiv", "≡"),
    ("sim", "∼"),
    ("simeq", "≃"),
    ("cong", "≅"),
    ("propto", "∝"),
    ("triangleq", "≜"),
    ("doteq", "≐"),
    ("coloneqq", "≔"),
    ("ll", "≪"),
    ("gg", "≫"),
    ("to", "→"),
    ("rightarrow", "→"),
    ("leftarrow", "←"),
    ("gets", "←"),
    ("leftrightarrow", "↔"),
    ("Rightarrow", "⇒"),
    ("Leftarrow", "⇐"),
    ("Leftrightarrow", "⇔"),
    ("implies", "⟹"),
    ("iff", "⟺"),
    ("mapsto", "↦"),
    ("longmapsto", "⟼"),
    ("longrightarrow", "⟶"),
    ("uparrow", "↑"),
    ("downarrow", "↓"),
    ("in", "∈"),
    ("notin", "∉"),
    ("ni", "∋"),
    ("subset", "⊂"),
    ("subseteq", "⊆"),
    ("supset", "⊃"),
    ("supseteq", "⊇"),
    ("cup", "∪"),
    ("cap", "∩"),
    ("setminus", "∖"),
    ("emptyset", "∅"),
    ("varnothing", "∅"),
    ("forall", "∀"),
    ("exists", "∃"),
    ("neg", "¬"),
    ("lnot", "¬"),
    ("land", "∧"),
    ("wedge", "∧"),
    ("lor", "∨"),
    ("vee", "∨"),
    ("oplus", "⊕"),
    ("otimes", "⊗"),
    ("circ", "∘"),
    ("ast", "∗"),
    ("star", "⋆"),
    ("bullet", "•"),
    ("dagger", "†"),
    ("perp", "⊥"),
    ("parallel", "∥"),
    ("angle", "∠"),
    ("langle", "⟨"),
    ("rangle", "⟩"),
    ("lfloor", "⌊"),
    ("rfloor", "⌋"),
    ("lceil", "⌈"),
    ("rceil", "⌉"),
    ("lbrace", "{"),
    ("rbrace", "}"),
    ("lVert", "‖"),
    ("rVert", "‖"),
    ("Vert", "‖"),
    ("lvert", "|"),
    ("rvert", "|"),
    ("vert", "|"),
    ("mid", "|"),
    ("hbar", "ℏ"),
    ("ell", "ℓ"),
    ("Re", "ℜ"),
    ("Im", "ℑ"),
    ("aleph", "ℵ"),
    ("prime", "′"),
    ("degree", "°"),
    ("triangle", "△"),
    ("square", "□"),
    ("Box", "□"),
    ("therefore", "∴"),
    ("because", "∵"),
    ("colon", ":"),
    ("backslash", "\\"),
    ("quad", "  "),
    ("qquad", "    "),
    ("lim", "lim"),
    ("sup", "sup"),
    ("inf", "inf"),
    ("max", "max"),
    ("min", "min"),
    ("sin", "sin"),
    ("cos", "cos"),
    ("tan", "tan"),
    ("cot", "cot"),
    ("sec", "sec"),
    ("csc", "csc"),
    ("sinh", "sinh"),
    ("cosh", "cosh"),
    ("tanh", "tanh"),
    ("arcsin", "arcsin"),
    ("arccos", "arccos"),
    ("arctan", "arctan"),
    ("log", "log"),
    ("ln", "ln"),
    ("exp", "exp"),
    ("det", "det"),
    ("dim", "dim"),
    ("ker", "ker"),
    ("deg", "deg"),
    ("gcd", "gcd"),
    ("arg", "arg"),
    ("Pr", "Pr"),
    ("bmod", " mod "),
    ("pmod", " mod "),
    ("mod", " mod "),
];

/// Commands that only style what follows or wrap one group: dropped, the
/// group's own content kept.
const TRANSPARENT: &[&str] = &[
    "displaystyle",
    "textstyle",
    "scriptstyle",
    "scriptscriptstyle",
    "left",
    "right",
    "big",
    "Big",
    "bigg",
    "Bigg",
    "bigl",
    "bigr",
    "Bigl",
    "Bigr",
    "biggl",
    "biggr",
    "Biggl",
    "Biggr",
    "mathrm",
    "mathit",
    "mathbf",
    "boldsymbol",
    "bm",
    "mathsf",
    "mathtt",
    "mathnormal",
    "text",
    "textrm",
    "textit",
    "textbf",
    "textsf",
    "texttt",
    "operatorname",
    "mbox",
    "hbox",
    "rm",
    "it",
    "bf",
    "limits",
    "nolimits",
    "nonumber",
    "notag",
    "tfrac_",
    "strut",
    "hfill",
    "smash",
    "phantom_",
    "mathstrut",
    "mathord",
    "mathbin",
    "mathrel",
    "mathop",
    "mathopen",
    "mathclose",
    "mathpunct",
    "mathinner",
    "underbrace",
    "overbrace",
    "boxed",
    "color_",
    "hline",
];

const DOUBLE_STRUCK: &[(char, char)] = &[
    ('R', 'ℝ'),
    ('C', 'ℂ'),
    ('N', 'ℕ'),
    ('Z', 'ℤ'),
    ('Q', 'ℚ'),
    ('H', 'ℍ'),
    ('P', 'ℙ'),
    ('E', '𝔼'),
    ('F', '𝔽'),
    ('T', '𝕋'),
    ('K', '𝕂'),
    ('D', '𝔻'),
    ('S', '𝕊'),
];

const CALLIGRAPHIC: &[(char, char)] = &[
    ('F', 'ℱ'),
    ('L', 'ℒ'),
    ('H', 'ℋ'),
    ('S', '𝒮'),
    ('B', 'ℬ'),
    ('M', 'ℳ'),
    ('E', 'ℰ'),
    ('R', 'ℛ'),
    ('P', '𝒫'),
    ('O', '𝒪'),
    ('D', '𝒟'),
    ('A', '𝒜'),
    ('C', '𝒞'),
    ('N', '𝒩'),
    ('T', '𝒯'),
    ('G', '𝒢'),
    ('I', 'ℐ'),
    ('J', '𝒥'),
    ('K', '𝒦'),
    ('U', '𝒰'),
    ('V', '𝒱'),
    ('W', '𝒲'),
    ('X', '𝒳'),
    ('Y', '𝒴'),
    ('Z', '𝒵'),
    ('Q', '𝒬'),
];

const SUPERSCRIPT: &[(char, char)] = &[
    ('0', '⁰'),
    ('1', '¹'),
    ('2', '²'),
    ('3', '³'),
    ('4', '⁴'),
    ('5', '⁵'),
    ('6', '⁶'),
    ('7', '⁷'),
    ('8', '⁸'),
    ('9', '⁹'),
    ('+', '⁺'),
    ('-', '⁻'),
    ('−', '⁻'),
    ('=', '⁼'),
    ('(', '⁽'),
    (')', '⁾'),
    ('n', 'ⁿ'),
    ('i', 'ⁱ'),
    ('a', 'ᵃ'),
    ('b', 'ᵇ'),
    ('c', 'ᶜ'),
    ('d', 'ᵈ'),
    ('e', 'ᵉ'),
    ('f', 'ᶠ'),
    ('g', 'ᵍ'),
    ('h', 'ʰ'),
    ('j', 'ʲ'),
    ('k', 'ᵏ'),
    ('l', 'ˡ'),
    ('m', 'ᵐ'),
    ('o', 'ᵒ'),
    ('p', 'ᵖ'),
    ('r', 'ʳ'),
    ('s', 'ˢ'),
    ('t', 'ᵗ'),
    ('u', 'ᵘ'),
    ('v', 'ᵛ'),
    ('w', 'ʷ'),
    ('x', 'ˣ'),
    ('y', 'ʸ'),
    ('z', 'ᶻ'),
    ('T', 'ᵀ'),
    ('′', '′'),
    ('*', '*'),
    ('∗', '*'),
];

const SUBSCRIPT: &[(char, char)] = &[
    ('0', '₀'),
    ('1', '₁'),
    ('2', '₂'),
    ('3', '₃'),
    ('4', '₄'),
    ('5', '₅'),
    ('6', '₆'),
    ('7', '₇'),
    ('8', '₈'),
    ('9', '₉'),
    ('+', '₊'),
    ('-', '₋'),
    ('−', '₋'),
    ('=', '₌'),
    ('(', '₍'),
    (')', '₎'),
    ('a', 'ₐ'),
    ('e', 'ₑ'),
    ('h', 'ₕ'),
    ('i', 'ᵢ'),
    ('j', 'ⱼ'),
    ('k', 'ₖ'),
    ('l', 'ₗ'),
    ('m', 'ₘ'),
    ('n', 'ₙ'),
    ('o', 'ₒ'),
    ('p', 'ₚ'),
    ('r', 'ᵣ'),
    ('s', 'ₛ'),
    ('t', 'ₜ'),
    ('u', 'ᵤ'),
    ('v', 'ᵥ'),
    ('x', 'ₓ'),
];

fn map_all(s: &str, table: &[(char, char)]) -> Option<String> {
    s.chars()
        .map(|c| table.iter().find(|(k, _)| *k == c).map(|(_, v)| *v))
        .collect()
}

/// Whether `s` reads as one thing, so a fraction needn't bracket it.
fn simple(s: &str) -> bool {
    !s.is_empty()
        && !s
            .chars()
            .any(|c| c.is_whitespace() || "+-−=<>≤≥·×/∫∑,".contains(c))
}

struct Reader {
    c: Vec<char>,
    i: usize,
}

impl Reader {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }

    fn skip_spaces(&mut self) {
        while self.peek().is_some_and(char::is_whitespace) {
            self.i += 1;
        }
    }

    fn command_name(&mut self) -> String {
        let mut name = String::new();
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphabetic() {
                name.push(ch);
                self.i += 1;
            } else {
                break;
            }
        }
        if name.is_empty() {
            if let Some(ch) = self.peek() {
                name.push(ch);
                self.i += 1;
            }
        } else {
            // TeX swallows the space after a command's name.
            self.skip_spaces();
        }
        name
    }

    /// One argument: a braced group, a command with its own arguments, or
    /// one character.
    fn arg(&mut self) -> String {
        self.skip_spaces();
        match self.peek() {
            Some('{') => {
                self.i += 1;
                let s = self.seq(Some('}'));
                s.trim().to_string()
            }
            Some('\\') => {
                self.i += 1;
                self.command()
            }
            Some(ch) => {
                self.i += 1;
                ch.to_string()
            }
            None => String::new(),
        }
    }

    /// An optional argument in square brackets, as `\sqrt[3]{x}` has.
    fn optional(&mut self) -> Option<String> {
        self.skip_spaces();
        if self.peek() != Some('[') {
            return None;
        }
        self.i += 1;
        Some(self.seq(Some(']')).trim().to_string())
    }

    fn command(&mut self) -> String {
        let name = self.command_name();
        match name.as_str() {
            "," | ";" | ":" | ">" | " " | "~" => " ".to_string(),
            "!" => String::new(),
            "\\" => "; ".to_string(),
            "{" | "}" | "%" | "$" | "#" | "&" | "_" | "|" => {
                if name == "|" {
                    "‖".to_string()
                } else {
                    name
                }
            }
            "frac" | "dfrac" | "tfrac" | "cfrac" | "binom" => {
                let (a, b) = (self.arg(), self.arg());
                if name == "binom" {
                    return format!("C({a}, {b})");
                }
                let wrap = |s: String| if simple(&s) { s } else { format!("({s})") };
                format!("{}/{}", wrap(a), wrap(b))
            }
            "sqrt" => {
                let root = self.optional();
                let x = self.arg();
                let sign = match root.as_deref() {
                    Some("3") => "∛",
                    Some("4") => "∜",
                    _ => "√",
                };
                if simple(&x) && x.chars().count() <= 2 {
                    format!("{sign}{x}")
                } else {
                    format!("{sign}({x})")
                }
            }
            "hat" | "widehat" | "tilde" | "widetilde" | "bar" | "overline" | "vec" | "dot"
            | "ddot" | "check" | "breve" | "acute" | "grave" | "underline" => {
                let x = self.arg();
                let mark = match name.as_str() {
                    "hat" | "widehat" => '\u{0302}',
                    "tilde" | "widetilde" => '\u{0303}',
                    "bar" | "overline" => '\u{0304}',
                    "vec" => '\u{20D7}',
                    "dot" => '\u{0307}',
                    "ddot" => '\u{0308}',
                    "check" => '\u{030C}',
                    "breve" => '\u{0306}',
                    "acute" => '\u{0301}',
                    "grave" => '\u{0300}',
                    _ => '\u{0332}',
                };
                if x.chars().count() == 1 {
                    format!("{x}{mark}")
                } else {
                    let word = match name.as_str() {
                        "bar" | "overline" => "‾",
                        "hat" | "widehat" => "^",
                        "tilde" | "widetilde" => "~",
                        _ => "",
                    };
                    format!("({x}){word}")
                }
            }
            "mathbb" | "mathcal" | "mathscr" | "mathfrak" => {
                let x = self.arg();
                let table = if name == "mathbb" {
                    DOUBLE_STRUCK
                } else {
                    CALLIGRAPHIC
                };
                map_all(&x, table).unwrap_or(x)
            }
            "begin" | "end" => {
                let _ = self.arg();
                // An alignment's column spec, `{rcl}`, follows some of them.
                String::new()
            }
            "color" | "phantom" | "hphantom" | "vphantom" | "label" | "tag" => {
                let _ = self.arg();
                String::new()
            }
            "stackrel" | "overset" | "underset" => {
                let (over, base) = (self.arg(), self.arg());
                format!("{base}[{over}]")
            }
            "not" => {
                let next = self.arg();
                match next.as_str() {
                    "=" => "≠".to_string(),
                    "∈" => "∉".to_string(),
                    "⊂" => "⊄".to_string(),
                    other => format!("¬{other}"),
                }
            }
            _ => {
                if TRANSPARENT.contains(&name.as_str()) {
                    return String::new();
                }
                match SYMBOLS.iter().find(|(k, _)| *k == name) {
                    Some((_, v)) => v.to_string(),
                    None => format!("\\{name}"),
                }
            }
        }
    }

    fn script(&mut self, table: &[(char, char)], mark: char) -> String {
        let s = self.arg();
        if s.is_empty() {
            return String::new();
        }
        match map_all(&s, table) {
            Some(mapped) => mapped,
            None if s.chars().count() == 1 => format!("{mark}{s}"),
            None => format!("{mark}({s})"),
        }
    }

    fn seq(&mut self, until: Option<char>) -> String {
        let mut out = String::new();
        while let Some(ch) = self.peek() {
            if Some(ch) == until {
                self.i += 1;
                break;
            }
            self.i += 1;
            match ch {
                '\\' => out.push_str(&self.command()),
                '{' => {
                    let inner = self.seq(Some('}'));
                    out.push_str(&inner);
                }
                '}' => {}
                '^' | '_' => {
                    // A script belongs to what it follows, with no gap.
                    while out.ends_with(' ') {
                        out.pop();
                    }
                    let (table, mark) = if ch == '^' {
                        (SUPERSCRIPT, '^')
                    } else {
                        (SUBSCRIPT, '_')
                    };
                    out.push_str(&self.script(table, mark));
                }
                '&' => out.push(' '),
                '~' => out.push(' '),
                '-' => out.push('−'),
                '\'' => out.push('′'),
                c if c.is_whitespace() => {
                    if !out.ends_with(' ') {
                        out.push(' ');
                    }
                }
                c => out.push(c),
            }
        }
        out
    }
}

/// TeX as one line of plain text.
pub fn plain(tex: &str) -> String {
    let mut r = Reader {
        c: tex.chars().collect(),
        i: 0,
    };
    let raw = r.seq(None);
    // Tidy the spacing TeX leaves behind.
    let mut out = String::with_capacity(raw.len());
    for word in raw.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        out.push_str(word);
    }
    // Relations read better with room around them: "a ≤ b", "f : ℝ → ℂ".
    let mut spaced = String::with_capacity(out.len() + 8);
    for ch in out.chars() {
        if "=≤≥≠≈≡→↦⇒⟹⟺∈∉⊂⊆∼≃≅∝≪≫".contains(ch) {
            if !spaced.ends_with(' ') && !spaced.is_empty() {
                spaced.push(' ');
            }
            spaced.push(ch);
            spaced.push(' ');
        } else if ch == ' ' && spaced.ends_with(' ') {
        } else {
            spaced.push(ch);
        }
    }
    let mut out = spaced.trim().to_string();
    for (from, to) in [
        ("( ", "("),
        (" )", ")"),
        (" ,", ","),
        (" .", "."),
        ("{ ", "{"),
        (" }", "}"),
    ] {
        out = out.replace(from, to);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::plain;

    #[test]
    fn formulas_read_as_text() {
        for (tex, want) in [
            (r"{\displaystyle f(x)}", "f(x)"),
            (r"{\displaystyle \alpha +\beta ^{2}}", "α+β²"),
            (r"x_{n+1}", "xₙ₊₁"),
            (r"\frac{a}{b}", "a/b"),
            (r"\frac{a+b}{2}", "(a+b)/2"),
            (r"\sqrt{x}", "√x"),
            (r"\sqrt[3]{x+1}", "∛(x+1)"),
            (r"\mathbb{R}^{n}", "ℝⁿ"),
            (r"\hat{f}(\xi )", "f̂(ξ)"),
            (r"a\leq b\neq c", "a ≤ b ≠ c"),
            (r"e^{-i2\pi \xi x}", "e^(−i2πξx)"),
            (r"\int _{-\infty }^{\infty }f(x)\,dx", "∫_(−∞)^∞f(x) dx"),
            (r"\operatorname {rect} (x)", "rect (x)"),
            (r"\text{if } x > 0", "if x > 0"),
            (
                r"{\displaystyle {\widehat {f}}(\xi )=\int _{-\infty }^{\infty }f(x)\ e^{-i2\pi \xi x}\,dx,\quad \forall \xi \in \mathbb {R} .}",
                "f̂(ξ) = ∫_(−∞)^∞f(x) e^(−i2πξx) dx, ∀ξ ∈ ℝ.",
            ),
        ] {
            assert_eq!(plain(tex), want, "{tex}");
        }
    }

    #[test]
    fn an_unknown_command_is_kept_and_nothing_fails() {
        assert_eq!(plain(r"\weird{x}"), r"\weirdx");
        for tex in [
            "", "{", "}", "\\", "^", "_", "\\frac", "\\sqrt[", "{{{", "a^", "\\hat", "\\not",
            "\\mathbb", "x_", "\\begin{", "\\\\\\",
        ] {
            let _ = plain(tex);
        }
        assert_eq!(
            plain("\\begin{aligned}x&=1\\\\y&=2\\end{aligned}"),
            "x = 1; y = 2"
        );
    }
}
