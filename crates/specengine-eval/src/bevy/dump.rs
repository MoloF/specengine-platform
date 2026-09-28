//! A typed reader of the Bevy schedule dump `app_data.ron`
//! (`bevy_dev_tools::schedule_data`, features `debug` + `schedule_data`).
//!
//! The Bevy 0.19 schema (identical in 0.19.0 and 0.19.1,
//! `bevy_dev_tools/src/schedule_data/serde.rs`):
//!
//! ```text
//! AppData { schedules: [ScheduleData] }
//! ScheduleData { name, systems: [SystemData], system_sets, hierarchy,
//!                dependency, components, conflicts }
//! SystemData { name, apply_deferred, exclusive, deferred }
//! ```
//!
//! Only `schedules[].name` and `schedules[].systems[].{name, apply_deferred}`
//! are needed; every other field is parsed and skipped, and a field outside
//! the schema is counted (the schema is unstable across Bevy minors). There
//! are no observers and no plugins in this schema.
//!
//! The file is tokenised by the RON lexer of `specengine-code` (spike group 2)
//! and parsed into a generic value tree on an explicit stack capped at
//! [`MAX_DEPTH`]: no recursion over the file, no `ron` or `bevy` crate in the
//! workspace graph. An error names the line.

use std::collections::BTreeSet;

use specengine_code::ron::lexer::{Delim, TokenKind, lex};

/// Container nesting accepted in a dump; the 0.19 schema needs 6.
pub const MAX_DEPTH: usize = 64;

const SCHEDULE_FIELDS: &[&str] = &[
    "name",
    "systems",
    "system_sets",
    "hierarchy",
    "dependency",
    "components",
    "conflicts",
];
const SYSTEM_FIELDS: &[&str] = &["name", "apply_deferred", "exclusive", "deferred"];

/// One dumped system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpSystem {
    pub schedule: String,
    /// `type_name` of the system (`Pipe(a, b)` for a piped system).
    pub name: String,
    /// An engine sync point (`ApplyDeferred`), not a user system.
    pub apply_deferred: bool,
}

/// What the dump holds, plus how well it matched the 0.19 schema.
#[derive(Debug, Clone, Default)]
pub struct Dump {
    pub schedules: usize,
    pub systems: Vec<DumpSystem>,
    /// `schedule.field` / `system.field` names outside the 0.19 schema, sorted.
    pub unknown_fields: BTreeSet<String>,
    /// Schema fields absent from at least one record, sorted.
    pub missing_fields: BTreeSet<String>,
}

/// A parse error at a 1-based line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DumpError {
    pub line: usize,
    pub message: String,
}

/// A generic RON value at the 1-based line of its first token (a struct's
/// name, else its opener); only what the reader needs is kept.
#[derive(Debug)]
struct Value {
    line: usize,
    data: Data,
}

#[derive(Debug)]
enum Data {
    Str(String),
    Ident(String),
    /// A struct body `(field: value, …)`, named or not.
    Struct(Vec<(String, Value)>),
    /// A list, a tuple, a map (keys and values in order) or a unit `()`.
    Seq(Vec<Value>),
    /// Numbers and chars.
    Scalar,
}

enum Frame {
    Seq {
        items: Vec<Value>,
        close: Delim,
        line: usize,
    },
    Struct {
        fields: Vec<(String, Value)>,
        key: Option<String>,
        line: usize,
    },
}

/// Parses a dump; `Err` names the line.
pub fn parse(source: &str) -> Result<Dump, DumpError> {
    let lines = LineIndex::new(source);
    let value = parse_value(source, &lines)?;
    read_app_data(value)
}

struct LineIndex {
    starts: Vec<usize>,
}

impl LineIndex {
    fn new(source: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(
            source
                .bytes()
                .enumerate()
                .filter(|(_, b)| *b == b'\n')
                .map(|(i, _)| i + 1),
        );
        Self { starts }
    }

    fn line(&self, offset: usize) -> usize {
        self.starts.partition_point(|start| *start <= offset)
    }
}

fn error(line: usize, message: impl Into<String>) -> DumpError {
    DumpError {
        line,
        message: message.into(),
    }
}

/// The file's one value, built on an explicit stack.
fn parse_value(source: &str, lines: &LineIndex) -> Result<Value, DumpError> {
    let (tokens, errors) = lex(source);
    if let Some(first) = errors.first() {
        return Err(error(
            lines.line(first.offset),
            format!("lexical error: {}", first.kind.as_str()),
        ));
    }
    let tokens: Vec<_> = tokens
        .into_iter()
        .filter(|t| !t.kind.is_comment() && t.kind != TokenKind::Attribute)
        .collect();
    let mut stack: Vec<Frame> = Vec::new();
    let mut root: Option<Value> = None;
    // The line of a skipped `Name` before `(`: the struct's own line.
    let mut name_line: Option<usize> = None;
    let mut i = 0;
    while i < tokens.len() {
        let token = &tokens[i];
        let line = lines.line(token.range.start);
        let text = &source[token.range.clone()];
        // A struct frame waiting for a key takes `key:` or its closer.
        if let Some(Frame::Struct {
            key: key @ None, ..
        }) = stack.last_mut()
        {
            match token.kind {
                TokenKind::Comma => {
                    i += 1;
                    continue;
                }
                TokenKind::Ident
                    if tokens
                        .get(i + 1)
                        .is_some_and(|t| t.kind == TokenKind::Colon) =>
                {
                    *key = Some(text.to_owned());
                    i += 2;
                    continue;
                }
                TokenKind::Close(Delim::Paren) => {}
                _ => return Err(error(line, "expected a field name or `)`")),
            }
        }
        let value = match token.kind {
            TokenKind::Comma | TokenKind::Colon => {
                i += 1;
                continue;
            }
            TokenKind::Open(delim) => {
                if stack.len() >= MAX_DEPTH {
                    return Err(error(line, format!("nesting deeper than {MAX_DEPTH}")));
                }
                let is_struct = delim == Delim::Paren
                    && tokens
                        .get(i + 1)
                        .is_some_and(|t| t.kind == TokenKind::Ident)
                    && tokens
                        .get(i + 2)
                        .is_some_and(|t| t.kind == TokenKind::Colon);
                let line = name_line.take().unwrap_or(line);
                stack.push(if is_struct {
                    Frame::Struct {
                        fields: Vec::new(),
                        key: None,
                        line,
                    }
                } else {
                    Frame::Seq {
                        items: Vec::new(),
                        close: delim,
                        line,
                    }
                });
                i += 1;
                continue;
            }
            TokenKind::Close(delim) => match stack.pop() {
                Some(Frame::Seq {
                    items,
                    close,
                    line: open_line,
                }) if close == delim => Value {
                    line: open_line,
                    data: Data::Seq(items),
                },
                Some(Frame::Struct {
                    fields,
                    key: None,
                    line: open_line,
                }) if delim == Delim::Paren => Value {
                    line: open_line,
                    data: Data::Struct(fields),
                },
                _ => return Err(error(line, "unbalanced delimiter")),
            },
            // `Name(…)`: a struct or tuple name; the opener follows.
            TokenKind::Ident
                if tokens
                    .get(i + 1)
                    .is_some_and(|t| t.kind == TokenKind::Open(Delim::Paren)) =>
            {
                name_line = Some(line);
                i += 1;
                continue;
            }
            TokenKind::Ident => Value {
                line,
                data: Data::Ident(text.to_owned()),
            },
            TokenKind::Str => Value {
                line,
                data: Data::Str(
                    unescape(text).ok_or_else(|| error(line, "invalid string escape"))?,
                ),
            },
            TokenKind::Number | TokenKind::Char => Value {
                line,
                data: Data::Scalar,
            },
            TokenKind::LineComment | TokenKind::BlockComment | TokenKind::Attribute => {
                i += 1;
                continue;
            }
            TokenKind::Unknown => return Err(error(line, "unexpected character")),
        };
        i += 1;
        match stack.last_mut() {
            None if root.is_none() => root = Some(value),
            None => return Err(error(line, "trailing value after the dump")),
            Some(Frame::Seq { items, .. }) => items.push(value),
            Some(Frame::Struct { fields, key, .. }) => match key.take() {
                Some(key) => fields.push((key, value)),
                None => return Err(error(line, "value without a field name")),
            },
        }
    }
    if let Some(frame) = stack.last() {
        let line = match frame {
            Frame::Seq { line, .. } | Frame::Struct { line, .. } => *line,
        };
        return Err(error(line, "unclosed delimiter"));
    }
    root.ok_or_else(|| error(1, "empty dump"))
}

/// The text of a RON string token: `"…"` with escapes, or raw `r#"…"#`.
fn unescape(token: &str) -> Option<String> {
    let body = token.strip_prefix('b').unwrap_or(token);
    if let Some(raw) = body.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        let inner = raw.get(hashes + 1..raw.len().checked_sub(hashes + 1)?)?;
        return Some(inner.to_owned());
    }
    let inner = body.strip_prefix('"')?.strip_suffix('"')?;
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next()? {
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            '0' => out.push('\0'),
            '\\' => out.push('\\'),
            '"' => out.push('"'),
            '\'' => out.push('\''),
            'x' => {
                let hex: String = chars.by_ref().take(2).collect();
                out.push(char::from(u8::from_str_radix(&hex, 16).ok()?));
            }
            'u' => {
                if chars.next()? != '{' {
                    return None;
                }
                let hex: String = chars.by_ref().take_while(|c| *c != '}').collect();
                out.push(char::from_u32(u32::from_str_radix(&hex, 16).ok()?)?);
            }
            // A line continuation: the newline and the next line's indentation vanish.
            '\n' => {
                while chars.clone().next().is_some_and(char::is_whitespace) {
                    chars.next();
                }
            }
            _ => return None,
        }
    }
    Some(out)
}

/// An element of the wrong kind is reported at its own line; a missing or
/// mistyped field at the line of the record that holds it.
fn read_app_data(value: Value) -> Result<Dump, DumpError> {
    let Data::Struct(fields) = value.data else {
        return Err(error(value.line, "the dump is not an `AppData` struct"));
    };
    let app_line = value.line;
    let mut dump = Dump::default();
    let mut schedules = None;
    for (key, value) in fields {
        if key == "schedules" {
            schedules = Some(value);
        } else {
            dump.unknown_fields.insert(format!("app.{key}"));
        }
    }
    let Some(Value {
        data: Data::Seq(schedules),
        ..
    }) = schedules
    else {
        return Err(error(
            app_line,
            "`schedules` list missing: not a Bevy 0.19 schedule dump",
        ));
    };
    for schedule in schedules {
        let Data::Struct(fields) = schedule.data else {
            return Err(error(schedule.line, "a schedule is not a struct"));
        };
        let line = schedule.line;
        dump.schedules += 1;
        note_fields(&fields, "schedule", SCHEDULE_FIELDS, &mut dump);
        let mut name = None;
        let mut systems = None;
        for (key, value) in fields {
            match (key.as_str(), value.data) {
                ("name", Data::Str(text)) => name = Some(text),
                ("systems", Data::Seq(items)) => systems = Some(items),
                ("name" | "systems", _) => {
                    return Err(error(
                        line,
                        format!("schedule field `{key}` has an unexpected type"),
                    ));
                }
                _ => {}
            }
        }
        let (Some(schedule), Some(systems)) = (name, systems) else {
            return Err(error(line, "a schedule lacks `name` or `systems`"));
        };
        for system in systems {
            let Data::Struct(fields) = system.data else {
                return Err(error(system.line, "a system is not a struct"));
            };
            let line = system.line;
            note_fields(&fields, "system", SYSTEM_FIELDS, &mut dump);
            let mut name = None;
            let mut apply_deferred = false;
            for (key, value) in fields {
                match (key.as_str(), value.data) {
                    ("name", Data::Str(text)) => name = Some(text),
                    ("apply_deferred", Data::Ident(flag)) => apply_deferred = flag == "true",
                    _ => {}
                }
            }
            let Some(name) = name else {
                return Err(error(line, "a system lacks a string `name`"));
            };
            dump.systems.push(DumpSystem {
                schedule: schedule.clone(),
                name,
                apply_deferred,
            });
        }
    }
    Ok(dump)
}

fn note_fields(fields: &[(String, Value)], record: &str, schema: &[&str], dump: &mut Dump) {
    for (key, _) in fields {
        if !schema.contains(&key.as_str()) {
            dump.unknown_fields.insert(format!("{record}.{key}"));
        }
    }
    for expected in schema {
        if !fields.iter().any(|(key, _)| key == expected) {
            dump.missing_fields.insert(format!("{record}.{expected}"));
        }
    }
}
