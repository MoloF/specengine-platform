//! docs/features/plain-questions.md (RM-0088, the question half)
//! "Acceptance criteria", the core's half: `question_problem` over the
//! four new fields of a `QuestionInput`, as "Fields and caps" lists them.
//!
//! - PQ2: each cap's line (`<field>: <problem>`), in input order: `node_ids`,
//!   `text`, `gist`, `matters`, `options`, `recommendation`,
//!   `working_answer`, `price_of_other`, `distinct_from`, the author; the
//!   limits pinned to the constants (`GIST_MAX` 160, `MATTERS_MAX` 512,
//!   `OPTIONS_MIN` 2, `QUESTION_OPTIONS_MAX` 3), each at the limit taken
//!   and one byte over refused. M: 4 options accepted.
//! - PQ3: none of the four given, a question is judged as before; a
//!   discrepancy keeps its 2 to 6 options. M: `gist` required.
//! - PQ6: agent text is judged, never rewritten: a gist naming an ID, a
//!   path and a code name passes (the plain-words note is advice, never a
//!   refusal). `gist_note` over a resolver of its own: the first token
//!   holding `::`, ending in `.md` (ASCII case-insensitive, the listed
//!   punctuation shed first) or resolving is named, once; plain words and a
//!   token the resolver refuses give none. M: the note naming the last
//!   flagged token.
//!
//! The words the tests use (a lighthouse's lamp, its oil) are invented
//! here.
//!
//! Non-Latin characters in Rust sources are Unicode escapes (ADR-0024).

use specengine_core::intake::{
    AuthorInput, DiscrepancyInput, Evidence, GIST_MAX, GapType, IntakeOption, IntakeSeverity,
    LABEL_MAX, MATTERS_MAX, OPTION_TEXT_MAX, OPTIONS_MAX, OPTIONS_MIN, QUESTION_OPTIONS_MAX,
    QuestionInput, discrepancy_problem, gist_note, question_problem,
};

const AGENT: AuthorInput<'static> = AuthorInput {
    role: Some("developer"),
    model: None,
    run: None,
};

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|id| (*id).to_owned()).collect()
}

fn option(label: &str) -> IntakeOption {
    IntakeOption {
        label: label.to_owned(),
        effect: format!("{label}: the lamp changes"),
        price: "one scene".to_owned(),
    }
}

/// The fields a question is judged on, owned; [`Fields::problem`] judges
/// them as `question_problem` does.
#[derive(Clone)]
struct Fields {
    node_ids: Vec<String>,
    text: String,
    gist: Option<String>,
    matters: Option<String>,
    options: Option<Vec<IntakeOption>>,
    recommendation: Option<u64>,
    working_answer: String,
    price_of_other: String,
    distinct_from: Vec<String>,
}

impl Fields {
    /// A valid question holding all four fields.
    fn full() -> Self {
        Self {
            node_ids: ids(&["LAMP-OIL"]),
            text: "Does LAMP-OIL refill at dusk?".to_owned(),
            gist: Some("Should the lamp be refilled at dusk?".to_owned()),
            matters: Some("Two scenes wait on it.".to_owned()),
            options: Some(vec![option("At dusk"), option("At dawn")]),
            recommendation: Some(0),
            working_answer: "At dusk.".to_owned(),
            price_of_other: "Two scenes re-timed.".to_owned(),
            distinct_from: Vec::new(),
        }
    }

    /// A valid question holding none of the four.
    fn bare() -> Self {
        Self {
            gist: None,
            matters: None,
            options: None,
            recommendation: None,
            ..Self::full()
        }
    }

    /// The first problem's line, `None` when there is none.
    fn problem(&self) -> Option<String> {
        question_problem(
            &QuestionInput {
                node_ids: &self.node_ids,
                text: &self.text,
                gist: self.gist.as_deref(),
                matters: self.matters.as_deref(),
                options: self.options.as_deref(),
                recommendation: self.recommendation,
                working_answer: &self.working_answer,
                price_of_other: &self.price_of_other,
                distinct_from: &self.distinct_from,
            },
            AGENT,
        )
        .map(|problem| problem.to_string())
    }
}

/// PQ2: the limits as "Fields and caps" names them; a discrepancy's
/// options keep their own cap of 6.
#[test]
fn pq2_the_caps_are_the_specs_numbers() {
    assert_eq!(GIST_MAX, 160);
    assert_eq!(MATTERS_MAX, 512);
    assert_eq!(OPTIONS_MIN, 2);
    assert_eq!(QUESTION_OPTIONS_MAX, 3);
    assert_eq!(OPTIONS_MAX, 6, "a discrepancy's cap unchanged");
    assert_eq!(LABEL_MAX, 128);
    assert_eq!(OPTION_TEXT_MAX, 512);
}

/// PQ2: each refusal's line, as the CLI prints it after `spec: ` and the MCP
/// answer carries it. M: 4 options accepted.
#[test]
fn pq2_each_cap_names_its_field_and_problem() {
    let with = |edit: &dyn Fn(&mut Fields)| {
        let mut fields = Fields::full();
        edit(&mut fields);
        fields.problem()
    };
    let cases: Vec<(&str, Option<String>, &str)> = vec![
        (
            "4 options",
            with(&|f| {
                f.options = Some(vec![option("a"), option("b"), option("c"), option("d")]);
            }),
            "options: 4 option(s); give 2 to 3, each priced",
        ),
        (
            "1 option",
            with(&|f| f.options = Some(vec![option("a")])),
            "options: 1 option(s); give 2 to 3, each priced",
        ),
        (
            "[]",
            with(&|f| {
                f.options = Some(Vec::new());
                f.recommendation = None;
            }),
            "options: 0 option(s); give 2 to 3, each priced",
        ),
        (
            "a 161-byte gist",
            with(&|f| f.gist = Some("g".repeat(161))),
            "gist: 161 bytes; at most 160",
        ),
        (
            "a gist holding LF",
            with(&|f| f.gist = Some("Should the lamp\nbe refilled?".to_owned())),
            "gist: holds a line break; give one line",
        ),
        (
            "a gist holding CR",
            with(&|f| f.gist = Some("Should the lamp\rbe refilled?".to_owned())),
            "gist: holds a line break; give one line",
        ),
        (
            "a blank gist",
            with(&|f| f.gist = Some(" \t ".to_owned())),
            "gist: blank: give a value",
        ),
        (
            "an empty gist",
            with(&|f| f.gist = Some(String::new())),
            "gist: blank: give a value",
        ),
        (
            "a blank matters",
            with(&|f| f.matters = Some("  ".to_owned())),
            "matters: blank: give a value",
        ),
        (
            "a 513-byte matters",
            with(&|f| f.matters = Some("m".repeat(513))),
            "matters: 513 bytes; at most 512",
        ),
        (
            "recommendation 2 of 2 options",
            with(&|f| f.recommendation = Some(2)),
            "recommendation: 2 is no index into the 2 options (0 to 1)",
        ),
        (
            "a recommendation without options",
            with(&|f| f.options = None),
            "recommendation: given without options",
        ),
        (
            "a 129-byte label",
            with(&|f| f.options.as_mut().unwrap()[0].label = "l".repeat(129)),
            "options[0].label: 129 bytes; at most 128",
        ),
        (
            "a blank effect",
            with(&|f| f.options.as_mut().unwrap()[1].effect = " ".to_owned()),
            "options[1].effect: blank: give a value",
        ),
        (
            "a 513-byte price",
            with(&|f| f.options.as_mut().unwrap()[1].price = "p".repeat(513)),
            "options[1].price: 513 bytes; at most 512",
        ),
    ];
    for (label, got, want) in cases {
        assert_eq!(got.as_deref(), Some(want), "{label}");
    }
}

/// PQ2: each limit taken at its edge: a 160-byte gist (multi-byte
/// characters counted in bytes), a 512-byte matters holding CRLF (only the
/// gist is one line), three options, a 128-byte label, the last index
/// recommended; options without a recommendation.
#[test]
fn pq2_each_limit_is_taken_at_its_edge() {
    let mut edge = Fields::full();
    // 80 two-byte letters: 160 bytes.
    edge.gist = Some("\u{e9}".repeat(80));
    assert_eq!(edge.gist.as_ref().unwrap().len(), 160);
    edge.matters = Some(format!("{}\r\n{}", "m".repeat(255), "n".repeat(255)));
    assert_eq!(edge.matters.as_ref().unwrap().len(), 512);
    edge.options = Some(vec![option("a"), option("b"), option(&"l".repeat(128))]);
    edge.recommendation = Some(2);
    assert_eq!(edge.problem(), None);
    let mut unrecommended = Fields::full();
    unrecommended.recommendation = None;
    assert_eq!(
        unrecommended.problem(),
        None,
        "a recommendation is optional"
    );
    let mut over = edge.clone();
    over.gist = Some(format!("{}a", "\u{e9}".repeat(80)));
    assert_eq!(
        over.problem().as_deref(),
        Some("gist: 161 bytes; at most 160")
    );
}

/// PQ2, "Input order": the first problem in order `node_ids`, `text`,
/// `gist`, `matters`, `options`, `recommendation`, `working_answer`,
/// `price_of_other`, `distinct_from`, the author.
#[test]
fn pq2_the_first_problem_in_input_order_is_named() {
    let mut fields = Fields::full();
    fields.node_ids = Vec::new();
    fields.text = " ".to_owned();
    fields.gist = Some(" ".to_owned());
    fields.matters = Some(" ".to_owned());
    fields.options = Some(vec![option("a")]);
    fields.recommendation = Some(7);
    fields.working_answer = " ".to_owned();
    fields.price_of_other = " ".to_owned();
    fields.distinct_from = vec![String::new()];
    let mut seen = Vec::new();
    let order = [
        "node_ids",
        "text",
        "gist",
        "matters",
        "options",
        "recommendation",
        "working_answer",
        "price_of_other",
        "distinct_from",
    ];
    for field in order {
        let line = fields
            .problem()
            .unwrap_or_else(|| panic!("a problem at {field}"));
        let named = line.split(':').next().unwrap().to_owned();
        assert!(
            named == field || named.starts_with(&format!("{field}[")),
            "{field} first: {line}"
        );
        seen.push(named);
        match field {
            "node_ids" => fields.node_ids = ids(&["LAMP-OIL"]),
            "text" => fields.text = "Does it refill?".to_owned(),
            "gist" => fields.gist = Some("Refill at dusk?".to_owned()),
            "matters" => fields.matters = Some("Scenes wait.".to_owned()),
            "options" => fields.options = Some(vec![option("a"), option("b")]),
            "recommendation" => fields.recommendation = Some(1),
            "working_answer" => fields.working_answer = "Yes.".to_owned(),
            "price_of_other" => fields.price_of_other = "One scene.".to_owned(),
            _ => fields.distinct_from = Vec::new(),
        }
    }
    assert_eq!(fields.problem(), None, "{seen:?}");
}

/// PQ3: none of the four given, a question is judged as before (a valid one
/// passes; its old refusals keep their lines); a gist alone, `matters`
/// alone, options alone are each optional. M: `gist` required.
#[test]
fn pq3_none_given_judges_a_question_as_before() {
    assert_eq!(Fields::bare().problem(), None);
    let mut blank = Fields::bare();
    blank.text = String::new();
    assert_eq!(
        blank.problem().as_deref(),
        Some("text: blank: give a value")
    );
    let mut long = Fields::bare();
    long.working_answer = "w".repeat(2049);
    assert_eq!(
        long.problem().as_deref(),
        Some("working_answer: 2049 bytes; at most 2048")
    );
    for (label, edit) in [
        (
            "a gist alone",
            &(|f: &mut Fields| f.gist = Some("Refill at dusk?".to_owned())) as &dyn Fn(&mut Fields),
        ),
        ("matters alone", &|f: &mut Fields| {
            f.matters = Some("Scenes wait.".to_owned());
        }),
        ("options alone", &|f: &mut Fields| {
            f.options = Some(vec![option("a"), option("b"), option("c")]);
        }),
    ] {
        let mut fields = Fields::bare();
        edit(&mut fields);
        assert_eq!(fields.problem(), None, "{label}");
    }
}

/// PQ3, D1: a discrepancy keeps its shape: 2 to 6 options, a required
/// recommendation; six options pass, seven are refused with its old line.
#[test]
fn pq3_a_discrepancy_keeps_its_six_options() {
    let input = |count: usize| DiscrepancyInput {
        node_ids: ids(&["LAMP-OIL"]),
        summary: "The lamp refills at once.".to_owned(),
        gap_type: GapType::Contradicts,
        severity: IntakeSeverity::High,
        evidence: vec![Evidence {
            file: "src/lamp.rs".to_owned(),
            qpath: None,
            lines: None,
            observed: "at once".to_owned(),
            documented: "after rest".to_owned(),
        }],
        options: (0..count)
            .map(|index| option(&format!("o{index}")))
            .collect(),
        recommendation: 0,
        working_answer: None,
        proposed_patch: None,
        distinct_from: None,
    };
    assert_eq!(discrepancy_problem(&input(6), AGENT), None);
    assert_eq!(
        discrepancy_problem(&input(7), AGENT).map(|p| p.to_string()),
        Some("options: 7 option(s); give 2 to 6, each priced".to_owned())
    );
}

/// PQ6: agent text is judged as given: a gist naming an ID, a path and a
/// code name, a `matters` holding U+202E and CRLF, a label holding `::`:
/// no refusal (the note is advice; the text is never rewritten).
#[test]
fn pq6_code_in_the_gist_is_no_refusal() {
    let mut fields = Fields::full();
    fields.gist = Some("Does MEC-STAMINA in docs/spec/x.md call a::b?".to_owned());
    fields.matters = Some("Sprint\u{202e} tuning\r\nwaits.".to_owned());
    fields.options.as_mut().unwrap()[0].label = "stamina_regen::tick".to_owned();
    assert_eq!(fields.problem(), None);
}

/// The note naming `token`, as "The plain-words note" writes it.
fn note(token: &str) -> String {
    format!(
        "gist: `{token}` reads as code to the owner: say it in plain words; IDs belong in \
         node_ids"
    )
}

/// PQ6, "The plain-words note": the first flagged token in order, once.
/// Core judges rule 2 itself, a `.md` path once the listed punctuation is
/// shed (whatever the caller's predicate says); the caller's predicate gets
/// each token as written and shed and judges the rest: here rule 1, a
/// token as written holding `::`, and rule 3, a shed token the index
/// resolves (`MEC-STAMINA`, the alias `TERM-tired`, `stamina-tuning/AC-07`);
/// `ZZ-99`, a look-alike and plain words give none. The token is named
/// shed. M: the note naming the last flagged token.
#[test]
fn pq6_the_note_names_the_first_flagged_token() {
    let known = ["MEC-STAMINA", "TERM-tired", "stamina-tuning/AC-07"];
    let mut asked: Vec<(String, String)> = Vec::new();
    let mut flags = |token: &str, shed: &str| {
        asked.push((token.to_owned(), shed.to_owned()));
        token.contains("::") || known.contains(&shed)
    };
    let cases: [(&str, Option<&str>); 16] = [
        ("Does MEC-STAMINA restart its wait?", Some("MEC-STAMINA")),
        (
            "Does stamina_regen::tick restart it?",
            Some("stamina_regen::tick"),
        ),
        ("Is a::b before MEC-STAMINA?", Some("a::b")),
        ("Is MEC-STAMINA before a::b?", Some("MEC-STAMINA")),
        ("Does docs/spec/x.md say so?", Some("docs/spec/x.md")),
        ("Is README.MD the place?", Some("README.MD")),
        ("Is the rule in (docs/spec/x.md)?", Some("docs/spec/x.md")),
        ("Is it `MEC-STAMINA`, then?", Some("MEC-STAMINA")),
        ("Is TERM-tired the word?", Some("TERM-tired")),
        (
            "Does stamina-tuning/AC-07 wait?",
            Some("stamina-tuning/AC-07"),
        ),
        (
            "Does docs/spec/x.md agree with MEC-STAMINA and a::b?",
            Some("docs/spec/x.md"),
        ),
        ("Does stamina refill at rest?", None),
        ("Is stamina fine?", None),
        ("Does ZZ-99 refill at rest?", None),
        ("Does MEC-STAMIN\u{410} refill at rest?", None),
        ("Is the .mdx file the place?", None),
    ];
    for (gist, flagged) in cases {
        assert_eq!(gist_note(gist, &mut flags), flagged.map(note), "{gist}");
    }
    // The predicate saw each token as written and shed.
    assert!(
        asked.contains(&("`MEC-STAMINA`,".to_owned(), "MEC-STAMINA".to_owned())),
        "{asked:?}"
    );
    assert!(
        asked.contains(&(
            "stamina_regen::tick".to_owned(),
            "stamina_regen::tick".to_owned()
        )),
        "{asked:?}"
    );
    // Rule 2 is core's: a `.md` path is flagged whatever the predicate says;
    // nothing else is.
    let never = |_: &str, _: &str| false;
    assert_eq!(
        gist_note("Is it in (docs/spec/x.md)?", never),
        Some(note("docs/spec/x.md"))
    );
    assert_eq!(gist_note("Is MEC-STAMINA a::b?", never), None);
}
