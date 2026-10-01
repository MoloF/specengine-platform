//! AC-13 of docs/features/spec-cli.md: `OUTPUT_CAP_CHARS` = 40 000
//! characters bound everything `spec show` prints before its one tail line
//! (JSON: the sum of `text`), cut at the last line end within the cap;
//! the tail names the path, the lines, the sections and the holders not
//! shown. `--limit` outside 1..=200 is exit 2.

#![cfg(unix)]

mod common;

use common::{Scratch, index, spec, write};
use specengine_cli::OUTPUT_CAP_CHARS;

const CAP: usize = 40_000;

/// A document of about `chars` characters: front-matter, a title, then
/// sections `{#<prefix>-<n>}` of `line`-built paragraphs.
fn huge(id: &str, section_prefix: &str, line: &str, chars: usize) -> String {
    let mut text = format!("---\nid: {id}\nclass: canon\n---\n\n# Huge\n\n");
    let mut n = 0;
    while text.chars().count() < chars {
        n += 1;
        text.push_str(&format!("## Part {n} {{#{section_prefix}-{n}}}\n\n"));
        for k in 0..12 {
            text.push_str(&format!("{line} {n}.{k}\n"));
        }
        text.push('\n');
    }
    text
}

/// The 1-based line of each `{#ID}` heading of `text`.
fn heading_lines(text: &str) -> Vec<(usize, String)> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let start = line.find("{#")?;
            let id = line[start + 2..].strip_suffix('}')?;
            Some((index + 1, id.to_owned()))
        })
        .collect()
}

/// Checks a capped text rendering of one node of `file` (`path`) and
/// returns the tail line.
fn check_capped_text(stdout: &str, path: &str, file: &str, holders: &str) {
    let (before, tail) = stdout
        .strip_suffix('\n')
        .and_then(|body| body.rsplit_once('\n'))
        .map(|(before, tail)| (format!("{before}\n"), tail))
        .expect("a tail line");
    assert_eq!(stdout.matches("[truncated: ").count(), 1, "one tail line");
    assert!(tail.starts_with("[truncated: "), "{tail}");
    let chars = before.chars().count();
    assert!(chars <= CAP, "{chars} characters before the tail");
    let (header, shown) = before.split_once('\n').unwrap();
    assert!(header.contains(&format!(" | {path}:1 | ")), "{header}");
    assert!(
        file.starts_with(shown),
        "the shown text is the file's start"
    );
    assert!(shown.ends_with('\n'), "cut at a line end");
    // The last line end within the cap: the next line would not fit.
    let next = &file[shown.len()..];
    let next_line = next.split_inclusive('\n').next().unwrap();
    assert!(
        chars + next_line.chars().count() > CAP,
        "{chars} + {} fits: not the last line end",
        next_line.chars().count()
    );
    let first_hidden = shown.matches('\n').count() + 1;
    let last = file.lines().count();
    let hidden_sections: Vec<String> = heading_lines(file)
        .into_iter()
        .filter(|(line, _)| *line >= first_hidden)
        .map(|(_, id)| id)
        .collect();
    assert!(!hidden_sections.is_empty());
    assert_eq!(
        tail,
        format!(
            "[truncated: {path} lines {first_hidden}-{last} not shown; sections not shown: {}; holders not shown: {holders}]",
            hidden_sections.join(", ")
        )
    );
}

#[test]
fn the_cap_constant_is_40_000() {
    assert_eq!(OUTPUT_CAP_CHARS, CAP);
}

#[test]
fn a_100_000_character_document_is_cut_at_a_line_end_with_one_tail_line() {
    // spec-a: ASCII prose; spec-b: Cyrillic prose, so characters are not
    // bytes (the cap counts characters).
    for (fixture, id, prefix, line, path) in [
        (
            "spec-a",
            "MEC-HUGE",
            "RULE-HUGE",
            "Plain filler words for a very long mechanic text, line",
            "docs/spec/huge.md",
        ),
        (
            "spec-b",
            "MOD-HUGE",
            "CMD-HUGE",
            "\u{0414}\u{043b}\u{0438}\u{043d}\u{043d}\u{044b}\u{0439} \u{0442}\u{0435}\u{043a}\u{0441}\u{0442} \u{043c}\u{043e}\u{0434}\u{0443}\u{043b}\u{044f}, \u{0441}\u{0442}\u{0440}\u{043e}\u{043a}\u{0430}",
            "docs/spec/huge.md",
        ),
    ] {
        let scratch = Scratch::new("bounds");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let text = huge(id, prefix, line, 100_000);
        assert!(text.chars().count() >= 100_000);
        write(&root, path, &text);
        index(&home, &root);

        let run = spec(&home, &root, &["show", id]);
        run.code(0);
        check_capped_text(&run.stdout, path, &text, "none");
        // By path, the same.
        let by_path = spec(&home, &root, &["show", path]);
        assert_eq!(by_path.stdout, run.stdout, "{fixture}");

        let json = spec(&home, &root, &["--json", "show", id]);
        json.code(0);
        let json = json.json();
        let node = &json["nodes"][0];
        assert_eq!(node["truncated"], true, "{fixture}");
        let shown = node["text"].as_str().unwrap();
        assert!(shown.chars().count() <= CAP, "{fixture}: JSON text");
        assert!(
            text.starts_with(shown) && shown.ends_with('\n'),
            "{fixture}"
        );
        let first_hidden = shown.matches('\n').count() + 1;
        assert_eq!(
            node["omitted"]["lines"],
            serde_json::json!([first_hidden, text.lines().count()]),
            "{fixture}"
        );
        assert_eq!(
            node["omitted"]["holders"],
            serde_json::json!([]),
            "{fixture}"
        );
        let sections: Vec<String> = heading_lines(&text)
            .into_iter()
            .filter(|(line, _)| *line >= first_hidden)
            .map(|(_, id)| id)
            .collect();
        assert_eq!(
            node["omitted"]["sections"],
            serde_json::json!(sections),
            "{fixture}"
        );
        // A small node is never truncated.
        let small = spec(&home, &root, &["--json", "show", &format!("{prefix}-1")]).json();
        assert_eq!(small["nodes"][0]["truncated"], false);
        assert!(small["nodes"][0]["omitted"].is_null());
    }
}

/// Two holders, the first huge: the tail names the second as not shown;
/// JSON drops it and lists it in `omitted.holders`.
#[test]
fn holders_after_the_cut_are_named_in_the_tail() {
    let scratch = Scratch::new("bounds-holders");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let mut big =
        String::from("---\nclass: spec\nstatus: draft\n---\n\n# Aa big\n\n### Big {#AC-50}\n\n");
    for k in 0..2000 {
        big.push_str(&format!(
            "A line of filler text for the big criterion, number {k}.\n"
        ));
    }
    write(&root, "docs/features/aa-big.md", &big);
    write(
        &root,
        "docs/features/zz-small.md",
        "---\nclass: spec\nstatus: draft\n---\n\n# Zz small\n\n### Small {#AC-50}\n\nShort.\n",
    );
    index(&home, &root);
    let run = spec(&home, &root, &["show", "AC-50"]);
    run.code(0);
    let tail = run.stdout.lines().last().unwrap();
    assert!(
        tail.starts_with("[truncated: docs/features/aa-big.md lines ")
            && tail.ends_with(
                "; sections not shown: none; holders not shown: docs/features/zz-small.md:8]"
            ),
        "{tail}"
    );
    assert!(
        !run.stdout.contains("Short."),
        "the second holder is not printed"
    );
    let before = run.stdout.strip_suffix(&format!("{tail}\n")).unwrap();
    assert!(before.chars().count() <= CAP);
    assert_eq!(
        run.stderr_lines().len(),
        1,
        "the several-holders warning\n{}",
        run.show()
    );

    let json = spec(&home, &root, &["--json", "show", "AC-50"]).json();
    let nodes = json["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1, "the nodes after the cut are dropped");
    assert_eq!(nodes[0]["truncated"], true);
    assert_eq!(
        nodes[0]["omitted"]["holders"],
        serde_json::json!(["docs/features/zz-small.md:8"])
    );
    assert_eq!(nodes[0]["omitted"]["sections"], serde_json::json!([]));
}

/// A first line longer than the cap is cut at the cap itself (text: one
/// character earlier, so the added line end fits).
#[test]
fn a_first_line_longer_than_the_cap_is_cut_at_the_cap() {
    let scratch = Scratch::new("bounds-line");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    // No heading: the title stays `-`, the long line is the text's first.
    let text = format!("{}\n\nMore.\n", "x".repeat(60_000));
    write(&root, "docs/spec/long.md", &text);
    index(&home, &root);
    let run = spec(&home, &root, &["show", "docs/spec/long.md"]);
    run.code(0);
    let tail = run.stdout.lines().last().unwrap();
    assert!(
        tail.starts_with("[truncated: docs/spec/long.md lines 1-3 not shown"),
        "{tail}"
    );
    let before = run.stdout.strip_suffix(&format!("{tail}\n")).unwrap();
    assert!(before.ends_with('\n'));
    assert_eq!(before.chars().count(), CAP, "cut at the cap");
    let json = spec(&home, &root, &["--json", "show", "docs/spec/long.md"]).json();
    let shown = json["nodes"][0]["text"].as_str().unwrap();
    assert_eq!(shown.chars().count(), CAP);
    assert!(text.starts_with(shown));
    assert_eq!(
        json["nodes"][0]["omitted"]["lines"],
        serde_json::json!([1, 3])
    );
}

/// A header line longer than the cap (a heading of 60 000 characters is
/// the node's title): "a longer first line: at the cap" — something of the
/// node is still printed before the tail, never the tail alone.
#[test]
fn a_header_longer_than_the_cap_is_cut_at_the_cap_not_dropped() {
    let scratch = Scratch::new("bounds-header");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let text = format!("# {}\n\nMore.\n", "x".repeat(60_000));
    write(&root, "docs/spec/long.md", &text);
    index(&home, &root);
    let run = spec(&home, &root, &["show", "docs/spec/long.md"]);
    run.code(0);
    let tail = run.stdout.lines().last().unwrap();
    assert!(
        tail.starts_with("[truncated: docs/spec/long.md lines 1-3 not shown"),
        "{tail}"
    );
    let before = run.stdout.strip_suffix(&format!("{tail}\n")).unwrap();
    assert!(before.chars().count() <= CAP, "{}", before.chars().count());
    assert!(
        before.starts_with("docs/spec/long.md | - | # xxx")
            || before.starts_with("docs/spec/long.md | - | xxx"),
        "the header is cut at the cap, not dropped: {} characters before the tail",
        before.chars().count()
    );
}

#[test]
fn limit_outside_1_to_200_is_exit_2() {
    let scratch = Scratch::new("bounds-limit");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    for limit in ["0", "201", "-1", "1000000000000", "many"] {
        for json in [false, true] {
            let mut args = vec!["search", "stamina", "--limit", limit];
            if json {
                args.insert(0, "--json");
            }
            let run = spec(&home, &root, &args);
            run.code(2);
            assert_eq!(run.stdout, "", "--limit {limit}");
            assert!(run.stderr.starts_with("spec: "), "{}", run.show());
            assert!(run.stderr_lines()[0].contains("limit"), "{}", run.show());
            if limit != "many" {
                // A number out of range is the command's own one-line error;
                // a non-number is clap's usage error.
                assert_eq!(
                    run.stderr_lines().len(),
                    1,
                    "--limit {limit}: {}",
                    run.show()
                );
            }
        }
    }
}

/// The two text lines `spec search` prints for `hit` (the Data format), as
/// the test rebuilds them from the store's own hit.
fn hit_block(hit: &specengine_store::SearchHit) -> String {
    let flat = |text: &str| text.replace("\r\n", " ").replace(['\n', '\r'], " ");
    let name = hit.id.clone().unwrap_or_else(|| hit.path.clone());
    let kind = hit.kind.as_deref().map_or_else(|| "-".to_owned(), flat);
    let title = hit.title.as_deref().map_or_else(|| "-".to_owned(), flat);
    let archived = if hit.tier3 { " | archived" } else { "" };
    format!(
        "{name} | {kind} | {title} | {}:{}{archived}\n    {}\n",
        hit.path,
        hit.line,
        flat(&hit.snippet)
    )
}

/// Iteration 2, item (3): 220 long-titled nodes match; with `--limit 200`
/// the store gives 200 hits, and `spec search` prints the best ones whose
/// lines (with the summary) fit in 40 000 characters, cut at a hit
/// boundary; the summary counts all 200; one tail line and one note name
/// the rest; `--json` holds the same hits; reruns are byte-identical.
#[test]
fn search_output_is_capped_at_a_hit_boundary() {
    let scratch = Scratch::new("bounds-search");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let term = "cinderglow";
    let words = "lantern keeper walks the long corridor while the embers fade ".repeat(6);
    for n in 0..220 {
        write(
            &root,
            &format!("docs/records/big/b-{n:03}.md"),
            format!("---\nclass: canon\n---\n\n# {term} {n:03} {words}\n\nThe {term} note {n}.\n"),
        );
    }
    index(&home, &root);

    let args = ["search", term, "--limit", "200"];
    let run = spec(&home, &root, &args);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    let tail = *lines.last().unwrap();
    let summary = lines[lines.len() - 2];
    assert_eq!(
        summary,
        "hits 200 (limit 200); archived matches left out: 0 (--archive)"
    );
    let shown = (lines.len() - 2) / 2;
    assert_eq!(
        lines.len(),
        shown * 2 + 2,
        "two lines per hit, the summary, the tail"
    );
    assert!(shown > 0 && shown < 200, "{shown} hits shown");
    assert_eq!(
        tail,
        format!(
            "[truncated: {} of 200 hits not shown; lower --limit or narrow the query]",
            200 - shown
        )
    );
    let before = run.stdout.strip_suffix(&format!("{tail}\n")).unwrap();
    let used = before.chars().count();
    assert!(used <= CAP, "{used} characters before the tail");

    // The store's own order, cut at a hit boundary: the next hit would not fit.
    let db = common::data_dir(&home).join("lantern-keep.db");
    let store = specengine_store::SqliteIndex::open(&db, "lantern-keep", &root).expect("open");
    let mut query = specengine_store::SearchQuery::new(term);
    query.limit = 200;
    let hits = specengine_store::SpecIndex::search(&store, &query)
        .expect("search")
        .hits;
    assert_eq!(hits.len(), 200);
    let mut want = String::new();
    for hit in &hits[..shown] {
        want.push_str(&hit_block(hit));
    }
    want.push_str(summary);
    want.push('\n');
    assert_eq!(before, want, "the store's first {shown} hits, as printed");
    assert!(
        used + hit_block(&hits[shown]).chars().count() > CAP,
        "hit {} would still fit: {used} + {}",
        shown + 1,
        hit_block(&hits[shown]).chars().count()
    );

    // The note, on stderr and in JSON `notes`.
    let notes: Vec<&str> = run.stderr_lines();
    assert_eq!(notes.len(), 1, "{}", run.show());
    assert!(
        notes[0].starts_with("note: ")
            && notes[0].contains(&format!("{} of 200 hits not shown", 200 - shown)),
        "{}",
        run.show()
    );
    let json_run = spec(&home, &root, &["--json", "search", term, "--limit", "200"]);
    json_run.code(0);
    let json = json_run.json();
    let json_hits = json["hits"].as_array().unwrap();
    assert_eq!(json_hits.len(), shown, "the same hits in text and JSON");
    for (hit, store_hit) in json_hits.iter().zip(&hits) {
        assert_eq!(hit["path"], store_hit.path.as_str());
        assert_eq!(hit["ord"], store_hit.ord);
    }
    assert_eq!(json["notes"].as_array().unwrap().len(), 1, "{json}");
    assert_eq!(
        format!("note: {}", json["notes"][0].as_str().unwrap()),
        notes[0],
        "the same note"
    );
    let mut keys: Vec<&str> = json
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "archive",
            "hits",
            "kinds",
            "limit",
            "notes",
            "query",
            "tier3_left_out",
            "truncated"
        ],
        "the key set"
    );
    assert_eq!(json["truncated"], true);
    assert_eq!(json_run.stderr, run.stderr);

    // Byte-identical reruns.
    assert_eq!(spec(&home, &root, &args).stdout, run.stdout);
    assert_eq!(
        spec(&home, &root, &["--json", "search", term, "--limit", "200"]).stdout,
        json_run.stdout
    );
    // A limit that fits prints no tail and no note.
    let small = spec(&home, &root, &["search", term, "--limit", "5"]);
    small.code(0);
    assert!(!small.stdout.contains("[truncated:"), "{}", small.show());
    assert_eq!(small.stderr, "");
    assert!(
        small
            .stdout
            .ends_with("hits 5 (limit 5); archived matches left out: 0 (--archive)\n")
    );
}

/// Iteration 3, item (2): the first hit alone passes the cap (a title of
/// 60 000 characters). It is still shown, its title (then snippet) cut at a
/// character boundary so everything before the tail fits; text and JSON
/// agree on the cut; `truncated: true`; the note and tail say so.
#[test]
fn a_first_hit_longer_than_the_cap_is_shown_cut() {
    let scratch = Scratch::new("bounds-first-hit");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let term = "emberquill";
    // Multi-byte characters, so the cut must fall on a character boundary.
    let title = format!("{term} {}", "\u{00e9}t\u{00e9} ".repeat(15_000));
    write(
        &root,
        "docs/records/huge-title.md",
        format!("---\nclass: canon\n---\n\n# {title}\n\n{term} {term} {term}.\n"),
    );
    // The only match (a shorter one would outrank it: bm25 weighs length).
    index(&home, &root);

    let run = spec(&home, &root, &["search", term]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(
        lines.len(),
        4,
        "the one hit (two lines), summary, tail\n{}",
        run.stdout.chars().take(300).collect::<String>()
    );
    let tail = lines[3];
    assert_eq!(
        tail,
        "[truncated: the first hit's title and snippet cut at the cap; 0 of 1 hits not shown; \
         lower --limit or narrow the query]"
    );
    assert_eq!(
        lines[2],
        "hits 1 (limit 20); archived matches left out: 0 (--archive)"
    );
    let before = run.stdout.strip_suffix(&format!("{tail}\n")).unwrap();
    let used = before.chars().count();
    assert!(
        (CAP - 1..=CAP).contains(&used),
        "{used} characters before the tail"
    );
    let notes = run.stderr_lines();
    assert_eq!(notes.len(), 1, "{}", run.stderr);
    assert!(
        notes[0].starts_with(
            "note: the first hit's title and snippet cut at the cap; 0 of 1 hits not shown"
        ),
        "{}",
        run.stderr
    );

    let json_run = spec(&home, &root, &["--json", "search", term]);
    json_run.code(0);
    let json = json_run.json();
    assert_eq!(json["truncated"], true);
    let hits = json["hits"].as_array().unwrap();
    assert_eq!(hits.len(), 1, "the first hit only");
    assert_eq!(hits[0]["path"], "docs/records/huge-title.md");
    let cut_title = hits[0]["title"].as_str().unwrap();
    let snippet = hits[0]["snippet"].as_str().unwrap();
    assert!(
        title.starts_with(cut_title) && cut_title.len() < title.len() && !cut_title.is_empty(),
        "the title is cut to a prefix ({} of {} characters)",
        cut_title.chars().count(),
        title.chars().count()
    );
    // Text and JSON agree on the cut title and snippet.
    let flat = |text: &str| text.replace("\r\n", " ").replace(['\n', '\r'], " ");
    assert_eq!(
        lines[0],
        format!(
            "docs/records/huge-title.md | - | {} | docs/records/huge-title.md:1",
            flat(cut_title)
        )
    );
    assert_eq!(lines[1], format!("    {}", flat(snippet)));
    assert_eq!(
        format!("note: {}", json["notes"][0].as_str().unwrap()),
        notes[0],
        "the same note"
    );
    // Reruns are byte-identical.
    assert_eq!(spec(&home, &root, &["search", term]).stdout, run.stdout);
    assert_eq!(
        spec(&home, &root, &["--json", "search", term]).stdout,
        json_run.stdout
    );
}

/// Iteration 4, items (3)+(4): a first hit whose front-matter `kind:` is
/// 60 000 characters. The snippet is kept, then name, kind and title share
/// the room shortest-first, so only the kind is cut; everything before the
/// tail is exactly the cap (the `-` a cut field would print is accounted);
/// text and JSON agree; `truncated: true`.
#[test]
fn a_first_hit_with_a_huge_kind_keeps_its_title() {
    let scratch = Scratch::new("bounds-kind");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let kind = "k".repeat(60_000);
    write(
        &root,
        "docs/spec/longkind.md",
        format!(
            "---\nclass: canon\nkind: {kind}\n---\n\n# Quasarfen title\n\nThe quasarfen text.\n"
        ),
    );
    index(&home, &root);
    let run = spec(&home, &root, &["search", "quasarfen"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    assert_eq!(lines.len(), 4, "one hit, summary, tail");
    let tail = lines[3];
    assert!(tail.starts_with("[truncated: "), "{tail}");
    let before = run.stdout.strip_suffix(&format!("{tail}\n")).unwrap();
    assert_eq!(
        before.chars().count(),
        CAP,
        "exactly the cap before the tail"
    );
    let fields: Vec<&str> = lines[0].split(" | ").collect();
    assert_eq!(fields.len(), 4, "name | kind | title | path:line");
    assert_eq!(fields[0], "docs/spec/longkind.md");
    assert!(kind.starts_with(fields[1]) && fields[1].len() < kind.len() && !fields[1].is_empty());
    assert_eq!(fields[2], "Quasarfen title", "the title is kept whole");
    assert_eq!(fields[3], "docs/spec/longkind.md:1");

    let json = spec(&home, &root, &["--json", "search", "quasarfen"]).json();
    assert_eq!(json["truncated"], true);
    let hit = &json["hits"][0];
    assert_eq!(hit["kind"], fields[1], "JSON kind cut as the text's");
    assert_eq!(hit["title"], "Quasarfen title");
    assert!(hit["id"].is_null());
    assert_eq!(
        lines[1],
        format!(
            "    {}",
            hit["snippet"].as_str().unwrap().replace('\n', " ")
        )
    );
}

/// Iteration 4: a first hit whose name-shaped ID is about 51 000
/// characters: the ID (the hit's name) is cut to fit; kind and title kept;
/// JSON `id` is the same cut prefix.
#[test]
fn a_first_hit_with_a_huge_id_is_cut_to_fit() {
    let scratch = Scratch::new("bounds-id");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let segments: Vec<String> = (0..3000).map(|n| format!("LONGSEGMENT{n:05}")).collect();
    let id = format!("RULE-{}", segments.join("-"));
    assert!(id.len() > 50_000);
    write(
        &root,
        "docs/spec/longid.md",
        format!(
            "---\nclass: canon\n---\n\n# Holder\n\n## Glimmerwock rule {{#{id}}}\n\nThe glimmerwock text.\n"
        ),
    );
    index(&home, &root);
    let run = spec(&home, &root, &["search", "glimmerwock"]);
    run.code(0);
    let lines: Vec<&str> = run.stdout.lines().collect();
    let tail = *lines.last().unwrap();
    assert!(tail.starts_with("[truncated: "), "{tail}");
    let before = run.stdout.strip_suffix(&format!("{tail}\n")).unwrap();
    assert!(before.chars().count() <= CAP, "{}", before.chars().count());
    // The ID is the first hit (the heading holds the term in its title).
    let fields: Vec<&str> = lines[0].split(" | ").collect();
    assert_eq!(fields.len(), 4, "name | kind | title | path:line");
    assert!(id.starts_with(fields[0]) && fields[0].len() < id.len() && fields[0].len() > 30_000);
    assert_eq!(fields[1], "rule");
    assert_eq!(fields[2], "Glimmerwock rule");
    assert_eq!(fields[3], "docs/spec/longid.md:7");
    let json = spec(&home, &root, &["--json", "search", "glimmerwock"]).json();
    assert_eq!(json["truncated"], true);
    assert_eq!(
        json["hits"][0]["id"], fields[0],
        "JSON id cut as the text's name"
    );
    assert_eq!(json["hits"][0]["kind"], "rule");
    assert_eq!(json["hits"][0]["title"], "Glimmerwock rule");
    // `show` of the full ID still answers (the cap there is its own).
    let shown = spec(&home, &root, &["show", &id]);
    shown.code(0);
}

// docs/features/spec-cli-graph.md AC-15: `OUTPUT_CAP_CHARS` cuts `spec
// tree` and `spec graph` at a node or edge line (the first item whole) and
// `spec show --links` at a link line; the links block precedes the text,
// so a long node keeps it; text and JSON hold the same tree nodes.

/// `(text, lines)` of a document `id` of `count` sections `{#<prefix>-<n>}`
/// titled `<title> <n>`, four lines each from line 9, each mentioning
/// `mention`; `parent` on line 4 (else an `owner:` line), and the node
/// line each section prints at `depth` (two spaces per level).
fn big_document(
    id: &str,
    parent: Option<&str>,
    (prefix, kind, title): (&str, &str, &str),
    count: usize,
    mention: &str,
    path: &str,
    depth: usize,
) -> (String, Vec<String>) {
    let line4 = parent.map_or_else(|| "owner: o".to_owned(), |p| format!("parent: {p}"));
    let mut text = format!("---\nid: {id}\nclass: canon\n{line4}\n---\n\n# Big\n\n");
    let mut lines = Vec::new();
    for n in 1..=count {
        text.push_str(&format!(
            "## {title} {n} {{#{prefix}-{n}}}\n\nSee {mention} {n}.\n\n"
        ));
        lines.push(format!(
            "{}{prefix}-{n} | {kind} | {title} {n} | {path}:{}",
            "  ".repeat(depth),
            9 + 4 * (n - 1)
        ));
    }
    (text, lines)
}

/// Splits a capped `tree`/`graph` stdout into (everything before the tail,
/// the tail), asserting one tail line, last.
fn split_tail(stdout: &str) -> (&str, &str) {
    let body = stdout.strip_suffix('\n').expect("a line end");
    let (before, tail) = body.rsplit_once('\n').expect("a tail line");
    assert!(tail.starts_with("[truncated: "), "{tail}");
    assert_eq!(stdout.matches("[truncated: ").count(), 1, "one tail line");
    (&stdout[..before.len() + 1], tail)
}

#[test]
fn a_tree_over_the_cap_is_cut_at_a_node_line() {
    let cyrillic =
        "\u{0427}\u{0430}\u{0441}\u{0442}\u{044c} \u{043d}\u{043e}\u{043c}\u{0435}\u{0440}";
    for fixture in ["spec-a", "spec-b"] {
        let scratch = Scratch::new("bounds-tree");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let path = "docs/spec/big.md";
        // The whole tree, in order, as it would print uncut.
        let (text, full, roots) = if fixture == "spec-a" {
            let (text, sections) = big_document(
                "MEC-BIG",
                Some("DOM-GAME"),
                ("RULE-BIG", "rule", "Part"),
                1000,
                "MEC-STAMINA",
                path,
                2,
            );
            let example: Vec<String> = [
                "DOM-GAME | domain | Lantern Keep | docs/spec/game.md:1 | status accepted",
                "  RULE-CORE-LOOP | rule | Core loop | docs/spec/game.md:21",
                "  MEC-BIG | mechanic | Big | docs/spec/big.md:1",
            ]
            .iter()
            .map(|line| (*line).to_owned())
            .collect();
            let mut full = example;
            full.extend(sections);
            full.push("  DOM-MOVEMENT | domain | Movement | docs/spec/movement/README.md:1 | status accepted".to_owned());
            (text, full, 1)
        } else {
            let (text, sections) = big_document(
                "MOD-BIG",
                None,
                ("CMD-BIG", "command", cyrillic),
                1000,
                "REQ-001",
                path,
                1,
            );
            let mut full = vec!["MOD-BIG | module | Big | docs/spec/big.md:1".to_owned()];
            full.extend(sections);
            full.push("MOD-CLI | module | \u{041a}\u{043e}\u{043c}\u{0430}\u{043d}\u{0434}\u{043d}\u{0430}\u{044f} \u{0441}\u{0442}\u{0440}\u{043e}\u{043a}\u{0430} | docs/spec/cli.md:1 | status accepted".to_owned());
            (text, full, 2)
        };
        write(&root, path, &text);
        let total = if fixture == "spec-a" { 1011 } else { 1005 };
        let run = spec(&home, &root, &["tree"]);
        run.code(0);
        let (before, tail) = split_tail(&run.stdout);
        let chars = before.chars().count();
        assert!(
            chars <= CAP,
            "{fixture}: {chars} characters before the tail"
        );
        let mut lines: Vec<&str> = before.lines().collect();
        let summary = lines.pop().unwrap();
        assert_eq!(
            summary,
            format!("nodes {total}, roots {roots}"),
            "{fixture}"
        );
        let shown = lines.len();
        assert!(shown > 100 && shown < total, "{fixture}: {shown} shown");
        assert_eq!(
            lines,
            full[..shown].iter().map(String::as_str).collect::<Vec<_>>(),
            "{fixture}: the shown lines are the tree's first ones"
        );
        // Cut at the last node line that fits.
        assert!(
            chars + full[shown].chars().count() + 1 > CAP,
            "{fixture}: the next node line fits ({chars} + {})",
            full[shown].chars().count() + 1
        );
        assert_eq!(
            tail,
            format!(
                "[truncated: {} of {total} nodes not shown; give a ROOT, lower --depth or add --kind]",
                total - shown
            ),
            "{fixture}"
        );
        // JSON: the same nodes, `truncated: true`.
        let json = spec(&home, &root, &["--json", "tree"]).json();
        assert_eq!(json["truncated"], true, "{fixture}");
        let nodes = json["nodes"].as_array().unwrap();
        assert_eq!(nodes.len(), shown, "{fixture}: JSON nodes vs text");
        for (node, line) in nodes.iter().zip(&lines) {
            let name = node["id"].as_str().unwrap();
            let depth = node["depth"].as_u64().unwrap() as usize;
            assert!(
                line.starts_with(&format!("{}{name} | ", "  ".repeat(depth))),
                "{fixture}: {node} vs {line}"
            );
        }
        // A small tree is never truncated.
        let json = spec(&home, &root, &["--json", "tree", "--depth", "0"]).json();
        assert_eq!(json["truncated"], false, "{fixture}");
    }
}

/// The first node line is shown whole, however long.
#[test]
fn a_first_tree_node_longer_than_the_cap_is_shown_whole() {
    let scratch = Scratch::new("bounds-tree-first");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let title = "Lanternwick ".repeat(4_000);
    let title = title.trim_end();
    write(
        &root,
        "docs/spec/aaa.md",
        format!("---\nid: DOM-AAA\nclass: canon\n---\n\n# {title}\n"),
    );
    let run = spec(&home, &root, &["tree"]);
    run.code(0);
    let (before, tail) = split_tail(&run.stdout);
    let first = before.lines().next().unwrap();
    assert_eq!(
        first,
        format!("DOM-AAA | domain | {title} | docs/spec/aaa.md:1"),
        "the first node whole"
    );
    assert_eq!(
        tail,
        "[truncated: 10 of 11 nodes not shown; give a ROOT, lower --depth or add --kind]"
    );
    let json = spec(&home, &root, &["--json", "tree"]).json();
    assert_eq!(json["nodes"].as_array().unwrap().len(), 1);
    assert_eq!(json["nodes"][0]["title"], title);
    assert_eq!(json["truncated"], true);
}

/// `spec graph` over the cap: cut at a node line (many nodes) or at an
/// edge line (two nodes, many edges); the tail counts both; JSON holds
/// the same nodes and edges.
#[test]
fn a_graph_over_the_cap_is_cut_at_a_node_or_edge_line() {
    let scratch = Scratch::new("bounds-graph");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let (text, _) = big_document(
        "MEC-BIG",
        Some("DOM-GAME"),
        ("RULE-BIG", "rule", "Part"),
        1000,
        "MEC-STAMINA",
        "docs/spec/big.md",
        2,
    );
    write(&root, "docs/spec/big.md", &text);
    let mut echo = "---\nid: MEC-ECHO\nclass: canon\n---\n\n# Echo\n\n".to_owned();
    for n in 0..1500 {
        echo.push_str(&format!("Again R-12, time {n}.\n"));
    }
    write(&root, "docs/spec/echo.md", &echo);
    for (args, cut_in_edges) in [
        (
            &["graph", "MEC-STAMINA", "--impact", "--type", "mentions"][..],
            false,
        ),
        (&["graph", "MEC-ECHO", "--type", "mentions"], true),
    ] {
        let run = spec(&home, &root, args);
        run.code(0);
        let (before, tail) = split_tail(&run.stdout);
        let chars = before.chars().count();
        assert!(chars <= CAP, "{args:?}: {chars}");
        let mut lines: Vec<&str> = before.lines().collect();
        let summary = lines.pop().unwrap();
        let (nodes_total, edges_total) = summary
            .split(';')
            .next()
            .unwrap()
            .strip_prefix("nodes ")
            .and_then(|rest| rest.split_once(", edges "))
            .map(|(n, e)| (n.parse::<usize>().unwrap(), e.parse::<usize>().unwrap()))
            .unwrap_or_else(|| panic!("{summary}"));
        let nodes = lines.iter().filter(|line| !line.contains("--> ")).count();
        let edges = lines.len() - nodes;
        if cut_in_edges {
            assert_eq!(nodes, nodes_total, "{args:?}");
            assert!(edges > 0 && edges < edges_total, "{args:?}: {edges}");
        } else {
            assert!(nodes < nodes_total && edges == 0, "{args:?}");
        }
        assert_eq!(
            tail,
            format!(
                "[truncated: {} of {nodes_total} nodes and {} of {edges_total} edges not shown; lower --depth or add --type]",
                nodes_total - nodes,
                edges_total - edges
            ),
            "{args:?}"
        );
        let mut json_args = vec!["--json"];
        json_args.extend(args);
        let json = spec(&home, &root, &json_args).json();
        assert_eq!(json["truncated"], true, "{args:?}");
        assert_eq!(json["nodes"].as_array().unwrap().len(), nodes, "{args:?}");
        assert_eq!(json["edges"].as_array().unwrap().len(), edges, "{args:?}");
    }
}

/// `show --links` on a node far over the cap: the header, the whole links
/// block, then the text cut at a line end; JSON keeps every link. M: links
/// after the text.
#[test]
fn show_links_on_a_long_node_keeps_its_links_block() {
    for (fixture, id, prefix, line, link, back) in [
        (
            "spec-a",
            "MEC-HUGE",
            "RULE-HUGE",
            "Plain filler words for a very long mechanic text, line",
            "depends_on: [MEC-STAMINA]",
            "docs/records/R/R-77.md",
        ),
        (
            "spec-b",
            "MOD-HUGE",
            "CMD-HUGE",
            "\u{0414}\u{043b}\u{0438}\u{043d}\u{043d}\u{044b}\u{0439} \u{0442}\u{0435}\u{043a}\u{0441}\u{0442} \u{043c}\u{043e}\u{0434}\u{0443}\u{043b}\u{044f}, \u{0441}\u{0442}\u{0440}\u{043e}\u{043a}\u{0430}",
            "derived_from: [\u{0422}\u{0420}\u{0411}-001]",
            "docs/records/REQ/REQ-077.md",
        ),
    ] {
        let scratch = Scratch::new("bounds-links");
        let home = scratch.home("h");
        let root = scratch.copy(fixture, "copy");
        let text = huge(id, prefix, line, 100_000).replacen(
            "class: canon\n",
            &format!("class: canon\nlinks:\n  {link}\n"),
            1,
        );
        write(&root, "docs/spec/huge.md", &text);
        let back_id = back.rsplit('/').next().unwrap().trim_end_matches(".md");
        write(
            &root,
            back,
            format!("---\nid: {back_id}\nclass: canon\n---\n\n# Back\n\nSee {id}.\n"),
        );
        let run = spec(&home, &root, &["show", id, "--links"]);
        run.code(0);
        let lines: Vec<&str> = run.stdout.lines().collect();
        assert!(lines[0].starts_with(&format!("{id} | ")), "{}", lines[0]);
        assert!(
            lines[1].starts_with("  out ") && lines[1].contains(" | docs/spec/huge.md:5"),
            "{fixture}: {}",
            lines[1]
        );
        assert_eq!(
            lines[2],
            format!("  in mentions {back_id} | {back}:8"),
            "{fixture}"
        );
        assert_eq!(lines[3], "  links 1 out, 1 in", "{fixture}");
        let (before, tail) = split_tail(&run.stdout);
        assert!(before.chars().count() <= CAP, "{fixture}");
        let shown = &before[lines[..4].iter().map(|l| l.len() + 1).sum::<usize>()..];
        assert!(
            shown.len() > 1000 && text.starts_with(shown) && shown.ends_with('\n'),
            "{fixture}: the text follows the block, cut at a line end"
        );
        assert!(tail.ends_with("; links not shown: 0]"), "{fixture}: {tail}");
        let json = spec(&home, &root, &["--json", "show", id, "--links"]).json();
        let node = &json["nodes"][0];
        assert_eq!(node["truncated"], true, "{fixture}");
        assert_eq!(node["links"]["outgoing"].as_array().unwrap().len(), 1);
        assert_eq!(node["links"]["incoming"].as_array().unwrap().len(), 1);
        // Nothing of the block cut: `omitted` 0, as the tail's `0`.
        assert_eq!(node["links"]["omitted"], 0, "{fixture}");
        // JSON holds exactly what the text prints: the text cut where the
        // block left it, not at the cap of the texts alone.
        assert_eq!(node["text"], shown, "{fixture}: JSON text vs printed");
    }
}

/// A links block longer than the cap: cut at a link line, no text, the
/// tail counts the links not shown; JSON keeps the first links in the
/// text's order, `truncated: true`.
#[test]
fn a_links_block_over_the_cap_is_cut_at_a_link_line() {
    let scratch = Scratch::new("bounds-links-cut");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let (text, _) = big_document(
        "MEC-BIG",
        Some("DOM-GAME"),
        ("RULE-BIG", "rule", "Part"),
        1000,
        "MEC-STAMINA",
        "docs/spec/big.md",
        2,
    );
    write(&root, "docs/spec/big.md", &text);
    // spec-a: MEC-STAMINA has 5 outgoing and 10 incoming links; 1000 more.
    let total = 5 + 10 + 1000;
    let run = spec(&home, &root, &["show", "MEC-STAMINA", "--links"]);
    run.code(0);
    let (before, tail) = split_tail(&run.stdout);
    assert!(before.chars().count() <= CAP);
    let lines: Vec<&str> = before.lines().collect();
    assert!(lines[0].starts_with("MEC-STAMINA | "), "{}", lines[0]);
    let printed = &lines[1..];
    assert!(
        printed
            .iter()
            .all(|line| line.starts_with("  out ") || line.starts_with("  in ")),
        "only link lines after the header: no text"
    );
    let not_shown = total - printed.len();
    assert!(not_shown > 0, "the block is cut");
    assert!(
        tail.ends_with(&format!("; links not shown: {not_shown}]")),
        "{tail} ({} printed)",
        printed.len()
    );
    let json = spec(&home, &root, &["--json", "show", "MEC-STAMINA", "--links"]).json();
    let node = &json["nodes"][0];
    assert_eq!(node["truncated"], true);
    assert_eq!(node["text"], "");
    // Exactly the printed links, and `omitted` the tail's count.
    let json_links = node["links"]["outgoing"].as_array().unwrap().len()
        + node["links"]["incoming"].as_array().unwrap().len();
    assert_eq!(
        json_links,
        printed.len(),
        "JSON links vs printed link lines"
    );
    assert_eq!(node["links"]["omitted"], not_shown, "omitted vs the tail");
    let names: Vec<String> = node["links"]["outgoing"]
        .as_array()
        .unwrap()
        .iter()
        .chain(node["links"]["incoming"].as_array().unwrap())
        .map(|link| {
            format!(
                "{}:{}",
                link["path"].as_str().unwrap(),
                link["line"].as_u64().unwrap()
            )
        })
        .collect();
    assert!(names.len() >= printed.len(), "{} JSON links", names.len());
    for (line, name) in printed.iter().zip(&names) {
        assert!(line.contains(&format!(" | {name}")), "{line} vs {name}");
    }
}

/// AC-15, several holders: the first holder's links block is cut, the
/// second holder is dropped. The tail's `links not shown: <k>` counts the
/// first's unprinted lines and every link of the dropped one; JSON holds
/// the first holder only, its printed links exactly, `omitted` that `<k>`.
/// M: `omitted` 0 after a cut; the JSON cut by the texts alone.
#[test]
fn a_cut_links_block_counts_the_dropped_holders_links() {
    let scratch = Scratch::new("bounds-links-holders");
    let home = scratch.home("h");
    let root = scratch.copy("spec-a", "copy");
    let (text, _) = big_document(
        "MEC-DUP",
        Some("DOM-GAME"),
        ("RULE-BIG", "rule", "Part"),
        1000,
        "R-12",
        "docs/spec/a-dup.md",
        2,
    );
    write(&root, "docs/spec/a-dup.md", &text);
    write(
        &root,
        "docs/spec/b-dup.md",
        "---\nid: MEC-DUP\nclass: canon\nlinks:\n  depends_on: [MEC-STAMINA, MEC-SPRINT]\n---\n\n# Dup\n\nSee R-12.\n",
    );
    // a-dup: 1000 mentions out; b-dup: two `depends_on` and one mention.
    let total = 1000 + 3;
    let run = spec(&home, &root, &["show", "MEC-DUP", "--links"]);
    run.code(0);
    let (before, tail) = split_tail(&run.stdout);
    assert!(before.chars().count() <= CAP);
    let lines: Vec<&str> = before.lines().collect();
    assert!(
        lines[0].starts_with("MEC-DUP | mechanic | Big | docs/spec/a-dup.md:1 | "),
        "{}",
        lines[0]
    );
    let printed = &lines[1..];
    assert!(
        printed.iter().all(|line| line.starts_with("  out ")),
        "only a-dup's link lines after its header"
    );
    assert!(!before.contains("docs/spec/b-dup.md"), "b-dup is not shown");
    let not_shown = total - printed.len();
    assert!(
        tail.contains("holders not shown: docs/spec/b-dup.md:1;")
            && tail.ends_with(&format!("; links not shown: {not_shown}]")),
        "{tail} ({} printed)",
        printed.len()
    );
    let json = spec(&home, &root, &["--json", "show", "MEC-DUP", "--links"]).json();
    let nodes = json["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 1, "the dropped holder is not in JSON");
    let node = &nodes[0];
    assert_eq!(node["truncated"], true);
    assert_eq!(node["text"], "");
    assert_eq!(
        node["omitted"]["holders"],
        serde_json::json!(["docs/spec/b-dup.md:1"])
    );
    let outgoing = node["links"]["outgoing"].as_array().unwrap();
    assert_eq!(outgoing.len(), printed.len(), "JSON links vs printed");
    assert_eq!(node["links"]["incoming"], serde_json::json!([]));
    assert_eq!(node["links"]["omitted"], not_shown, "omitted vs the tail");
    for (line, link) in printed.iter().zip(outgoing) {
        let place = format!(
            " | {}:{}",
            link["path"].as_str().unwrap(),
            link["line"].as_u64().unwrap()
        );
        assert!(line.contains(&place), "{line} vs {place}");
    }
}
