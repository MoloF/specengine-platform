//! Spike group 5 `bevy-schedule` (docs/features/phase-0-spikes.md, AC-10 and
//! "Rules and edge cases"): the syntactic Bevy registration detector of
//! `specengine-code` on hand-written sources.
//!
//! Covered: nested tuples with every combinator of `COMBINATORS` and every
//! adapter of `ADAPTERS` (the `pipe` target recorded); comments inside
//! argument lists and tuples; turbofish and scoped paths, qualified trait
//! paths, closures (named by their enclosing `fn`) and factory calls; the
//! one-argument `add_systems`; the uncertain categories (`unknown_method`,
//! `macro_in_arguments`, `expression`, `arguments`, `parse_error`,
//! `metavariable`, `nesting_too_deep`); the plugin rules (`impl Plugin for`,
//! `fn(&mut App)`) and what is not a plugin; registrations and plugins read
//! from `macro_rules!` transcribers and macro invocation arguments; the
//! nesting cap on a 2 MB stack; a linear-cost check (including unclosed `<`
//! in macro bodies, macro arguments and code); a parse error confined to its
//! own call of a builder chain (first, middle, last) at its method name's
//! line; generic arguments with `->`, `>=`, `>>` and a turbofish after an
//! unclosed comparison in tokens; token-reader parity with the code reader
//! (several generic arguments, tokens after a path, a `<<`-opened
//! turbofish, a qualified path opening an element, `>>` inside a `::<<`
//! turbofish, comparisons in a tuple); the text cap across UTF-8 window
//! boundaries. Known token-reader gaps pinned (no guessed name): a
//! metavariable method or path segment is an `expression`; commas in type
//! positions and closure parameters split an element; an unclosed
//! turbofish at an element's end is read as its path.
//!
//! Every input is generated here. Deep and large inputs run on a thread with
//! an explicit 2 MB stack, so a regression to recursion over user input is a
//! stack overflow of this test binary, not a green run on a lucky stack.

use std::thread;
use std::time::{Duration, Instant};

use specengine_code::RustParser;
use specengine_code::bevy::{
    self, ADAPTERS, BevyAnalysis, COMBINATORS, Form, MAX_NESTING, MAX_TEXT_BYTES, Origin,
    PluginKind, TRUNCATION_MARK, Target, UncertainCategory,
};

/// Stack of the analysis thread: the size a caller could reasonably give a worker.
const TWO_MB: usize = 2 * 1024 * 1024;

fn detect(source: &str) -> BevyAnalysis {
    let mut parser = RustParser::new().expect("grammar loads");
    bevy::detect_file(&mut parser, source).expect("parse is never cancelled")
}

/// Parses and detects on a thread with `stack` bytes of stack.
fn detect_on_stack(source: String, stack: usize) -> BevyAnalysis {
    thread::Builder::new()
        .name("bevy-detector".to_owned())
        .stack_size(stack)
        .spawn(move || detect(&source))
        .expect("detector thread spawns")
        .join()
        .expect("the detector must not panic or overflow the stack")
}

/// `(target, origin, form, schedule, name)` of every registration.
type Row = (
    &'static str,
    &'static str,
    &'static str,
    Option<String>,
    Option<String>,
);

fn rows(analysis: &BevyAnalysis) -> Vec<Row> {
    analysis
        .registrations
        .iter()
        .map(|r| {
            (
                r.target.as_str(),
                r.origin.as_str(),
                r.form.as_str(),
                r.schedule.clone(),
                r.name.clone(),
            )
        })
        .collect()
}

fn names(analysis: &BevyAnalysis) -> Vec<Option<String>> {
    analysis
        .registrations
        .iter()
        .map(|r| r.name.clone())
        .collect()
}

fn some(names: &[&str]) -> Vec<Option<String>> {
    names.iter().map(|n| Some((*n).to_owned())).collect()
}

fn uncertain(analysis: &BevyAnalysis) -> Vec<(&'static str, &'static str, &'static str)> {
    analysis
        .uncertain
        .iter()
        .map(|u| (u.category.as_str(), u.origin.as_str(), u.call))
        .collect()
}

fn plugins(analysis: &BevyAnalysis) -> Vec<(&'static str, &'static str, Option<String>)> {
    analysis
        .plugins
        .iter()
        .map(|p| (p.kind.as_str(), p.origin.as_str(), p.name.clone()))
        .collect()
}

fn plugin_use_names(analysis: &BevyAnalysis) -> Vec<Option<String>> {
    analysis
        .plugin_uses
        .iter()
        .map(|u| u.name.clone())
        .collect()
}

// ------------------------------------------------ tuples, combinators, adapters

const EVERY_COMBINATOR_AND_ADAPTER: &str = r#"
fn build(app: &mut App) {
    app.add_systems(
        Update,
        (
            a.in_set(Set).before(x1).after(x2).before_ignore_deferred(x3).after_ignore_deferred(x4),
            (
                b.run_if(cond).distributive_run_if(cond2),
                (c.ambiguous_with(x5).ambiguous_with_all(),),
            ),
            (d, e).chain(),
            ((f, g).chain_ignore_deferred(), h.into_configs()),
            i.pipe(sink).map(drop),
            j.with_input(3),
            k.with_input_from(),
            l.pipe(first_sink).pipe(second_sink).run_if(cond3),
        ),
    );
}
"#;

#[test]
fn the_fixture_source_names_every_combinator_and_adapter() {
    for method in COMBINATORS.iter().chain(ADAPTERS) {
        assert!(
            EVERY_COMBINATOR_AND_ADAPTER.contains(&format!(".{method}(")),
            "the test source must exercise `.{method}(…)`"
        );
    }
}

#[test]
fn nested_tuples_with_every_combinator_and_adapter_yield_only_the_systems() {
    let analysis = detect(EVERY_COMBINATOR_AND_ADAPTER);
    assert!(!analysis.has_error);
    assert_eq!(uncertain(&analysis), vec![], "nothing is uncertain");
    assert_eq!(
        names(&analysis),
        some(&["a", "b", "c", "d", "e", "f", "g", "h", "i", "j", "k", "l"]),
        "combinator and adapter arguments are not systems; tuples nest to any depth"
    );
    for registration in &analysis.registrations {
        assert_eq!(registration.target, Target::System);
        assert_eq!(registration.origin, Origin::Code);
        assert_eq!(registration.form, Form::Path);
        assert_eq!(registration.schedule.as_deref(), Some("Update"));
        assert_eq!(registration.enclosing_fn.as_deref(), Some("build"));
    }
    let adapters: Vec<(&str, Vec<&str>, Vec<String>)> = analysis
        .registrations
        .iter()
        .filter(|r| !r.adapters.is_empty())
        .map(|r| {
            (
                r.name.as_deref().unwrap(),
                r.adapters.clone(),
                r.piped.clone(),
            )
        })
        .collect();
    assert_eq!(
        adapters,
        vec![
            ("i", vec!["pipe", "map"], vec!["sink".to_owned()]),
            ("j", vec!["with_input"], vec![]),
            ("k", vec!["with_input_from"], vec![]),
            (
                "l",
                vec!["pipe", "pipe"],
                vec!["first_sink".to_owned(), "second_sink".to_owned()]
            ),
        ],
        "adapters in source order, the `pipe` targets recorded"
    );
    assert_eq!(analysis.call_sites.add_systems, 1);
    assert_eq!(analysis.call_sites.in_macros, 0);
}

#[test]
fn comments_inside_arguments_and_tuples_are_ignored() {
    let source = r#"
fn build(app: &mut App) {
    app.add_systems(/* schedule */ Update, /* systems */ (
        // a line comment before the first element
        a, /* between */ b,
        /// a doc-like comment
        c, // trailing
        /* last */
    ));
    app.add_observer(/* o */ on_event /* after */);
    app.add_plugins((/* p */ PluginA, // q
        PluginB));
}
"#;
    let analysis = detect(source);
    assert_eq!(uncertain(&analysis), vec![], "a comment is not an argument");
    assert_eq!(names(&analysis), some(&["a", "b", "c", "on_event"]));
    assert_eq!(
        analysis.registrations[0].schedule.as_deref(),
        Some("Update")
    );
    assert_eq!(plugin_use_names(&analysis), some(&["PluginA", "PluginB"]));
}

// ------------------------------------------------------------- leaf forms

#[test]
fn turbofish_scoped_and_qualified_paths_closures_and_factories() {
    let source = r#"
fn setup(app: &mut App) {
    app.add_systems(
        Update,
        (
            generic_sys::<A>,
            systems::movement::walk,
            crate::sys::run::<B, C>,
            Self::apply,
            <Foo as Tr>::trait_sys,
            || {},
            move |query: Query<&A>| { let _ = query; },
            make_counter(3),
            factories::make::<T>(1, 2),
        ),
    );
}
"#;
    let analysis = detect(source);
    assert_eq!(uncertain(&analysis), vec![]);
    let got: Vec<(Form, Option<&str>, &str)> = analysis
        .registrations
        .iter()
        .map(|r| (r.form, r.name.as_deref(), r.text.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            (Form::Path, Some("generic_sys"), "generic_sys::<A>"),
            (Form::Path, Some("walk"), "systems::movement::walk"),
            (Form::Path, Some("run"), "crate::sys::run::<B,C>"),
            (Form::Path, Some("apply"), "Self::apply"),
            (Form::Path, Some("trait_sys"), "<FooasTr>::trait_sys"),
            (Form::Closure, None, "closure"),
            (Form::Closure, None, "closure"),
            (Form::Factory, Some("make_counter"), "make_counter"),
            (Form::Factory, Some("make"), "factories::make::<T>"),
        ],
        "name = last path segment without generic arguments; a closure has none; \
         a factory is named by its callee"
    );
    for registration in &analysis.registrations {
        assert_eq!(
            registration.enclosing_fn.as_deref(),
            Some("setup"),
            "every leaf records the innermost fn"
        );
    }
}

#[test]
fn a_closure_is_named_by_the_innermost_enclosing_fn() {
    let source = r#"
fn outer(app: &mut App) {
    fn inner(app: &mut App) {
        app.add_systems(Startup, || {});
    }
    app.add_systems(Update, || {});
    inner(app);
}
fn after(app: &mut App) {
    app.add_observer(|trigger: On<Add, Name>| { let _ = trigger; });
}
"#;
    let analysis = detect(source);
    let got: Vec<(Target, Form, Option<&str>)> = analysis
        .registrations
        .iter()
        .map(|r| (r.target, r.form, r.enclosing_fn.as_deref()))
        .collect();
    assert_eq!(
        got,
        vec![
            (Target::System, Form::Closure, Some("inner")),
            (Target::System, Form::Closure, Some("outer")),
            (Target::Observer, Form::Closure, Some("after")),
        ]
    );
}

#[test]
fn one_argument_add_systems_has_no_schedule_and_turbofish_methods_are_read() {
    let source = r#"
fn build(app: &mut App, schedule: &mut Schedule) {
    schedule.add_systems(tick);
    schedule.add_systems((a, b).chain());
    app.add_systems::<Marker>(Update, turbo);
    app.add_observer::<E, B, M>(observe);
    app.add_plugins::<M>(Turbo);
}
"#;
    let analysis = detect(source);
    assert_eq!(uncertain(&analysis), vec![]);
    assert_eq!(
        rows(&analysis),
        vec![
            ("system", "code", "path", None, Some("tick".to_owned())),
            ("system", "code", "path", None, Some("a".to_owned())),
            ("system", "code", "path", None, Some("b".to_owned())),
            (
                "system",
                "code",
                "path",
                Some("Update".to_owned()),
                Some("turbo".to_owned())
            ),
            ("observer", "code", "path", None, Some("observe".to_owned())),
        ]
    );
    assert_eq!(plugin_use_names(&analysis), some(&["Turbo"]));
    assert_eq!(analysis.call_sites.add_systems, 3);
    assert_eq!(analysis.call_sites.add_observer, 1);
    assert_eq!(analysis.call_sites.add_plugins, 1);
}

#[test]
fn schedule_labels_are_kept_without_whitespace_and_capped() {
    let long_label = format!("OnEnter(State::{})", "V".repeat(MAX_TEXT_BYTES * 2));
    let source = format!(
        "fn build(app: &mut App) {{\n    app.add_systems(OnEnter( GameState :: Menu ), enter);\n    app.add_systems({long_label}, long);\n}}\n"
    );
    let analysis = detect(&source);
    let labels: Vec<&str> = analysis
        .registrations
        .iter()
        .map(|r| r.schedule.as_deref().unwrap())
        .collect();
    assert_eq!(labels[0], "OnEnter(GameState::Menu)");
    assert!(labels[1].ends_with(TRUNCATION_MARK), "{}", labels[1]);
    assert_eq!(
        labels[1].len(),
        MAX_TEXT_BYTES + TRUNCATION_MARK.len_utf8(),
        "a long label is cut at MAX_TEXT_BYTES plus the mark"
    );
}

/// The text cap written out, independent of how the source is read: the
/// characters without whitespace, kept while they fit in `MAX_TEXT_BYTES`,
/// the mark appended at the first one that does not.
fn capped(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars().filter(|c| !c.is_whitespace()) {
        if out.len() + c.len_utf8() > MAX_TEXT_BYTES {
            out.push(TRUNCATION_MARK);
            return out;
        }
        out.push(c);
    }
    out
}

/// How many `MAX_TEXT_BYTES` windows over `text` end inside a character
/// before the cap is reached: the cases this test exists for.
fn straddled_windows(text: &str) -> usize {
    let (mut at, mut kept, mut straddled) = (0, 0, 0);
    while at < text.len() && kept <= MAX_TEXT_BYTES {
        let mut cut = (at + MAX_TEXT_BYTES).min(text.len());
        if !text.is_char_boundary(cut) {
            straddled += 1;
            while !text.is_char_boundary(cut) {
                cut -= 1;
            }
        }
        kept += text[at..cut]
            .chars()
            .filter(|c| !c.is_whitespace())
            .map(char::len_utf8)
            .sum::<usize>();
        at = cut;
    }
    straddled
}

#[test]
fn capped_texts_cut_at_a_character_boundary_whatever_the_window_alignment() {
    // Characters of 1-4 bytes, shifted by 0-7 bytes of padding and spaced by
    // ASCII and multi-byte whitespace, so that windows of the source end
    // inside characters and inside whitespace at every alignment. The
    // schedule label (code and macro body) and an uncertain call's text must
    // equal the cap written out; each width must reach exactly
    // `MAX_TEXT_BYTES` plus the mark for some padding.
    let wide = ['x', '\u{E9}', '\u{4E2D}', '\u{10348}'];
    let spacings = [
        "",
        " ",
        "\t \n  ",
        "\u{A0}",
        "\u{3000}",
        " \u{3000}\u{A0}\t",
    ];
    let full = MAX_TEXT_BYTES + TRUNCATION_MARK.len_utf8();
    for c in wide {
        let (mut exact, mut straddled) = (0, 0);
        for spacing in spacings {
            for pad in 0..8 {
                let label = format!(
                    "OnEnter(\"{}{}\")",
                    "x".repeat(pad),
                    format!("{spacing}{c}").repeat(200)
                );
                let call = format!("add_systems({label}, a, b)");
                let source = format!(
                    "fn build(app: &mut App) {{\n    app.add_systems({label}, sys);\n    app.{call};\n}}\n\
                     macro_rules! m {{\n    ($app:expr) => {{ $app.add_systems({label}, sys); }};\n}}\n"
                );
                let analysis = detect_on_stack(source, TWO_MB);
                let context = format!("char U+{:04X}, spacing {spacing:?}, pad {pad}", c as u32);
                assert!(!analysis.has_error, "{context}");
                let labels: Vec<&str> = analysis
                    .registrations
                    .iter()
                    .map(|r| r.schedule.as_deref().unwrap_or_default())
                    .collect();
                let expected = capped(&label);
                assert_eq!(labels, vec![expected.as_str(); 2], "{context}");
                assert_eq!(analysis.uncertain.len(), 1, "{context}");
                assert_eq!(analysis.uncertain[0].text, capped(&call), "{context}");
                for text in [expected.as_str(), analysis.uncertain[0].text.as_str()] {
                    assert!(text.len() <= full, "{context}: {} bytes", text.len());
                    assert!(text.ends_with(TRUNCATION_MARK), "{context}");
                }
                exact += usize::from(expected.len() == full);
                straddled += straddled_windows(&label) + straddled_windows(&call);
            }
        }
        assert!(
            exact > 0,
            "U+{:04X}: no case reached exactly the cap",
            c as u32
        );
        if c.len_utf8() > 1 {
            assert!(
                straddled > 0,
                "U+{:04X}: no window ended inside a character",
                c as u32
            );
        }
    }
}

#[test]
fn a_whitespace_heavy_label_is_compacted_across_many_windows_without_a_mark() {
    // Rust whitespace only: outside a string literal U+3000 is not.
    let gap = " \t\n  \r\n".repeat(1_000);
    let label = format!("OnEnter({gap}GameState{gap}::{gap}Menu{gap})");
    let source = format!(
        "fn build(app: &mut App) {{\n    app.add_systems({label}, sys);\n}}\n\
         macro_rules! m {{\n    ($app:expr) => {{ $app.add_systems({label}, sys); }};\n}}\n"
    );
    let analysis = detect_on_stack(source, TWO_MB);
    assert!(!analysis.has_error);
    let labels: Vec<Option<&str>> = analysis
        .registrations
        .iter()
        .map(|r| r.schedule.as_deref())
        .collect();
    assert_eq!(labels, vec![Some("OnEnter(GameState::Menu)"); 2]);

    // Over the cap after compaction: exactly the cap plus the mark.
    let long = format!(
        "OnEnter(({}))",
        vec![format!("{gap}V{gap}"); MAX_TEXT_BYTES].join(",")
    );
    let source = format!("fn build(app: &mut App) {{\n    app.add_systems({long}, sys);\n}}\n");
    let analysis = detect_on_stack(source, TWO_MB);
    assert!(!analysis.has_error);
    let label = analysis.registrations[0].schedule.clone().unwrap();
    assert_eq!(label, capped(&long));
    assert!(label.starts_with("OnEnter((V,V,V,"), "{label}");
    assert_eq!(label.len(), MAX_TEXT_BYTES + TRUNCATION_MARK.len_utf8());
}

// ----------------------------------------------------------- uncertain

#[test]
fn an_unknown_method_is_uncertain_and_its_siblings_are_still_read() {
    let source = r#"
fn build(app: &mut App) {
    app.add_systems(Update, tick.frobnicate());
    app.add_systems(Update, (ok_sys, bad.run_if(c).frobnicate(), after_bad));
}
"#;
    let analysis = detect(source);
    assert_eq!(
        uncertain(&analysis),
        vec![
            ("unknown_method", "code", "add_systems"),
            ("unknown_method", "code", "add_systems"),
        ]
    );
    assert_eq!(analysis.uncertain[0].text, "frobnicate");
    assert_eq!(names(&analysis), some(&["ok_sys", "after_bad"]));
}

#[test]
fn a_macro_in_a_system_position_is_uncertain() {
    let source = r#"
fn build(app: &mut App) {
    app.add_systems(Update, my_systems!());
    app.add_systems(Update, (before, other_systems![a, b], after));
    app.add_observer(observer_macro!(x));
}
"#;
    let analysis = detect(source);
    assert_eq!(
        uncertain(&analysis),
        vec![
            ("macro_in_arguments", "code", "add_systems"),
            ("macro_in_arguments", "code", "add_systems"),
            ("macro_in_arguments", "code", "add_observer"),
        ]
    );
    assert_eq!(names(&analysis), some(&["before", "after"]));
}

#[test]
fn other_expressions_wrong_arity_and_parse_errors_are_uncertain() {
    let source = r#"
fn build(app: &mut App) {
    app.add_systems(Update, self.systems);
    app.add_systems(Update, if flag { a } else { b });
    app.add_systems(Update, a, b);
    app.add_observer();
    app.add_plugins(A, B);
}
"#;
    let analysis = detect(source);
    assert!(!analysis.has_error);
    assert_eq!(
        uncertain(&analysis),
        vec![
            ("expression", "code", "add_systems"),
            ("expression", "code", "add_systems"),
            ("arguments", "code", "add_systems"),
            ("arguments", "code", "add_observer"),
            ("arguments", "code", "add_plugins"),
        ]
    );
    assert!(analysis.registrations.is_empty());
    assert!(analysis.plugin_uses.is_empty());

    let broken = "fn build(app: &mut App) {\n    app.add_systems(Update, (a, b c));\n    app.add_systems(Update, fine);\n}\n";
    let analysis = detect(broken);
    assert!(analysis.has_error, "the file has a parse error");
    assert_eq!(
        uncertain(&analysis),
        vec![("parse_error", "code", "add_systems")],
        "a registration with a parse error is not read, never guessed"
    );
    assert_eq!(analysis.uncertain[0].line, 2);
    assert_eq!(names(&analysis), some(&["fine"]), "the next call is read");
}

/// A builder chain on `app`: line 2 holds the receiver, call `i` sits on
/// line `3 + i` and registers `s{i}`, except the calls in `broken`, whose
/// system argument is `error`.
fn chain_with_errors(len: usize, broken: &[usize], error: &str) -> String {
    let calls: String = (0..len)
        .map(|i| {
            if broken.contains(&i) {
                format!("\n        .add_systems(Update, {error})")
            } else {
                format!("\n        .add_systems(Update, s{i})")
            }
        })
        .collect();
    format!("fn build(app: &mut App) {{\n    app{calls};\n}}\n")
}

/// Registered names, their lines and the uncertain `(category, line, text)`
/// of a chain built by [`chain_with_errors`], as the detector must report them.
type ChainReport = (
    Vec<Option<String>>,
    Vec<usize>,
    Vec<(&'static str, usize, String)>,
);

fn chain_report(analysis: &BevyAnalysis) -> ChainReport {
    (
        names(analysis),
        analysis.registrations.iter().map(|r| r.line).collect(),
        analysis
            .uncertain
            .iter()
            .map(|u| (u.category.as_str(), u.line, u.text.clone()))
            .collect(),
    )
}

fn expected_chain_report(len: usize, broken: &[usize], error: &str) -> ChainReport {
    let read: Vec<usize> = (0..len).filter(|i| !broken.contains(i)).collect();
    let compact: String = error.chars().filter(|c| !c.is_whitespace()).collect();
    (
        read.iter().map(|i| Some(format!("s{i}"))).collect(),
        read.iter().map(|i| 3 + i).collect(),
        broken
            .iter()
            .map(|i| {
                (
                    "parse_error",
                    3 + i,
                    format!("add_systems(Update,{compact})"),
                )
            })
            .collect(),
    )
}

#[test]
fn a_parse_error_in_a_builder_chain_stays_in_its_own_call() {
    // The broken call first, in the middle, last, and two at once: every
    // other call is read, and each broken one is exactly one `parse_error`
    // at its own method name's line with its own text (never the receiver).
    let len = 5;
    for error in ["(a b)", "(a, b c)", "(a.run_if(x y), b)"] {
        for broken in [&[0][..], &[len / 2], &[len - 1], &[0, len - 1]] {
            let source = chain_with_errors(len, broken, error);
            let analysis = detect(&source);
            let context = format!("calls {broken:?} of {len} broken by `{error}`:\n{source}");
            assert!(analysis.has_error, "{context}");
            assert_eq!(analysis.call_sites.add_systems, len, "{context}");
            assert_eq!(
                chain_report(&analysis),
                expected_chain_report(len, broken, error),
                "{context}"
            );
        }
    }
}

#[test]
fn a_chain_starting_on_the_receiver_line_keeps_its_parse_error_there() {
    let source = "fn build(app: &mut App) {\n    app.add_systems(Update, (a b))\n        .add_systems(Update, s1)\n        .add_systems(Update, s2);\n}\n";
    let analysis = detect(source);
    assert_eq!(names(&analysis), some(&["s1", "s2"]));
    assert_eq!(
        uncertain(&analysis),
        vec![("parse_error", "code", "add_systems")]
    );
    assert_eq!(analysis.uncertain[0].line, 2);
    assert_eq!(analysis.uncertain[0].text, "add_systems(Update,(ab))");
}

#[test]
fn wrong_arity_inside_a_chain_is_reported_at_the_method_name_line() {
    let source = "fn build(app: &mut App) {\n    app\n        .add_systems(Update, s0)\n        .add_systems(Update, a, b)\n        .add_observer()\n        .add_systems(Update, s3);\n}\n";
    let analysis = detect(source);
    assert!(!analysis.has_error);
    assert_eq!(names(&analysis), some(&["s0", "s3"]));
    let got: Vec<(&str, usize, &str)> = analysis
        .uncertain
        .iter()
        .map(|u| (u.category.as_str(), u.line, u.text.as_str()))
        .collect();
    assert_eq!(
        got,
        vec![
            ("arguments", 4, "add_systems(Update,a,b)"),
            ("arguments", 5, "add_observer()"),
        ],
        "the line and text of the call itself, not of the chain's receiver"
    );
}

// ------------------------------------------------------------- plugins

#[test]
fn plugin_rules_accept_impl_plugin_and_fn_mut_app_only() {
    let source = r#"
struct Foo;
struct Bar<T>(T);
struct Baz;
impl Plugin for Foo {
    fn build(&self, app: &mut App) {}
}
impl<T> bevy::app::Plugin for Bar<T> {
    fn build(&self, app: &mut App) {}
}
impl NotPlugin for Baz {}
impl Baz {
    fn method(&self, app: &mut App) {}
    fn by_mut_self(&mut self) {}
}
fn plugin_ok(app: &mut App) {}
fn plugin_qualified(app: &mut bevy::app::App) {}
fn plugin_unit(app: &mut App) -> () {}
fn plugin_mut_binding(mut app: &mut App) {}
fn plugin_generic<T>(app: &mut App) {}
fn returns_app(app: &mut App) -> &mut App { app }
fn world(world: &mut World) {}
fn shared(app: &App) {}
fn owned(app: App) {}
fn two(app: &mut App, extra: u32) {}
fn none() {}
fn sub_app(app: &mut SubApp) {}
"#;
    let analysis = detect(source);
    assert_eq!(
        plugins(&analysis),
        vec![
            ("impl_plugin", "code", Some("Foo".to_owned())),
            ("impl_plugin", "code", Some("Bar".to_owned())),
            ("fn_app", "code", Some("plugin_ok".to_owned())),
            ("fn_app", "code", Some("plugin_qualified".to_owned())),
            ("fn_app", "code", Some("plugin_unit".to_owned())),
            ("fn_app", "code", Some("plugin_mut_binding".to_owned())),
            ("fn_app", "code", Some("plugin_generic".to_owned())),
        ],
        "`-> &mut App`, `&self` methods, `&mut World`, `&App`, `App` by value, \
         two parameters and `&mut SubApp` are not plugins"
    );
}

#[test]
fn plugin_uses_read_names_through_builders_and_constructors() {
    let source = r#"
fn main() {
    App::new()
        .add_plugins(DefaultPlugins.set(WindowPlugin::default()).disable::<LogPlugin>())
        .add_plugins((
            MyPlugin,
            crate::net::NetPlugin,
            ConfiguredPlugin { rate: 3 },
            TuplePlugin(1),
            ByDefault::default(),
            a::Built::new(3),
            setup_fn,
        ))
        .run();
}
"#;
    let analysis = detect(source);
    assert_eq!(uncertain(&analysis), vec![]);
    assert_eq!(
        plugin_use_names(&analysis),
        some(&[
            "DefaultPlugins",
            "MyPlugin",
            "NetPlugin",
            "ConfiguredPlugin",
            "TuplePlugin",
            "ByDefault",
            "Built",
            "setup_fn",
        ])
    );
    assert_eq!(analysis.call_sites.add_plugins, 2);
}

// ---------------------------------------------------------- macro bodies

#[test]
fn macro_rules_transcribers_yield_names_metavariables_and_plugins() {
    let source = r#"
macro_rules! register {
    ($app:expr, $sys:ident) => {
        $app.add_systems(Update, $crate::systems::tick);
        $app.add_systems(PostUpdate, ($sys, other.run_if(cond), made(1)));
        $app.add_observer(on_thing);
        $app.add_plugins(($crate::net::NetPlugin, Local));
    };
}
macro_rules! plugin {
    ($name:ident, $setup:ident) => {
        impl Plugin for $name {
            fn build(&self, app: &mut App) {}
        }
        impl Plugin for Named {
            fn build(&self, app: &mut App) {}
        }
        pub fn $setup(app: &mut App) {}
        fn setup_named(mut app: &mut App) {}
        fn not_a_plugin(app: &mut App) -> &mut App { app }
        fn world_fn(world: &mut World) {}
    };
}
"#;
    let analysis = detect(source);
    assert!(!analysis.has_error);
    assert_eq!(
        rows(&analysis),
        vec![
            (
                "system",
                "macro_rules",
                "path",
                Some("Update".to_owned()),
                Some("tick".to_owned())
            ),
            (
                "system",
                "macro_rules",
                "path",
                Some("PostUpdate".to_owned()),
                Some("other".to_owned())
            ),
            (
                "system",
                "macro_rules",
                "factory",
                Some("PostUpdate".to_owned()),
                Some("made".to_owned())
            ),
            (
                "observer",
                "macro_rules",
                "path",
                None,
                Some("on_thing".to_owned())
            ),
        ],
        "`$crate::path` gives its last segment; macro registrations carry names only"
    );
    assert_eq!(
        uncertain(&analysis),
        vec![("metavariable", "macro_rules", "add_systems")],
        "`$sys` in a system position is a metavariable, never a guess"
    );
    assert_eq!(
        plugins(&analysis),
        vec![
            ("impl_plugin", "macro_rules", None),
            ("impl_plugin", "macro_rules", Some("Named".to_owned())),
            ("fn_app", "macro_rules", None),
            ("fn_app", "macro_rules", Some("setup_named".to_owned())),
        ],
        "`impl Plugin for $name` is a plugin with no name; `&self`, `-> &mut App` \
         and `&mut World` are not plugins in tokens either"
    );
    assert_eq!(plugin_use_names(&analysis), some(&["NetPlugin", "Local"]));
    assert_eq!(analysis.call_sites.add_systems, 2);
    assert_eq!(analysis.call_sites.add_observer, 1);
    assert_eq!(analysis.call_sites.add_plugins, 1);
    assert_eq!(analysis.call_sites.in_macros, 4);
}

#[test]
fn macro_invocation_arguments_are_read_with_their_enclosing_fn() {
    let source = r#"
fn wire(app: &mut App) {
    wrap! {
        app.add_systems(Startup, (setup_level, spawn.after(setup_level)));
        app.add_systems(Update, systems![a, b]);
        app.add_systems(Update, tick.frobnicate());
    }
    cfg_if::cfg_if! { if #[cfg(x)] { app.add_observer(|t: On<E>| {}); } }
}
"#;
    let analysis = detect(source);
    let got: Vec<(&str, &str, Option<&str>, Option<&str>)> = analysis
        .registrations
        .iter()
        .map(|r| {
            (
                r.origin.as_str(),
                r.form.as_str(),
                r.name.as_deref(),
                r.enclosing_fn.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        got,
        vec![
            ("macro_call", "path", Some("setup_level"), Some("wire")),
            ("macro_call", "path", Some("spawn"), Some("wire")),
            ("macro_call", "closure", None, Some("wire")),
        ]
    );
    assert_eq!(
        uncertain(&analysis),
        vec![
            ("macro_in_arguments", "macro_call", "add_systems"),
            ("unknown_method", "macro_call", "add_systems"),
        ]
    );
    assert_eq!(analysis.call_sites.in_macros, 4);
}

// ------------------------------------------------ angle brackets in tokens

#[test]
fn a_turbofish_after_an_unclosed_comparison_is_read_in_macro_bodies_and_arguments() {
    // An unclosed `<` (a comparison) is dropped at `;` or its group's end,
    // and even when still pending it never swallows a later turbofish.
    let source = r#"
macro_rules! wire {
    ($app:expr) => {
        let _ = a < b;
        $app.add_systems::<T>(Update, after_semicolon);
        if a < b { $app.add_systems::<T>(Update, in_block); }
        a < b, $app.add_systems::<T>(Update, after_comma);
        fn f < fn g < $app.add_observer::<T>(after_fn_angles);
    };
}
fn build(app: &mut App) {
    wrap! {
        let _ = a < b;
        app.add_systems::<T>(Update, arg_after_semicolon);
        x < y, app.add_systems::<T>(Update, arg_after_comma);
    }
    let _ = a < b;
    app.add_systems::<T>(Update, code_after_comparison);
}
"#;
    let analysis = detect(source);
    assert_eq!(
        names(&analysis),
        some(&[
            "after_semicolon",
            "in_block",
            "after_comma",
            "after_fn_angles",
            "arg_after_semicolon",
            "arg_after_comma",
            "code_after_comparison",
        ])
    );
    assert_eq!(uncertain(&analysis), vec![]);
    assert_eq!(analysis.call_sites.add_systems, 6);
    assert_eq!(analysis.call_sites.add_observer, 1);
}

#[test]
fn generic_arguments_with_arrows_comparisons_and_shifts_are_skipped_whole() {
    // `->` and `>=` never close a `<`; `>>` closes two. Checked in a macro
    // body, in macro arguments and in code, for the turbofish of the call,
    // of a system path, of a combinator and of a plugin function.
    let generics = [
        ("arrow", "fn() -> u8"),
        ("ge", "Foo<{ N >= 1 }>"),
        ("shift", "Vec<Vec<u8>>"),
        ("shift_three", "Vec<Vec<Vec<u8>>>"),
    ];
    let mut expected = Vec::new();
    let mut body = String::new();
    for (tag, generic) in generics {
        body.push_str(&format!(
            "        APP.add_systems::<{generic}>(Update, (call_{tag}, path_{tag}::<{generic}>, comb_{tag}.run_if::<{generic}>(c)));\n"
        ));
        expected.extend([
            format!("call_{tag}"),
            format!("path_{tag}"),
            format!("comb_{tag}"),
        ]);
    }
    let plugin_fns: String = generics
        .iter()
        .map(|(tag, generic)| format!("fn plugin_{tag}<T: Into<{generic}>>(app: &mut App) {{}}\n"))
        .collect();
    let in_macro = format!(
        "macro_rules! wire {{\n    ($app:expr) => {{\n{}{plugin_fns}    }};\n}}\n",
        body.replace("APP", "$app")
    );
    let in_arguments = format!(
        "fn build(app: &mut App) {{\n    wrap! {{\n{}{plugin_fns}    }}\n}}\n",
        body.replace("APP", "app")
    );
    let in_code = format!(
        "fn build(app: &mut App) {{\n{}}}\n{plugin_fns}",
        body.replace("APP", "app")
    );
    let plugin_names: Vec<Option<String>> = generics
        .iter()
        .map(|(tag, _)| Some(format!("plugin_{tag}")))
        .collect();
    for (what, source) in [
        ("macro body", in_macro),
        ("macro arguments", in_arguments),
        ("code", in_code),
    ] {
        let analysis = detect(&source);
        assert!(!analysis.has_error, "{what}: the source parses\n{source}");
        let expected: Vec<Option<String>> = expected.iter().cloned().map(Some).collect();
        assert_eq!(names(&analysis), expected, "{what}:\n{source}");
        assert_eq!(uncertain(&analysis), vec![], "{what}:\n{source}");
        assert_eq!(
            analysis.call_sites.add_systems,
            generics.len(),
            "{what}: every call is counted"
        );
        let fn_plugins: Vec<Option<String>> = analysis
            .plugins
            .iter()
            .filter(|p| p.kind == PluginKind::FnApp && p.name.as_deref() != Some("build"))
            .map(|p| p.name.clone())
            .collect();
        assert_eq!(fn_plugins, plugin_names, "{what}: generic plugin fns");
    }
}

/// `body` with `APP` as the receiver, in a `macro_rules!` transcriber, in
/// macro invocation arguments and in ordinary code.
fn in_three_contexts(body: &str) -> [(&'static str, String); 3] {
    [
        (
            "macro body",
            format!(
                "macro_rules! wire {{\n    ($app:expr) => {{\n        {};\n    }};\n}}\n",
                body.replace("APP", "$app")
            ),
        ),
        (
            "macro arguments",
            format!(
                "fn build(app: &mut App) {{\n    wrap! {{ {} }}\n}}\n",
                body.replace("APP", "app")
            ),
        ),
        (
            "code",
            format!(
                "fn build(app: &mut App) {{\n    {}\n}}\n",
                body.replace("APP", "app")
            ),
        ),
    ]
}

/// Runs `body` in the three contexts and collects every context whose
/// names and uncertain categories differ from the expected ones.
fn mismatches(body: &str, names_expected: &[&str], categories: &[&str]) -> Vec<String> {
    let mut wrong = Vec::new();
    for (what, source) in in_three_contexts(body) {
        let analysis = detect(&source);
        assert!(!analysis.has_error, "{what}: the source parses\n{source}");
        let got_names = names(&analysis);
        let got_categories: Vec<&str> = analysis
            .uncertain
            .iter()
            .map(|u| u.category.as_str())
            .collect();
        if got_names != some(names_expected) || got_categories != categories {
            let texts: Vec<(&str, &str)> = analysis
                .registrations
                .iter()
                .map(|r| (r.name.as_deref().unwrap_or("-"), r.text.as_str()))
                .collect();
            wrong.push(format!(
                "{what}: registrations (name, text) {texts:?}, uncertain {got_categories:?}"
            ));
        }
    }
    wrong
}

#[test]
fn a_turbofish_with_several_generic_arguments_is_one_system_element() {
    // The comma between generic arguments is not an element separator: a
    // path or combinator with `::<A, B>` is one system, never a guessed
    // registration named after the second argument.
    let wrong = mismatches(
        "APP.add_systems(Update, (sys::<A, B>, comb.run_if::<A, B>(c), last));",
        &["sys", "comb", "last"],
        &[],
    );
    assert!(
        wrong.is_empty(),
        "expected names [sys, comb, last] and nothing uncertain:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn an_element_with_tokens_after_its_path_is_an_expression_in_tokens_as_in_code() {
    // The code reader reports `a + b`, `c as D`, `e?`, `f[0]` as
    // `expression`; the token reader must not register `a`, `c`, `e`, `f`.
    let wrong = mismatches(
        "APP.add_systems(Update, (a + b, c as D, e?, f[0], ok));",
        &["ok"],
        &["expression", "expression", "expression", "expression"],
    );
    assert!(
        wrong.is_empty(),
        "expected names [ok] and four `expression`, as the code reader reports:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn a_turbofish_opening_with_a_qualified_path_is_counted_and_read() {
    // `::<<T as Tr>::M>` lexes as `::` `<<` in a token tree; the call must
    // still be counted and read (or reported), never silently dropped.
    let wrong = mismatches(
        "APP.add_systems::<<T as Tr>::M>(Update, (qualified, s::<<T as Tr>::M>));",
        &["qualified", "s"],
        &[],
    );
    assert!(
        wrong.is_empty(),
        "expected names [qualified, s]:\n{}",
        wrong.join("\n")
    );
    for (what, source) in in_three_contexts("APP.add_systems::<<T as Tr>::M>(Update, q);") {
        assert_eq!(
            detect(&source).call_sites.add_systems,
            1,
            "{what}: the call is counted"
        );
    }
}

#[test]
fn a_qualified_path_opening_an_element_keeps_its_generic_commas() {
    // `<` and `<<` at an element's start open generic arguments: their
    // commas separate nothing, in tokens as in code.
    let wrong = mismatches(
        "APP.add_systems(Update, (<A as Tr<X, Y>>::f, <<A as Tr<X, Y>>::M as Tr2>::g, \
         <T as Tr>::h::<X, Y>, ok));",
        &["f", "g", "h", "ok"],
        &[],
    );
    assert!(
        wrong.is_empty(),
        "expected names [f, g, h, ok] and nothing uncertain:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn a_double_shift_closes_inside_a_turbofish_opened_by_a_double_angle() {
    // `::<<` pairs through its outer unit; `>>` of `Vec<Vec<T>>` (or of
    // `M<u8>>`) closes two units inside it, and the call stays readable.
    let body = "APP.add_systems::<<Vec<Vec<T>> as Tr>::M>(Update, (s::<<Vec<Vec<T>> as Tr>::M>, \
                t::<<T as Tr<Vec<Vec<u8>>>>::M>, u::<<T as Tr>::M<u8>>, \
                v::<<A as Tr>::M, <B as Tr>::N>, ok));";
    let wrong = mismatches(body, &["s", "t", "u", "v", "ok"], &[]);
    assert!(
        wrong.is_empty(),
        "expected names [s, t, u, v, ok] and nothing uncertain:\n{}",
        wrong.join("\n")
    );
    for (what, source) in in_three_contexts(body) {
        assert_eq!(
            detect(&source).call_sites.add_systems,
            1,
            "{what}: the call is counted"
        );
    }
}

#[test]
fn comparisons_in_a_tuple_are_separate_expressions_in_tokens_as_in_code() {
    // A comparison's `<` / `<<` is not at an element's start nor after
    // `::`: the comma after it separates elements, as it does in code, and
    // a turbofish between two comparisons is still skipped whole.
    let wrong = mismatches(
        "APP.add_systems(Update, (a < b, s::<X, Y>, c > d, e << f, g >> h, ok));",
        &["s", "ok"],
        &["expression", "expression", "expression", "expression"],
    );
    assert!(
        wrong.is_empty(),
        "expected names [s, ok] and four `expression`, as the code reader reports:\n{}",
        wrong.join("\n")
    );
}

#[test]
fn a_metavariable_method_or_path_segment_makes_its_element_an_expression() {
    // Pinned side effect of reading leftover tokens as an expression:
    // `sys.$m(c)` and `systems::$name` register nothing (not `sys`, not
    // `systems`); a metavariable inside a combinator's argument and
    // `$crate::` at a path's start do not stop the read.
    let source = "macro_rules! wire {\n    ($app:expr, $m:ident, $name:ident, $c:expr) => {\n        \
                  $app.add_systems(Update, (sys.$m(c), systems::$name, ok, sys.run_if($c), $crate::s::f));\n    \
                  };\n}\n";
    let analysis = detect(source);
    assert!(!analysis.has_error, "the source parses\n{source}");
    let registered: Vec<(Option<String>, &str, &str)> = analysis
        .registrations
        .iter()
        .map(|r| (r.name.clone(), r.text.as_str(), r.origin.as_str()))
        .collect();
    assert_eq!(
        registered,
        vec![
            (Some("ok".to_owned()), "ok", "macro_rules"),
            (Some("sys".to_owned()), "sys", "macro_rules"),
            (Some("f".to_owned()), "$crate::s::f", "macro_rules"),
        ]
    );
    let reported: Vec<(&str, &str)> = analysis
        .uncertain
        .iter()
        .map(|u| (u.category.as_str(), u.text.as_str()))
        .collect();
    assert_eq!(
        reported,
        vec![
            ("expression", "sys.$m(c)"),
            ("expression", "systems::$name"),
        ]
    );
}

/// Names and uncertain categories of `source`, which must parse.
fn observed(what: &str, source: &str) -> (Vec<Option<String>>, Vec<&'static str>) {
    let analysis = detect(source);
    assert!(!analysis.has_error, "{what}: the source parses\n{source}");
    let categories = analysis
        .uncertain
        .iter()
        .map(|u| u.category.as_str())
        .collect();
    (names(&analysis), categories)
}

fn optional(names: &[Option<&str>]) -> Vec<Option<String>> {
    names.iter().map(|n| n.map(str::to_owned)).collect()
}

#[test]
fn commas_in_type_positions_split_a_token_element_but_never_yield_a_guessed_name() {
    // Known gap, pinned. In a token tree a comma inside a closure's
    // parameter list or a type (`HashMap<A, B>`, `Query<&T, With<P>>`,
    // `Result<(), E>`) is in no delimiter group, so it splits the element;
    // the code reader keeps one element. Checked first: the fragments never
    // become a named registration and are reported as `expression` or
    // `arguments`. Then the exact disagreement, so a change shows up here.
    type Seen<'a> = (&'a [Option<&'a str>], &'a [&'a str]);
    let closure_and_ok: &[Option<&str>] = &[None, Some("ok")];
    let cases: [(&str, Seen, Seen); 8] = [
        (
            "APP.add_systems(Update, (|x: HashMap<A, B>| x, ok));",
            (closure_and_ok, &[]),
            (closure_and_ok, &["expression"]),
        ),
        (
            "APP.add_systems(Update, (|mut commands: Commands, time: Res<Time>| {}, ok));",
            (closure_and_ok, &[]),
            (closure_and_ok, &["expression"]),
        ),
        (
            "APP.add_systems(Update, (|x| -> Result<(), E> { Ok(()) }, ok));",
            (closure_and_ok, &[]),
            (closure_and_ok, &["expression"]),
        ),
        (
            "APP.add_systems(Update, (c as HashMap<A, B>, ok));",
            (&[Some("ok")], &["expression"]),
            (&[Some("ok")], &["expression", "expression"]),
        ),
        (
            "APP.add_systems(Update, (c as Foo<A, <T as Tr>::M>, ok));",
            (&[Some("ok")], &["expression"]),
            (&[Some("ok")], &["expression", "expression"]),
        ),
        (
            "APP.add_systems(Update, |q: Query<&Transform, With<Player>>| {});",
            (&[None], &[]),
            (&[], &["arguments"]),
        ),
        (
            "APP.add_observer(|trigger: Trigger<OnAdd, Foo>, mut commands: Commands| {});",
            (&[None], &[]),
            (&[], &["arguments"]),
        ),
        (
            "APP.add_systems(|a: A, b: B| {});",
            (&[None], &[]),
            (&[], &["expression"]),
        ),
    ];
    let mut guessed = Vec::new();
    let mut changed = Vec::new();
    for (body, (code_names, code_categories), (token_names, token_categories)) in cases {
        for (what, source) in in_three_contexts(body) {
            let (got_names, got_categories) = observed(what, &source);
            let names_guessed: Vec<&String> = got_names
                .iter()
                .flatten()
                .filter(|name| *name != "ok")
                .collect();
            let other_categories: Vec<&&str> = got_categories
                .iter()
                .filter(|c| !matches!(**c, "expression" | "arguments"))
                .collect();
            if !names_guessed.is_empty() || !other_categories.is_empty() {
                guessed.push(format!(
                    "{what}: {body}\n  names {got_names:?}, uncertain {got_categories:?}"
                ));
            }
            let (want_names, want_categories) = if what == "code" {
                (code_names, code_categories)
            } else {
                (token_names, token_categories)
            };
            if got_names != optional(want_names) || got_categories != want_categories {
                changed.push(format!(
                    "{what}: {body}\n  names {got_names:?} (pinned {want_names:?}), \
                     uncertain {got_categories:?} (pinned {want_categories:?})"
                ));
            }
        }
    }
    assert!(
        guessed.is_empty(),
        "a fragment of a split element became a named registration:\n{}",
        guessed.join("\n")
    );
    assert!(
        changed.is_empty(),
        "the pinned token/code disagreement changed (update the pin and the report):\n{}",
        changed.join("\n")
    );
}

#[test]
fn an_unclosed_turbofish_at_an_element_end_is_read_as_its_path_in_tokens() {
    // Known gap, pinned. `a::<` / `b::<<` closed by nothing before the
    // element ends is not Rust: code reports a parse error per call, the
    // token reader keeps the path before the turbofish.
    let body = "APP.add_systems(Update, (a::<, ok)); APP.add_systems(Update, (b::<<, ok2)); \
                APP.add_systems(Update, c::<);";
    for (what, source) in in_three_contexts(body) {
        let analysis = detect(&source);
        let registered: Vec<(Option<String>, &str)> = analysis
            .registrations
            .iter()
            .map(|r| (r.name.clone(), r.text.as_str()))
            .collect();
        let categories: Vec<&str> = analysis
            .uncertain
            .iter()
            .map(|u| u.category.as_str())
            .collect();
        assert_eq!(analysis.call_sites.add_systems, 3, "{what}: calls counted");
        if what == "code" {
            assert!(analysis.has_error, "code: the source does not parse");
            assert_eq!(registered, vec![], "code: nothing registered");
            assert_eq!(categories, vec!["parse_error"; 3], "code: one per call");
        } else {
            let pinned: Vec<(Option<String>, &str)> = vec![
                (Some("a".to_owned()), "a::<"),
                (Some("ok".to_owned()), "ok"),
                (Some("b".to_owned()), "b::<<"),
                (Some("ok2".to_owned()), "ok2"),
                (Some("c".to_owned()), "c::<"),
            ];
            assert_eq!(registered, pinned, "{what}: pinned gap changed\n{source}");
            assert_eq!(categories, Vec::<&str>::new(), "{what}: nothing uncertain");
        }
    }
}

// ------------------------------------------------------------ nesting cap

/// `app.add_systems(Update, ((…(leaf)…)))` with `depth` parentheses around `leaf`.
fn nested_system(depth: usize, leaf: &str) -> String {
    format!(
        "fn build(app: &mut App) {{\n    app.add_systems(Update, {}{leaf}{});\n}}\n",
        "(".repeat(depth),
        ")".repeat(depth)
    )
}

#[test]
fn nesting_up_to_the_cap_is_read_and_past_it_is_nesting_too_deep() {
    let at_cap = detect_on_stack(nested_system(MAX_NESTING, "deep"), TWO_MB);
    assert_eq!(uncertain(&at_cap), vec![]);
    assert_eq!(
        names(&at_cap),
        some(&["deep"]),
        "{MAX_NESTING} levels are read"
    );

    let past = detect_on_stack(nested_system(MAX_NESTING + 1, "deep"), TWO_MB);
    assert!(
        past.registrations.is_empty(),
        "nothing past the cap is read"
    );
    assert_eq!(
        uncertain(&past),
        vec![("nesting_too_deep", "code", "add_systems")]
    );
}

#[test]
fn deep_nesting_in_code_macros_and_plugins_is_bounded_on_a_two_megabyte_stack() {
    let depth = 20_000;
    let open = "(".repeat(depth);
    let close = ")".repeat(depth);
    let source = format!(
        "fn build(app: &mut App) {{\n\
         \x20   app.add_systems(Update, (first, {open}deep{close}, last));\n\
         \x20   app.add_plugins((P1, {open}DeepPlugin{close}));\n\
         }}\n\
         macro_rules! deep {{\n\
         \x20   ($app:expr) => {{\n\
         \x20       $app.add_systems(Update, (m_first, {open}m_deep{close}));\n\
         \x20       $app.add_plugins({open}MDeep{close});\n\
         \x20   }};\n\
         }}\n"
    );
    let analysis = detect_on_stack(source, TWO_MB);
    assert_eq!(
        names(&analysis),
        some(&["first", "last", "m_first"]),
        "the siblings of a too-deep element are still read"
    );
    assert_eq!(plugin_use_names(&analysis), some(&["P1"]));
    let categories: Vec<&str> = analysis
        .uncertain
        .iter()
        .map(|u| u.category.as_str())
        .collect();
    assert_eq!(
        categories,
        vec!["nesting_too_deep"; 4],
        "one nesting_too_deep per registration argument that went past the cap"
    );
    let origins: Vec<Origin> = analysis.uncertain.iter().map(|u| u.origin).collect();
    assert_eq!(
        origins,
        vec![
            Origin::Code,
            Origin::Code,
            Origin::MacroRules,
            Origin::MacroRules
        ]
    );
    for item in &analysis.uncertain {
        assert!(
            item.text.len() <= MAX_TEXT_BYTES + TRUNCATION_MARK.len_utf8(),
            "uncertain text is capped"
        );
    }
}

// ----------------------------------------------------------- determinism

#[test]
fn detection_is_deterministic() {
    let first = detect(EVERY_COMBINATOR_AND_ADAPTER);
    let second = detect(EVERY_COMBINATOR_AND_ADAPTER);
    assert_eq!(first, second);
}

// -------------------------------------------------------------- cost

/// Parses once, then times only the detector: the fastest of three runs on
/// a 2 MB-stack thread.
fn detector_time(source: String) -> (BevyAnalysis, Duration) {
    thread::Builder::new()
        .name("bevy-cost".to_owned())
        .stack_size(TWO_MB)
        .spawn(move || {
            let mut parser = RustParser::new().expect("grammar loads");
            let tree = parser.parse(&source).expect("parsed");
            let mut best = Duration::MAX;
            let mut analysis = None;
            for _ in 0..3 {
                let start = Instant::now();
                let result = bevy::detect(&tree, &source);
                best = best.min(start.elapsed());
                if let Some(previous) = &analysis {
                    assert_eq!(previous, &result, "detection is deterministic");
                }
                analysis = Some(result);
            }
            (analysis.unwrap(), best)
        })
        .expect("cost thread spawns")
        .join()
        .expect("the detector must not panic or overflow")
}

/// `large` is `scale` times `small`'s input: its time stays within
/// `scale × 4` times the small time plus a fixed allowance for noise.
fn assert_linear(what: &str, small: Duration, large: Duration, scale: u32) {
    let bound = small * scale * 4 + Duration::from_millis(250);
    eprintln!("{what}: small {small:?}, large {large:?} (×{scale}), bound {bound:?}");
    assert!(
        large <= bound,
        "{what}: {large:?} for ×{scale} input against {small:?} (bound {bound:?}) — \
         the detector is not linear"
    );
}

/// Shapes a quadratic walk would expose, each generated at `n` units, with
/// the registrations expected (`None`: not asserted here).
fn shapes(n: usize) -> Vec<(&'static str, String, Option<usize>)> {
    let wide_tuple = format!(
        "fn build(app: &mut App) {{\n    app.add_systems(Update, ({}));\n}}\n",
        (0..n)
            .map(|i| format!("s{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let long_chain = format!(
        "fn build(app: &mut App) {{\n    app.add_systems(Update, tick{});\n}}\n",
        ".run_if(c)".repeat(n)
    );
    let chained_calls = format!(
        "fn build(app: &mut App) {{\n    app{};\n}}\n",
        (0..n)
            .map(|i| format!(".add_systems(Update, s{i})"))
            .collect::<String>()
    );
    let broken_chain = format!(
        "fn build(app: &mut App) {{\n    app.add_systems(Update, (a b)){};\n}}\n",
        (0..n)
            .map(|i| format!(".add_systems(Update, s{i})"))
            .collect::<String>()
    );
    let macro_body = format!(
        "macro_rules! many {{\n    ($app:expr) => {{\n{}    }};\n}}\n",
        (0..n)
            .map(|i| format!("        $app.add_systems(Update, (s{i}, t{i}.run_if(c)));\n"))
            .collect::<String>()
    );
    let macro_wide = format!(
        "fn build(app: &mut App) {{\n    wrap! {{ app.add_systems(Update, ({})); }}\n}}\n",
        (0..n)
            .map(|i| format!("s{i}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let macro_deep = format!(
        "macro_rules! deep {{\n    ($app:expr) => {{ $app.add_systems(Update, {}x{}); }};\n}}\n",
        "(".repeat(n),
        ")".repeat(n)
    );
    // Unclosed `<` a rescan-per-`<` would walk to the end of the tree from
    // every position; each shape ends with one readable turbofish call.
    let fn_angles_in_macro_body = format!(
        "macro_rules! m {{\n    ($app:expr) => {{\n        {}\n        $app.add_systems::<T>(Update, after);\n    }};\n}}\n",
        "fn a < ".repeat(n)
    );
    let turbofish_angles_in_macro_body = format!(
        "macro_rules! m {{\n    ($app:expr) => {{\n        {}\n        $app.add_systems::<T>(Update, after);\n    }};\n}}\n",
        "$app.add_systems::< ".repeat(n)
    );
    let plugin_path_angles_in_macro_body = format!(
        "macro_rules! m {{\n    ($app:expr) => {{\n        {}\n        $app.add_systems::<T>(Update, after);\n    }};\n}}\n",
        "impl Plugin for x::< ".repeat(n)
    );
    let fn_angles_in_macro_arguments = format!(
        "fn build(app: &mut App) {{\n    wrap! {{ {} app.add_systems::<T>(Update, after); }}\n}}\n",
        "fn a < ".repeat(n)
    );
    let comparisons_in_code = format!(
        "fn build(app: &mut App) {{\n    let _ = ({});\n    app.add_systems::<T>(Update, after);\n}}\n",
        "a < b, ".repeat(n)
    );
    // `::<<` and `<<` never closed: the turbofish of a call, of a plugin
    // path, at an element's end, and `<<` opening an element.
    let double_turbofish_calls_in_macro_body = format!(
        "macro_rules! m {{\n    ($app:expr) => {{\n        {}\n        $app.add_systems::<T>(Update, after);\n    }};\n}}\n",
        "$app.add_systems::<< ".repeat(n)
    );
    let double_turbofish_calls_in_macro_arguments = format!(
        "fn build(app: &mut App) {{\n    wrap! {{ {} app.add_systems::<T>(Update, after); }}\n}}\n",
        "app.add_systems::<< ".repeat(n)
    );
    let double_turbofish_plugin_paths_in_macro_body = format!(
        "macro_rules! m {{\n    ($app:expr) => {{\n        {}\n        $app.add_systems::<T>(Update, after);\n    }};\n}}\n",
        "impl Plugin for x::<< ".repeat(n)
    );
    let double_turbofish_element_ends_in_macro_arguments = format!(
        "fn build(app: &mut App) {{\n    wrap! {{ app.add_systems(Update, ({})); }}\n}}\n",
        "s::<<, ".repeat(n)
    );
    let double_angle_element_starts_in_macro_body = format!(
        "macro_rules! m {{\n    ($app:expr) => {{ $app.add_systems(Update, ({})); }};\n}}\n",
        "<< s, ".repeat(n)
    );
    let many_fns = (0..n)
        .map(|i| format!("fn f{i}(app: &mut App) {{ app.add_systems(Update, || {{}}); }}\n"))
        .collect::<String>();
    vec![
        ("wide tuple", wide_tuple, Some(n)),
        ("long combinator chain", long_chain, Some(1)),
        ("chained add_systems calls", chained_calls, Some(n)),
        ("chained calls over a parse error", broken_chain, Some(n)),
        ("macro_rules body", macro_body, Some(2 * n)),
        ("wide tuple in macro arguments", macro_wide, Some(n)),
        ("deep groups in a macro body", macro_deep, Some(0)),
        ("many fns with closures", many_fns, Some(n)),
        (
            "unclosed `fn a <` in a macro body",
            fn_angles_in_macro_body,
            Some(1),
        ),
        (
            "unclosed turbofish calls in a macro body",
            turbofish_angles_in_macro_body,
            Some(1),
        ),
        (
            "unclosed `impl Plugin for x::<` in a macro body",
            plugin_path_angles_in_macro_body,
            Some(1),
        ),
        (
            "unclosed `fn a <` in macro arguments",
            fn_angles_in_macro_arguments,
            Some(1),
        ),
        ("comparisons in code", comparisons_in_code, Some(1)),
        (
            "unclosed `::<<` calls in a macro body",
            double_turbofish_calls_in_macro_body,
            Some(1),
        ),
        (
            "unclosed `::<<` calls in macro arguments",
            double_turbofish_calls_in_macro_arguments,
            Some(1),
        ),
        (
            "unclosed `impl Plugin for x::<<` in a macro body",
            double_turbofish_plugin_paths_in_macro_body,
            Some(1),
        ),
        (
            "unclosed `s::<<` at element ends in macro arguments",
            double_turbofish_element_ends_in_macro_arguments,
            None,
        ),
        (
            "unclosed `<<` at element starts in a macro body",
            double_angle_element_starts_in_macro_body,
            None,
        ),
    ]
}

#[test]
fn detector_cost_is_linear_in_the_input() {
    let small_n = 2_000;
    let scale = 8;
    let small = shapes(small_n);
    let large = shapes(small_n * scale as usize);
    for ((what, small_source, small_count), (_, large_source, large_count)) in
        small.into_iter().zip(large)
    {
        let (small_analysis, small_time) = detector_time(small_source);
        let (large_analysis, large_time) = detector_time(large_source);
        if let (Some(small_count), Some(large_count)) = (small_count, large_count) {
            assert_eq!(
                small_analysis.registrations.len(),
                small_count,
                "{what}: registrations at n"
            );
            assert_eq!(
                large_analysis.registrations.len(),
                large_count,
                "{what}: registrations at {scale}n"
            );
        }
        assert_linear(what, small_time, large_time, scale);
    }
}

#[test]
fn the_uncertain_category_names_are_stable() {
    let all = [
        (UncertainCategory::MacroInArguments, "macro_in_arguments"),
        (UncertainCategory::Metavariable, "metavariable"),
        (UncertainCategory::UnknownMethod, "unknown_method"),
        (UncertainCategory::Expression, "expression"),
        (UncertainCategory::Arguments, "arguments"),
        (UncertainCategory::NestingTooDeep, "nesting_too_deep"),
        (UncertainCategory::ParseError, "parse_error"),
    ];
    for (category, name) in all {
        assert_eq!(category.as_str(), name);
    }
    assert_eq!(PluginKind::ImplPlugin.as_str(), "impl_plugin");
    assert_eq!(PluginKind::FnApp.as_str(), "fn_app");
}
