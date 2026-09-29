//! Front-matter YAML through `serde-saphyr` into a generic value tree, under
//! the budgets of the spec: nesting over [`MAX_DEPTH`] (32) and alias
//! expansion over 10 000 nodes are errors, never a stack overflow or a hang.
//!
//! Stack: serde deserialization recurses once per YAML level, and the cost
//! of a level is set by `serde-saphyr`'s frames. In a debug build they are
//! about 20 KiB per sequence level, 22 KiB per mapping value level and
//! 27 KiB per level of a collection used as a key (`deserialize_any`,
//! `deserialize_map`/`_seq`, `next_value_seed`/`_element_seed`/`_key_seed`);
//! a `Spanned<T>` wrapper adds about 17 KiB more per level. So only the
//! levels the reader takes a line or a span from are spanned ([`Root`]).
//! Peak stack of a whole `parse` at the cap of 32 in a debug build: 0.85–
//! 0.91 MiB for nested sequences and mappings, 1.05 MiB when every level is
//! a mapping used as a key, so half of a 2 MiB worker stack stays free
//! whatever the build profile. The depth budget stops deeper input at level
//! 33, so an over-deep block costs no more stack than one at the cap.

use std::borrow::Cow;
use std::fmt;
use std::marker::PhantomData;
use std::ops::Range;

use serde::Deserialize;
use serde::de::{self, Deserializer, MapAccess, SeqAccess, Visitor};
use serde_saphyr::options::DuplicateKeyPolicy;
use serde_saphyr::{MessageFormatter, Spanned, UserMessageFormatter};

/// Deepest nesting of sequences and mappings accepted, counting the root
/// mapping as 1. Sized for a 2 MiB worker stack in a debug build (see the
/// module doc); it is the one source of both depth budgets below.
pub const MAX_DEPTH: usize = 32;

/// Most nodes alias expansion may replay.
pub const MAX_ALIAS_EXPANSION: usize = 10_000;

/// The tree as deserialized: the four levels the reader takes a line or a
/// span from are [`Located`] — the root (1), its keys and values (2), list
/// items and `links` entries (3), `links` list items (4), counted as
/// [`MAX_DEPTH`] counts. Deeper nodes are only kept as values (`extra`,
/// `raised_by`), so they are [`Plain`].
type Root = Located<Located<Located<Located<Plain>>>>;

/// A YAML node with where it was written.
#[derive(Debug, Clone)]
pub(crate) struct YNode {
    pub value: YValue,
    /// Byte range in the YAML text; `None` when unknown or below the
    /// spanned levels of [`Root`].
    pub span: Option<Range<usize>>,
    /// 1-based line in the YAML text; 0 when unknown or below the spanned
    /// levels of [`Root`].
    pub line: usize,
}

#[derive(Debug, Clone)]
pub(crate) enum YValue {
    Null,
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Str(String),
    Seq(Vec<YNode>),
    Map(Vec<(YNode, YNode)>),
}

impl YValue {
    /// Scalar text of a map key or an item; `None` for a collection.
    pub fn scalar_text(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::Null => Some(Cow::Borrowed("null")),
            Self::Bool(value) => Some(Cow::Owned(value.to_string())),
            Self::Int(value) => Some(Cow::Owned(value.to_string())),
            Self::UInt(value) => Some(Cow::Owned(value.to_string())),
            Self::Float(value) => Some(Cow::Owned(value.to_string())),
            Self::Str(value) => Some(Cow::Borrowed(value)),
            Self::Seq(_) | Self::Map(_) => None,
        }
    }

    /// Name of the YAML type, for messages.
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Null => "null",
            Self::Bool(_) => "a boolean",
            Self::Int(_) | Self::UInt(_) => "an integer",
            Self::Float(_) => "a float",
            Self::Str(_) => "a string",
            Self::Seq(_) => "a sequence",
            Self::Map(_) => "a mapping",
        }
    }
}

/// A YAML error at a line of the YAML text (0 when unknown).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct YamlError {
    pub line: usize,
    pub message: String,
}

/// Parses the YAML text of a front-matter block. Never panics on input;
/// budget breaches are errors like any syntax error.
pub(crate) fn parse(text: &str) -> Result<YNode, YamlError> {
    let options = serde_saphyr::options! {
        budget: serde_saphyr::budget! {
            max_depth: MAX_DEPTH,
            flow_nesting_limit: MAX_DEPTH,
        },
        alias_limits: serde_saphyr::alias_limits! {
            max_total_replayed_events: MAX_ALIAS_EXPANSION,
        },
        duplicate_keys: DuplicateKeyPolicy::Error,
        emit_comments: false,
        strict_booleans: true,
        reject_non_finite_typeless_float: false,
        with_snippet: false,
    };
    match serde_saphyr::from_str_with_options::<Root>(text, options) {
        Ok(root) => Ok(root.0),
        Err(error) => {
            let line = error
                .location()
                .map_or(0, |location| usize::try_from(location.line()).unwrap_or(0));
            // The message without "at line X, column Y" (nested alias errors
            // carry them inside the text): the line reported is the file's,
            // not the YAML block's.
            let raw = UserMessageFormatter.format_message(&error);
            let raw = raw.find(" at line ").map_or(&*raw, |cut| &raw[..cut]);
            let mut message = String::with_capacity(raw.len());
            for c in raw.chars() {
                if c.is_control() {
                    message.extend(c.escape_debug());
                } else {
                    message.push(c);
                }
            }
            Err(YamlError { line, message })
        }
    }
}

fn node(spanned: Spanned<YValue>) -> YNode {
    let location = spanned.referenced;
    let line = usize::try_from(location.line()).unwrap_or(0);
    let span = if line == 0 {
        None
    } else {
        let span = location.span();
        match (span.byte_offset(), span.byte_len()) {
            (Some(offset), Some(len)) => {
                let start = usize::try_from(offset).ok();
                let end = usize::try_from(offset.saturating_add(len)).ok();
                start.zip(end).map(|(start, end)| start..end)
            }
            _ => None,
        }
    };
    YNode {
        value: spanned.value,
        span,
        line,
    }
}

/// A node type of one level of the tree; its children are the next level.
trait Level<'de>: Deserialize<'de> {
    fn into_node(self) -> YNode;
}

/// A node with its span and line; children deserialized as `C`.
struct Located<C>(YNode, PhantomData<C>);

impl<'de, C: Level<'de>> Deserialize<'de> for Located<C> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let spanned = Spanned::<Tree<C>>::deserialize(deserializer)?;
        let spanned = Spanned::new(spanned.value.0, spanned.referenced, spanned.defined);
        Ok(Self(node(spanned), PhantomData))
    }
}

impl<'de, C: Level<'de>> Level<'de> for Located<C> {
    fn into_node(self) -> YNode {
        self.0
    }
}

/// A node without a span, and every node under it.
struct Plain(YNode);

impl<'de> Deserialize<'de> for Plain {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Tree::<Self>::deserialize(deserializer)?.0;
        Ok(Self(YNode {
            value,
            span: None,
            line: 0,
        }))
    }
}

impl Level<'_> for Plain {
    fn into_node(self) -> YNode {
        self.0
    }
}

/// A value whose items, keys and values are deserialized as `C`.
struct Tree<C>(YValue, PhantomData<C>);

impl<'de, C: Level<'de>> Deserialize<'de> for Tree<C> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer
            .deserialize_any(TreeVisitor::<C>(PhantomData))
            .map(|value| Self(value, PhantomData))
    }
}

struct TreeVisitor<C>(PhantomData<C>);

impl<'de, C: Level<'de>> Visitor<'de> for TreeVisitor<C> {
    type Value = YValue;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("any YAML value")
    }

    fn visit_unit<E: de::Error>(self) -> Result<YValue, E> {
        Ok(YValue::Null)
    }

    fn visit_none<E: de::Error>(self) -> Result<YValue, E> {
        Ok(YValue::Null)
    }

    fn visit_some<D: Deserializer<'de>>(self, deserializer: D) -> Result<YValue, D::Error> {
        Tree::<C>::deserialize(deserializer).map(|tree| tree.0)
    }

    fn visit_bool<E: de::Error>(self, value: bool) -> Result<YValue, E> {
        Ok(YValue::Bool(value))
    }

    fn visit_i64<E: de::Error>(self, value: i64) -> Result<YValue, E> {
        Ok(YValue::Int(value))
    }

    fn visit_u64<E: de::Error>(self, value: u64) -> Result<YValue, E> {
        Ok(i64::try_from(value).map_or(YValue::UInt(value), YValue::Int))
    }

    fn visit_i128<E: de::Error>(self, value: i128) -> Result<YValue, E> {
        Ok(match (i64::try_from(value), u64::try_from(value)) {
            (Ok(value), _) => YValue::Int(value),
            (_, Ok(value)) => YValue::UInt(value),
            _ => YValue::Str(value.to_string()),
        })
    }

    fn visit_u128<E: de::Error>(self, value: u128) -> Result<YValue, E> {
        Ok(match (i64::try_from(value), u64::try_from(value)) {
            (Ok(value), _) => YValue::Int(value),
            (_, Ok(value)) => YValue::UInt(value),
            _ => YValue::Str(value.to_string()),
        })
    }

    fn visit_f64<E: de::Error>(self, value: f64) -> Result<YValue, E> {
        Ok(YValue::Float(value))
    }

    fn visit_char<E: de::Error>(self, value: char) -> Result<YValue, E> {
        Ok(YValue::Str(value.to_string()))
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<YValue, E> {
        Ok(YValue::Str(value.to_owned()))
    }

    fn visit_string<E: de::Error>(self, value: String) -> Result<YValue, E> {
        Ok(YValue::Str(value))
    }

    fn visit_bytes<E: de::Error>(self, value: &[u8]) -> Result<YValue, E> {
        Ok(YValue::Str(String::from_utf8_lossy(value).into_owned()))
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<YValue, A::Error> {
        let mut items = Vec::new();
        while let Some(item) = seq.next_element::<C>()? {
            items.push(item.into_node());
        }
        Ok(YValue::Seq(items))
    }

    fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<YValue, A::Error> {
        let mut entries = Vec::new();
        while let Some(key) = map.next_key::<C>()? {
            let value = map.next_value::<C>()?;
            entries.push((key.into_node(), value.into_node()));
        }
        Ok(YValue::Map(entries))
    }
}
