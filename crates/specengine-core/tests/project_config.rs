//! AC-19 of docs/features/spec-cli.md (core): `ProjectConfig::from_toml`,
//! the one loader of `specengine.toml` for the `spec` commands. A closed
//! `[project]` (`slug`, `name`, `language`), `[ids]` and `[paths]` through
//! their own readers with lines kept, the known top-level keys only
//! name-checked, and every error at its line.

mod common;

use common::fixture;
use specengine_core::{
    IdSchemeToml, MAX_SLUG_BYTES, Paths, Project, ProjectConfig, project_from_toml, slug_problem,
};
use specengine_model::IdScheme;

fn error_line(text: &str) -> (Option<usize>, String) {
    let error = ProjectConfig::from_toml(text).expect_err(text);
    (error.line, error.message)
}

#[test]
fn both_fixture_configs_load_with_their_slugs() {
    for (name, slug, project_name, paths_written, line) in [
        ("spec-a", "lantern-keep", Some("lantern-keep"), false, 5),
        ("spec-b", "zerkalo", None, true, 22),
    ] {
        let text = std::fs::read_to_string(fixture(name).join("specengine.toml")).unwrap();
        let config = ProjectConfig::from_toml(&text).unwrap_or_else(|e| panic!("{}", e.at(name)));
        assert_eq!(config.slug(), Ok(slug), "{name}");
        assert_eq!(config.project.slug.as_deref(), Some(slug));
        assert_eq!(config.project.name.as_deref(), project_name, "{name}");
        assert_eq!(config.project.language, None);
        assert_eq!(config.paths_written, paths_written, "{name}");
        assert_eq!(config.project_line, Some(line), "{name}");
        // [ids] and [paths]: exactly what their own readers give.
        assert_eq!(config.scheme, IdScheme::from_toml(&text).unwrap(), "{name}");
        assert_eq!(config.paths, Paths::from_toml(&text).unwrap(), "{name}");
        assert_eq!(project_from_toml(&text), Ok(config));
    }
}

#[test]
fn the_data_example_loads() {
    let text = "[project]\nslug = \"lantern-keep\"   # comment\nname = \"Lantern Keep\"\nlanguage = \"en\"\n";
    let config = ProjectConfig::from_toml(text).unwrap();
    assert_eq!(
        config.project,
        Project {
            slug: Some("lantern-keep".into()),
            name: Some("Lantern Keep".into()),
            language: Some("en".into()),
            profile: None,
        }
    );
    assert_eq!(config.project_line, Some(1));
    assert!(!config.paths_written);
    assert!(
        config.scheme.prefixes().is_empty(),
        "no [ids]: the empty scheme"
    );
    assert_eq!(config.paths, Paths::default());
}

#[test]
fn project_is_closed_and_typed_errors_are_at_their_line() {
    for (text, line) in [
        ("[project]\nslug = \"ok\"\nowner = \"x\"\n", 3),
        ("\n\n[project]\nslugs = \"ok\"\n", 4),
        ("[project]\nname = 7\n", 2),
        ("[project]\nlanguage = [\"en\"]\n", 2),
        ("[project]\nslug = 3\n", 2),
        ("[project]\nslug = true\n", 2),
        ("project = \"flat\"\n", 1),
    ] {
        let (got, message) = error_line(text);
        assert_eq!(got, Some(line), "{text:?}: {message}");
        assert!(!message.is_empty());
    }
}

#[test]
fn a_bad_slug_is_an_error_at_the_slug_line() {
    let long = "a".repeat(MAX_SLUG_BYTES + 1);
    for bad in [
        "Lantern",
        "1x",
        "a/b",
        "../x",
        "",
        "a_b",
        "-a",
        "a b",
        "\u{0430}",
        long.as_str(),
    ] {
        let text = format!("# c\n[project]\nname = \"n\"\nslug = \"{bad}\"\n");
        let (line, message) = error_line(&text);
        assert_eq!(line, Some(4), "{bad:?}: {message}");
        assert!(message.contains("slug"), "{bad:?}: {message}");
        assert!(slug_problem(bad).is_some(), "{bad:?}");
    }
    let edge = format!("a{}", "-9".repeat((MAX_SLUG_BYTES - 1) / 2));
    assert!(edge.len() <= MAX_SLUG_BYTES);
    for good in [
        "a",
        "x-1",
        "my-project-2",
        "lantern-keep",
        edge.as_str(),
        &"z".repeat(MAX_SLUG_BYTES),
    ] {
        assert_eq!(slug_problem(good), None, "{good:?}");
        let config = ProjectConfig::from_toml(&format!("[project]\nslug = \"{good}\"\n")).unwrap();
        assert_eq!(config.slug(), Ok(good));
    }
    assert_eq!(MAX_SLUG_BYTES, 64);
}

/// A missing slug is no load error; `slug()` gives it at the `[project]`
/// line, else line 1.
#[test]
fn a_missing_slug_is_an_error_of_slug_only() {
    let config = ProjectConfig::from_toml("# x\n\n[project]\nname = \"n\"\n").unwrap();
    assert_eq!(config.project_line, Some(3));
    let error = config.slug().expect_err("no slug");
    assert_eq!(error.line, Some(3));
    assert!(
        error
            .at("specengine.toml")
            .starts_with("specengine.toml:3: ")
    );
    let config =
        ProjectConfig::from_toml("[ids]\nR = { kind = \"requirement\", width = 2 }\n").unwrap();
    assert_eq!(config.project_line, None);
    assert_eq!(config.slug().expect_err("no table").line, Some(1));
    let config = ProjectConfig::from_toml("").unwrap();
    assert_eq!(config.slug().expect_err("empty file").line, Some(1));
}

#[test]
fn top_level_keys_are_closed_the_known_ones_only_name_checked() {
    let known = "[project]\nslug = \"ok\"\n[budgets]\nx = 1\n[classes]\nwhatever = [1, \"a\"]\n\
                 [check]\n[generators]\nk = \"v\"\n[zones]\n[gate]\n[code]\n";
    ProjectConfig::from_toml(known).unwrap();
    for (text, line) in [
        (
            "[project]\nslug = \"ok\"\n\n[path]\nroots = [\"docs\"]\n",
            4,
        ),
        ("[projects]\nslug = \"ok\"\n", 1),
        ("[project]\nslug = \"ok\"\n\nextra = 1\n", 4),
        ("unknown = 1\n", 1),
    ] {
        let (got, message) = error_line(text);
        assert_eq!(got, Some(line), "{text:?}: {message}");
    }
}

#[test]
fn ids_and_paths_errors_keep_their_lines() {
    // A lower-case prefix in [ids], at its line.
    let text = "[project]\nslug = \"ok\"\n\n[ids]\nR = { kind = \"requirement\", width = 2 }\nlow = { kind = \"x\", width = 2 }\n";
    let from_ids = IdScheme::from_toml(text).expect_err("bad prefix");
    let (line, _) = error_line(text);
    assert_eq!(line, from_ids.line, "the [ids] reader's line");
    assert_eq!(line, Some(6));
    // An absolute [paths] root, at its line.
    let text = "[project]\nslug = \"ok\"\n\n[paths]\nroots = [\"/abs\"]\n";
    let from_paths = Paths::from_toml(text).expect_err("absolute root");
    let (line, _) = error_line(text);
    assert_eq!(line, from_paths.line, "the [paths] reader's line");
    assert_eq!(line, Some(5));
    // `at(file)`: `file:line: message`.
    let error = ProjectConfig::from_toml(text).unwrap_err();
    assert_eq!(
        error.at("cfg.toml"),
        format!("cfg.toml:5: {}", error.message)
    );
}

#[test]
fn a_toml_syntax_error_is_at_its_line() {
    let (line, message) = error_line("[project]\nslug = \"ok\"\nname = \n");
    assert_eq!(line, Some(3), "{message}");
}
