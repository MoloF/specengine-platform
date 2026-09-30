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
