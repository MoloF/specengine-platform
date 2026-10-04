//! A small TOML writer for the emitted `specengine.toml`
//! (`docs/canon/import-layout.md` "Emitted configs"): the workspace's
//! `toml` is built without `display`, so the text is written here. Keys are
//! sorted at every level; root tables become `[sections]`, arrays of tables
//! `[[sections]]` (also under a section), every other table is inline.

use toml::{Table, Value};

/// The TOML text of `table`, keys sorted, final LF.
pub(super) fn write(table: &Table) -> String {
    let mut out = String::new();
    values(&mut out, table, true);
    for (key, value) in sorted(table) {
        match value {
            Value::Table(inner) => section(&mut out, &[key], inner),
            Value::Array(items) if is_table_array(items) => {
                for item in items {
                    if let Value::Table(inner) = item {
                        array_section(&mut out, &[key], inner);
                    }
                }
            }
            _ => {}
        }
    }
    out
}

/// A table's entries in key order.
fn sorted(table: &Table) -> Vec<(&str, &Value)> {
    let mut entries: Vec<(&str, &Value)> = table
        .iter()
        .map(|(key, value)| (key.as_str(), value))
        .collect();
    entries.sort_by(|a, b| a.0.cmp(b.0));
    entries
}

/// A non-empty array whose items are all tables.
fn is_table_array(items: &[Value]) -> bool {
    !items.is_empty() && items.iter().all(Value::is_table)
}

/// The `key = value` lines of a table: at the root, tables and arrays of
/// tables are left to sections; inside a section, arrays of tables only.
fn values(out: &mut String, table: &Table, root: bool) {
    for (key, value) in sorted(table) {
        let sectioned = match value {
            Value::Table(_) => root,
            Value::Array(items) => is_table_array(items),
            _ => false,
        };
        if !sectioned {
            out.push_str(&format!("{} = {}\n", bare_or_quoted(key), inline(value)));
        }
    }
}

fn header_path(path: &[&str]) -> String {
    path.iter()
        .map(|key| bare_or_quoted(key))
        .collect::<Vec<_>>()
        .join(".")
}

fn separate(out: &mut String) {
    if !out.is_empty() {
        out.push('\n');
    }
}

fn section(out: &mut String, path: &[&str], table: &Table) {
    separate(out);
    out.push_str(&format!("[{}]\n", header_path(path)));
    values(out, table, false);
    nested_arrays(out, path, table);
}

fn array_section(out: &mut String, path: &[&str], table: &Table) {
    separate(out);
    out.push_str(&format!("[[{}]]\n", header_path(path)));
    values(out, table, false);
    nested_arrays(out, path, table);
}

/// The arrays of tables directly under a section, as `[[path.key]]`.
fn nested_arrays(out: &mut String, path: &[&str], table: &Table) {
    for (key, value) in sorted(table) {
        if let Value::Array(items) = value
            && is_table_array(items)
        {
            let mut nested = path.to_vec();
            nested.push(key);
            for item in items {
                if let Value::Table(inner) = item {
                    array_section(out, &nested, inner);
                }
            }
        }
    }
}

/// A value written inline.
fn inline(value: &Value) -> String {
    match value {
        Value::String(text) => basic_string(text),
        Value::Integer(number) => number.to_string(),
        Value::Float(number) => float(*number),
        Value::Boolean(flag) => flag.to_string(),
        Value::Datetime(datetime) => datetime.to_string(),
        Value::Array(items) => {
            let items: Vec<String> = items.iter().map(inline).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Table(table) if table.is_empty() => "{}".to_owned(),
        Value::Table(table) => {
            let entries: Vec<String> = sorted(table)
                .into_iter()
                .map(|(key, value)| format!("{} = {}", bare_or_quoted(key), inline(value)))
                .collect();
            format!("{{ {} }}", entries.join(", "))
        }
    }
}

fn float(number: f64) -> String {
    if number.is_nan() {
        "nan".to_owned()
    } else if number.is_infinite() {
        if number > 0.0 { "inf" } else { "-inf" }.to_owned()
    } else {
        // Debug keeps a fraction or an exponent, as a TOML float needs.
        format!("{number:?}")
    }
}

/// A key: bare when it is `[A-Za-z0-9_-]+`, else a basic string.
fn bare_or_quoted(key: &str) -> String {
    if !key.is_empty()
        && key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        key.to_owned()
    } else {
        basic_string(key)
    }
}

/// A TOML basic string holding `text` exactly.
fn basic_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            '\u{8}' => out.push_str("\\b"),
            '\u{C}' => out.push_str("\\f"),
            c if u32::from(c) < 0x20 || u32::from(c) == 0x7F => {
                out.push_str(&format!("\\u{:04X}", u32::from(c)));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
