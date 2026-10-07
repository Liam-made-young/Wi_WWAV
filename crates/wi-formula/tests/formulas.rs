//! Formulas worked out against two small tables, as the Database tab does.

use std::collections::BTreeMap;
use std::rc::Rc;

use wi_formula::{days_from_civil, Context, Error, Formula, Ref, Value};

fn day(y: i64, m: i64, d: i64) -> Value {
    Value::Date(days_from_civil(y, m, d))
}

fn n(x: f64) -> Value {
    Value::Number(x)
}

fn t(s: &str) -> Value {
    Value::text(s)
}

/// Tables as `name -> column -> values`, and the row a formula stands on.
struct Tables {
    tables: BTreeMap<&'static str, BTreeMap<&'static str, Vec<Value>>>,
    table: &'static str,
    row: usize,
}

impl Context for Tables {
    fn field(&self, name: &str) -> Option<Value> {
        let cols = &self.tables[self.table];
        cols.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| v[self.row].clone())
    }

    fn column(&self, table: &str, name: &str) -> Result<Rc<Vec<Value>>, Error> {
        let (_, cols) = self
            .tables
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(table))
            .ok_or_else(|| Error::reference(format!("There is no table called '{table}'.")))?;
        cols.iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| Rc::new(v.clone()))
            .ok_or_else(|| Error::reference(format!("{table} has no column called '{name}'.")))
    }

    fn today(&self) -> f64 {
        days_from_civil(2026, 10, 7)
    }

    fn now(&self) -> f64 {
        days_from_civil(2026, 10, 7) + 0.5
    }

    fn row_number(&self) -> usize {
        self.row + 1
    }
}

fn tables(table: &'static str, row: usize) -> Tables {
    let mut tasks = BTreeMap::new();
    tasks.insert(
        "Title",
        vec![
            t("Kanji quiz"),
            t("Lab report"),
            t("Essay draft"),
            t("Laundry"),
        ],
    );
    tasks.insert(
        "Course",
        vec![t("JPN 101"), t("PHY 204"), t("JPN 101"), Value::Blank],
    );
    tasks.insert("Est min", vec![n(45.0), n(120.0), n(90.0), Value::Blank]);
    tasks.insert(
        "Done",
        vec![
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(false),
        ],
    );
    tasks.insert(
        "Due",
        vec![
            day(2026, 10, 9),
            day(2026, 10, 6),
            day(2026, 10, 12),
            Value::Blank,
        ],
    );
    let mut courses = BTreeMap::new();
    courses.insert("Code", vec![t("JPN 101"), t("PHY 204")]);
    courses.insert("Name", vec![t("Elementary Japanese"), t("Physics II")]);
    courses.insert("Credits", vec![n(4.0), n(3.0)]);
    let mut focus = BTreeMap::new();
    focus.insert(
        "Course",
        vec![t("JPN 101"), t("JPN 101"), t("PHY 204"), t("JPN 101")],
    );
    focus.insert("Minutes", vec![n(25.0), n(50.0), n(25.0), n(25.0)]);
    focus.insert(
        "Started",
        vec![
            day(2026, 10, 5),
            day(2026, 10, 6),
            day(2026, 10, 6),
            day(2026, 9, 28),
        ],
    );
    let mut all = BTreeMap::new();
    all.insert("Tasks", tasks);
    all.insert("Courses", courses);
    all.insert("Focus sessions", focus);
    Tables {
        tables: all,
        table,
        row,
    }
}

fn eval_on(table: &'static str, row: usize, src: &str) -> Value {
    Formula::parse(src)
        .unwrap_or_else(|e| panic!("{src}: {e}"))
        .eval(&tables(table, row))
}

fn eval(src: &str) -> Value {
    eval_on("Tasks", 0, src)
}

fn code(v: Value) -> &'static str {
    match v {
        Value::Error(e) => e.code,
        other => panic!("not an error: {other:?}"),
    }
}

#[test]
fn arithmetic_follows_the_usual_order() {
    assert_eq!(eval("1 + 2 * 3"), n(7.0));
    assert_eq!(eval("=(1 + 2) * 3"), n(9.0));
    assert_eq!(eval("2 ^ 3 ^ 2"), n(512.0));
    assert_eq!(eval("-2 ^ 2"), n(4.0));
    assert_eq!(eval("10 / 4"), n(2.5));
    assert_eq!(eval("50%"), n(0.5));
    assert_eq!(eval("200 * 15%"), n(30.0));
    assert_eq!(eval("1e3 + .5"), n(1000.5));
    assert_eq!(eval("\"a\" & 1 & \"b\""), t("a1b"));
    assert_eq!(eval("\"He said \"\"hi\"\"\""), t("He said \"hi\""));
    assert_eq!(code(eval("1 / 0")), "#DIV/0!");
    assert_eq!(code(eval("\"abc\" + 1")), "#VALUE!");
}

#[test]
fn comparisons_ignore_case_and_read_blanks_as_empty() {
    assert_eq!(eval("\"JPN 101\" = \"jpn 101\""), Value::Bool(true));
    assert_eq!(eval("3 <> 4"), Value::Bool(true));
    assert_eq!(eval("2 >= 2"), Value::Bool(true));
    assert_eq!(eval("\"apple\" < \"Banana\""), Value::Bool(true));
    assert_eq!(eval_on("Tasks", 3, "[Course] = \"\""), Value::Bool(true));
    assert_eq!(eval_on("Tasks", 3, "[Est min] = 0"), Value::Bool(true));
}

#[test]
fn a_column_is_named_bare_or_in_brackets_in_any_case() {
    assert_eq!(eval("[Est min] / 60"), n(0.75));
    assert_eq!(eval("[est MIN] * 2"), n(90.0));
    assert_eq!(eval("Title"), t("Kanji quiz"));
    assert_eq!(
        eval("title & \" (\" & Course & \")\""),
        t("Kanji quiz (JPN 101)")
    );
    assert_eq!(
        eval_on("Tasks", 1, "IF(Done, \"Done\", \"Open\")"),
        t("Open")
    );
    assert_eq!(code(eval("[Nope] + 1")), "#REF!");
    assert_eq!(code(eval("Nope")), "#REF!");
}

#[test]
fn a_whole_column_feeds_the_totals() {
    assert_eq!(eval("SUM(Tasks[Est min])"), n(255.0));
    assert_eq!(eval("AVERAGE(Tasks[Est min])"), n(85.0));
    assert_eq!(eval("MIN(Tasks[Est min])"), n(45.0));
    assert_eq!(eval("MAX(Tasks[Est min], 500)"), n(500.0));
    assert_eq!(eval("COUNT(Tasks[Est min])"), n(3.0));
    assert_eq!(eval("COUNTA(Tasks[Course])"), n(3.0));
    assert_eq!(eval("COUNTBLANK(Tasks[Course])"), n(1.0));
    assert_eq!(eval("COUNTUNIQUE(Tasks[Course])"), n(2.0));
    assert_eq!(eval("MEDIAN(Tasks[Est min])"), n(90.0));
    assert_eq!(eval("ROUND(STDEV(Tasks[Est min]), 2)"), n(37.75));
    assert_eq!(eval("MAX(Tasks[Due])"), day(2026, 10, 12));
    // The dotted form, and a table name of two words.
    assert_eq!(eval("SUM(tasks.Done)"), n(0.0));
    assert_eq!(eval("SUM(Focus sessions[Minutes])"), n(125.0));
    assert_eq!(code(eval("SUM(Nowhere[Minutes])")), "#REF!");
    assert_eq!(code(eval("Tasks[Est min] + 1")), "#VALUE!");
}

#[test]
fn the_if_family_counts_and_sums_what_matches() {
    assert_eq!(eval("COUNTIF(Tasks[Course], \"JPN 101\")"), n(2.0));
    assert_eq!(eval("COUNTIF(Tasks[Course], \"jpn*\")"), n(2.0));
    assert_eq!(eval("COUNTIF(Tasks[Est min], \">=90\")"), n(2.0));
    assert_eq!(eval("COUNTIF(Tasks[Course], \"<>JPN 101\")"), n(2.0));
    assert_eq!(eval("COUNTIF(Tasks[Done], TRUE)"), n(1.0));
    assert_eq!(eval("COUNTIF(Tasks[Course], \"\")"), n(1.0));
    assert_eq!(
        eval("SUMIF(Tasks[Course], \"JPN 101\", Tasks[Est min])"),
        n(135.0)
    );
    assert_eq!(eval("SUMIF(Tasks[Est min], \">50\")"), n(210.0));
    assert_eq!(
        eval("AVERAGEIF(Tasks[Course], \"JPN 101\", Tasks[Est min])"),
        n(67.5)
    );
    assert_eq!(
        eval("SUMIFS(Tasks[Est min], Tasks[Course], \"JPN 101\", Tasks[Done], FALSE)"),
        n(90.0)
    );
    assert_eq!(
        eval("COUNTIFS(Tasks[Course], \"JPN 101\", Tasks[Est min], \"<60\")"),
        n(1.0)
    );
    assert_eq!(
        eval("MAXIFS(Tasks[Est min], Tasks[Course], \"JPN 101\")"),
        n(90.0)
    );
    assert_eq!(
        eval("MINIFS(Tasks[Est min], Tasks[Course], \"JPN 101\")"),
        n(45.0)
    );
    assert_eq!(eval("COUNTIF(Tasks[Due], \"<\" & TODAY())"), n(1.0));
    assert_eq!(
        code(eval("SUMIF(Tasks[Course], \"JPN 101\", Courses[Credits])")),
        "#VALUE!"
    );
}

#[test]
fn hours_logged_per_course_this_week() {
    // On Courses: each course's focus minutes since the week began, as hours.
    let src = "ROUND(SUMIFS(Focus sessions[Minutes], Focus sessions[Course], [Code], Focus sessions[Started], \">=\" & STARTOFWEEK(TODAY())) / 60, 2)";
    assert_eq!(eval_on("Courses", 0, src), n(1.25));
    assert_eq!(eval_on("Courses", 1, src), n(0.42));
}

#[test]
fn lookups_cross_tables() {
    assert_eq!(
        eval("LOOKUP([Course], Courses[Code], Courses[Name])"),
        t("Elementary Japanese")
    );
    assert_eq!(
        eval_on(
            "Tasks",
            1,
            "XLOOKUP(Course, Courses[Code], Courses[Credits])"
        ),
        n(3.0)
    );
    assert_eq!(
        eval_on(
            "Tasks",
            3,
            "LOOKUP(Course, Courses[Code], Courses[Name], \"No course\")"
        ),
        t("No course")
    );
    assert_eq!(
        code(eval_on(
            "Tasks",
            3,
            "LOOKUP(Course, Courses[Code], Courses[Name])"
        )),
        "#N/A"
    );
    assert_eq!(eval("MATCH(\"PHY 204\", Courses[Code])"), n(2.0));
    assert_eq!(eval("INDEX(Courses[Name], 2)"), t("Physics II"));
    assert_eq!(code(eval("INDEX(Courses[Name], 3)")), "#REF!");
    assert_eq!(
        eval("TEXTJOIN(\", \", UNIQUE(Tasks[Course]))"),
        t("JPN 101, PHY 204")
    );
    assert_eq!(eval("SUM(FILTER(Tasks[Est min], Tasks[Done]))"), n(45.0));
}

#[test]
fn logic() {
    assert_eq!(eval("IF(1 > 2, \"a\", \"b\")"), t("b"));
    assert_eq!(eval("IF(TRUE, 1)"), n(1.0));
    assert_eq!(
        eval("IFS([Est min] > 100, \"long\", [Est min] > 30, \"medium\", TRUE, \"short\")"),
        t("medium")
    );
    assert_eq!(eval("AND(TRUE, 1, \"yes\")"), Value::Bool(true));
    assert_eq!(eval("OR(FALSE, 0)"), Value::Bool(false));
    assert_eq!(eval("NOT(Done)"), Value::Bool(false));
    assert_eq!(eval("XOR(TRUE, TRUE, TRUE)"), Value::Bool(true));
    assert_eq!(
        eval("SWITCH(Course, \"JPN 101\", \"Japanese\", \"PHY 204\", \"Physics\", \"Other\")"),
        t("Japanese")
    );
    assert_eq!(
        eval_on("Tasks", 3, "SWITCH(Course, \"JPN 101\", 1, 0)"),
        n(0.0)
    );
    assert_eq!(eval("IFERROR(1 / 0, \"n/a\")"), t("n/a"));
    assert_eq!(eval("IFERROR(4 / 2, \"n/a\")"), n(2.0));
    // Only the branch taken is worked out.
    assert_eq!(eval("IF(TRUE, 1, 1 / 0)"), n(1.0));
    assert_eq!(eval_on("Tasks", 3, "IFBLANK(Course, \"None\")"), t("None"));
    assert_eq!(eval_on("Tasks", 3, "ISBLANK([Due])"), Value::Bool(true));
    assert_eq!(eval("ISNUMBER([Est min])"), Value::Bool(true));
    assert_eq!(eval("ISTEXT(Title)"), Value::Bool(true));
    assert_eq!(eval("ISERROR(1/0)"), Value::Bool(true));
}

#[test]
fn rounding_and_the_rest_of_the_numbers() {
    assert_eq!(eval("ROUND(2.5)"), n(3.0));
    assert_eq!(eval("ROUND(-2.5)"), n(-3.0));
    assert_eq!(eval("ROUND(1.005, 2)"), n(1.01));
    assert_eq!(eval("ROUND(1234.5678, -2)"), n(1200.0));
    assert_eq!(eval("ROUNDUP(1.21, 1)"), n(1.3));
    assert_eq!(eval("ROUNDDOWN(1.29, 1)"), n(1.2));
    assert_eq!(eval("FLOOR(47, 15)"), n(45.0));
    assert_eq!(eval("CEILING(47, 15)"), n(60.0));
    assert_eq!(eval("INT(-1.5)"), n(-2.0));
    assert_eq!(eval("ABS(-3)"), n(3.0));
    assert_eq!(eval("MOD(-1, 3)"), n(2.0));
    assert_eq!(eval("POWER(2, 10)"), n(1024.0));
    assert_eq!(eval("SQRT(81)"), n(9.0));
    assert_eq!(eval("SIGN(-9)"), n(-1.0));
    assert_eq!(eval("LOG10(1000)"), n(3.0));
    assert_eq!(eval("ROUND(LN(EXP(2)), 6)"), n(2.0));
    assert_eq!(code(eval("SQRT(-1)")), "#NUM!");
    assert_eq!(code(eval("MOD(1, 0)")), "#DIV/0!");
}

#[test]
fn date_math() {
    assert_eq!(eval("TODAY()"), day(2026, 10, 7));
    assert_eq!(eval("[Due] - TODAY()"), n(2.0));
    assert_eq!(eval("TODAY() + 7"), day(2026, 10, 14));
    assert_eq!(eval("DAYS([Due], TODAY())"), n(2.0));
    assert_eq!(
        eval_on("Tasks", 1, "IF([Due] < TODAY(), \"Late\", \"On time\")"),
        t("Late")
    );
    assert_eq!(eval("WEEKDAY(TODAY())"), n(4.0));
    assert_eq!(eval("WEEKDAY(TODAY(), 2)"), n(3.0));
    assert_eq!(eval("STARTOFWEEK(TODAY())"), day(2026, 10, 4));
    assert_eq!(eval("STARTOFWEEK(TODAY(), 2)"), day(2026, 10, 5));
    assert_eq!(
        eval("YEAR([Due]) * 10000 + MONTH([Due]) * 100 + DAY([Due])"),
        n(20261009.0)
    );
    assert_eq!(eval("DATE(2026, 12, 25) - TODAY()"), n(79.0));
    assert_eq!(eval("DATE(2026, 14, 1)"), day(2027, 2, 1));
    assert_eq!(
        eval("DATEVALUE(\"Oct 9, 2026\") = [Due]"),
        Value::Bool(true)
    );
    assert_eq!(
        eval("DATEDIF(DATE(2026, 1, 31), DATE(2026, 10, 7), \"m\")"),
        n(8.0)
    );
    assert_eq!(eval("DATEDIF(DATE(2000, 10, 8), TODAY(), \"y\")"), n(25.0));
    assert_eq!(eval("EDATE(DATE(2026, 1, 31), 1)"), day(2026, 2, 28));
    assert_eq!(eval("EOMONTH(TODAY(), 0)"), day(2026, 10, 31));
    assert_eq!(eval("HOUR(NOW())"), n(12.0));
    assert_eq!(eval("WEEKNUM(DATE(2026, 1, 1))"), n(1.0));
    assert_eq!(eval("TEXT([Due], \"mmm d, yyyy\")"), t("Oct 9, 2026"));
    assert_eq!(eval("TEXT([Due], \"ddd\")"), t("Fri"));
    assert_eq!(eval("\"Due \" & [Due]"), t("Due 2026-10-09"));
    assert_eq!(code(eval_on("Tasks", 3, "WEEKDAY([Due])")), "#VALUE!");
}

#[test]
fn text_functions() {
    assert_eq!(eval("LEN(Title)"), n(10.0));
    assert_eq!(eval("LEFT(Course, 3)"), t("JPN"));
    assert_eq!(eval("RIGHT(Course, 3)"), t("101"));
    assert_eq!(eval("MID(Title, 7, 4)"), t("quiz"));
    assert_eq!(eval("UPPER(Title)"), t("KANJI QUIZ"));
    assert_eq!(eval("LOWER(Course)"), t("jpn 101"));
    assert_eq!(
        eval("PROPER(\"elementary japanese\")"),
        t("Elementary Japanese")
    );
    assert_eq!(eval("TRIM(\"  a   b  \")"), t("a b"));
    assert_eq!(eval("SUBSTITUTE(Course, \" \", \"-\")"), t("JPN-101"));
    assert_eq!(eval("REPLACE(Course, 1, 3, \"JAP\")"), t("JAP 101"));
    assert_eq!(eval("FIND(\"quiz\", Title)"), n(7.0));
    assert_eq!(eval("SEARCH(\"QUIZ\", Title)"), n(7.0));
    assert_eq!(code(eval("FIND(\"QUIZ\", Title)")), "#N/A");
    assert_eq!(eval("CONTAINS(Title, \"KANJI\")"), Value::Bool(true));
    assert_eq!(eval("EXACT(\"a\", \"A\")"), Value::Bool(false));
    assert_eq!(eval("REPT(\"ab\", 3)"), t("ababab"));
    assert_eq!(eval("VALUE(\"1,234.5\") + VALUE(\"50%\")"), n(1235.0));
    assert_eq!(
        eval("CONCAT(Title, \": \", [Est min], \"m\")"),
        t("Kanji quiz: 45m")
    );
    assert_eq!(eval("TEXT(0.256, \"0.0%\")"), t("25.6%"));
    assert_eq!(eval("TEXT(1234567.891, \"#,##0.00\")"), t("1,234,567.89"));
    assert_eq!(eval("LEN(\"日本語\")"), n(3.0));
    assert_eq!(eval("LEFT(\"日本語\", 2)"), t("日本"));
    assert_eq!(eval("ROW()"), n(1.0));
}

#[test]
fn a_formula_that_cant_be_read_says_why_and_where() {
    for (src, part, at) in [
        ("", "Type a formula first", 0),
        ("1 +", "stops before it is finished", 3),
        ("(1 + 2", "no closing )", 0),
        ("1 + 2)", "goes on after", 5),
        ("SUM(1, 2", "no closing )", 0),
        ("NOPE(1)", "no function called NOPE", 0),
        ("\"open", "no closing quote", 0),
        ("[Est min", "no closing ]", 0),
        ("1 $ 2", "doesn't belong", 2),
        ("Est min + 1", "in brackets: [Est min]", 0),
        ("* 2", "needs a value before it", 0),
        ("1 2", "goes on after", 2),
        ("[]", "needs a column name", 0),
    ] {
        let e = Formula::parse(src).expect_err(src);
        assert!(e.message.contains(part), "{src}: {}", e.message);
        assert_eq!(e.at, at, "{src}");
    }
}

#[test]
fn a_formula_lists_the_columns_it_reads() {
    let f = Formula::parse(
        "SUMIFS(Focus sessions[Minutes], Focus sessions[Course], [Code]) + Credits + [Code]",
    )
    .unwrap();
    assert_eq!(
        f.refs(),
        [
            Ref {
                table: None,
                column: "Code".into()
            },
            Ref {
                table: None,
                column: "Credits".into()
            },
            Ref {
                table: Some("Focus sessions".into()),
                column: "Course".into()
            },
            Ref {
                table: Some("Focus sessions".into()),
                column: "Minutes".into()
            },
        ]
    );
    assert_eq!(
        f.source(),
        "SUMIFS(Focus sessions[Minutes], Focus sessions[Course], [Code]) + Credits + [Code]"
    );
}

#[test]
fn curly_quotes_and_typographic_signs_are_read() {
    assert_eq!(eval("IF(Done, “yes”, “no”)"), t("yes"));
    assert_eq!(eval("6 × 7 − 2 ÷ 2"), n(41.0));
    assert_eq!(eval("1 != 2"), Value::Bool(true));
    assert_eq!(eval("SUM(1; 2; 3)"), n(6.0));
}

#[test]
fn every_listed_function_exists() {
    let names = wi_formula::function_names();
    assert!(names.len() > 80);
    for name in names {
        // Each parses as a call; what it answers with no arguments is its own business.
        let f = Formula::parse(&format!("{name}()")).unwrap_or_else(|e| panic!("{name}: {e}"));
        let _ = f.eval(&tables("Tasks", 0));
    }
}
