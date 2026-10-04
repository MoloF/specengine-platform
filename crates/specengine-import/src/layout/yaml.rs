//! A small YAML writer for the front-matter the layout emits
//! (`docs/canon/import-layout.md` "YAML"): double-quoted strings,
//! plain keys where YAML reads them back unchanged, flow sequences. The
//! workspace's `toml` and `serde` carry no YAML writer, and none is added.

/// A double-quoted YAML scalar holding `text` exactly: `\` and `"` escaped,
/// every character YAML would not read back as itself (controls, DEL, C1,
/// line and paragraph separators, BOM, non-characters) as an escape.
pub(super) fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\0' => out.push_str("\\0"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            c if u32::from(c) < 0x20 || (0x7F..=0x9F).contains(&u32::from(c)) => {
                out.push_str(&format!("\\x{:02X}", u32::from(c)));
            }
            '\u{2028}' | '\u{2029}' | '\u{FEFF}' | '\u{FFFE}' | '\u{FFFF}' => {
                out.push_str(&format!("\\u{:04X}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// The words YAML reads as null or as a boolean, any case (YAML 1.2's
/// core schema and the 1.1 booleans core's reader still accepts).
const NOT_STRINGS: [&str; 10] = [
    "null", "true", "false", "yes", "no", "on", "off", "y", "n", "~",
];

/// A null- or bool-like word, any case.
fn null_or_bool_like(text: &str) -> bool {
    NOT_STRINGS
        .iter()
        .any(|word| text.eq_ignore_ascii_case(word))
}

/// Whether a key is written plain: `^[A-Za-z_][A-Za-z0-9_-]*$` and not
/// null- or bool-like (`docs/features/import-layout.md` AC-05).
pub(super) fn is_plain_key(text: &str) -> bool {
    let mut chars = text.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        && !null_or_bool_like(text)
}

/// Whether a plain scalar as written surely reads back as the same string:
/// not empty, not null- or bool-like, not opening like a number (a sign,
/// a digit, a dot before a digit or `inf`/`nan`). Conservative: a `false`
/// only costs quotes.
pub(super) fn reads_as_string(plain: &str) -> bool {
    let unsigned = plain.strip_prefix(['+', '-']).unwrap_or(plain);
    let after_dot = unsigned.strip_prefix('.').unwrap_or(unsigned);
    let numeric = after_dot.starts_with(|c: char| c.is_ascii_digit())
        || (unsigned.starts_with('.')
            && ["inf", "nan"].iter().any(|word| {
                after_dot
                    .get(..word.len())
                    .is_some_and(|head| head.eq_ignore_ascii_case(word))
            }));
    !plain.is_empty() && !null_or_bool_like(plain) && !numeric
}

/// A mapping key: plain when [`is_plain_key`], else double-quoted.
pub(super) fn key(text: &str) -> String {
    if is_plain_key(text) {
        text.to_owned()
    } else {
        quoted(text)
    }
}

/// An `id` or `class` value: plain when YAML reads it back as the same
/// string (a letter or `_`, then letters, digits, `_`, `-`, `.`; no word
/// YAML reads as a boolean or null), else double-quoted.
pub(super) fn plain_or_quoted(text: &str) -> String {
    let mut chars = text.chars();
    let shaped = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'));
    if shaped && reads_as_string(text) {
        text.to_owned()
    } else {
        quoted(text)
    }
}

/// A flow sequence of double-quoted strings.
pub(super) fn sequence(items: &[String]) -> String {
    let items: Vec<String> = items.iter().map(|item| quoted(item)).collect();
    format!("[{}]", items.join(", "))
}

/// One `key: value` line.
pub(super) fn entry(name: &str, value: &str) -> String {
    format!("{}: {value}", key(name))
}
