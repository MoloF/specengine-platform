//! Framework roles of Rust items (slice 10a-1,
//! `docs/canon/code-roles.md` "Role facts"): which items a framework gives
//! a role, as data, and the role facts of one parsed file.
//!
//! - **Data**: [`KNOWN`] names the frameworks `[code] frameworks` may
//!   list; each [`Framework`] its role [`Word`]s (each with the item kinds
//!   a subject of the word may name), its sources and its categories. Core,
//!   store and CLI hold none of these words: they store, compare and count
//!   the strings this module gives (ADR-0008). A second framework, or a
//!   version gate of one, is a new entry here, never a key or a branch
//!   elsewhere.
//! - **Facts** ([`role_facts`]): each listed framework's detector on the
//!   file's one tree (Bevy: [`crate::bevy::detect`]), its findings as
//!   [`RoleFact`]s by first byte. A fact sits on an item ([`RoleFact::symbol_ord`],
//!   by byte range over the file's items), names a subject (a path as
//!   written, [`crate::bevy::Subject`]) for a read to link, or is
//!   `uncertain` with a category. Text as written, never resolved (layer
//!   C); a role never binds.

use std::collections::BTreeMap;
use std::ops::Range;

use tree_sitter::Tree;

use crate::bevy::{
    self, ADD_OBSERVER, ADD_PLUGINS, ADD_SYSTEMS, BevyAnalysis, DeriveRole, Form, ImplTrait,
    Origin, PluginKind, Subject, Target, UncertainCategory,
};
use crate::items::ItemRecord;

/// A framework's role vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Framework {
    /// Its name in `[code] frameworks`.
    pub name: &'static str,
    /// Its role words, byte order.
    pub words: &'static [Word],
    /// Where a role is read: [`RoleFact::source`].
    pub sources: &'static [&'static str],
    /// Why a fact is [`UNCERTAIN`]: [`RoleFact::category`].
    pub categories: &'static [&'static str],
}

/// One role word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Word {
    pub word: &'static str,
    /// The `item_kind`s (`crate::ItemRecord::kind`) of the symbols a
    /// subject of this word may name.
    pub item_kinds: &'static [&'static str],
}

/// A fact read exactly ([`RoleFact::certainty`]).
pub const CERTAIN: &str = "certain";
/// A fact the detector saw and could not read, or read under a condition
/// ([`RoleFact::category`] says which).
pub const UNCERTAIN: &str = "uncertain";

/// The [`RoleFact::form`] of a subject written as a path: the plain one,
/// which a read's text leaves out
/// (`docs/canon/code-roles.md` "Role facts").
pub const PATH_FORM: &str = Form::Path.as_str();
/// The [`RoleFact::origin`] of ordinary code: the plain one, which a
/// read's text leaves out.
pub const CODE_ORIGIN: &str = Origin::Code.as_str();

// Bevy 0.19's words.
const COMPONENT: &str = "component";
const ENTITY_EVENT: &str = "entity_event";
const EVENT: &str = "event";
const MESSAGE: &str = "message";
const OBSERVER: &str = "observer";
const PLUGIN: &str = "plugin";
const REFLECT: &str = "reflect";
const RESOURCE: &str = "resource";
const SYSTEM: &str = "system";

// Bevy's sources.
/// `#[derive(..)]` on a struct, enum or union.
const DERIVE: &str = "derive";
/// `impl Component for T`, `impl Plugin for T`.
const IMPL: &str = "impl";
/// A function whose only parameter is `&mut App`.
const FN: &str = "fn";
/// A registration call: `add_systems`, `add_observer`, `add_plugins`,
/// `add_message`.
const CALL: &str = "call";

/// A derive inside `#[cfg_attr(<p>, derive(..))]`.
const CFG_ATTR: &str = "cfg_attr";

/// Item kinds a function role names.
const FUNCTION_KINDS: &[&str] = &["function_item"];
/// Item kinds a plugin names: a function or a type.
const PLUGIN_KINDS: &[&str] = &["function_item", "struct_item", "enum_item"];
/// Item kinds a type role names.
const TYPE_KINDS: &[&str] = &["struct_item", "enum_item", "union_item", "type_item"];

/// Bevy 0.19 (`crate::bevy`).
pub const BEVY: Framework = Framework {
    name: "bevy",
    words: &[
        Word {
            word: COMPONENT,
            item_kinds: TYPE_KINDS,
        },
        Word {
            word: ENTITY_EVENT,
            item_kinds: TYPE_KINDS,
        },
        Word {
            word: EVENT,
            item_kinds: TYPE_KINDS,
        },
        Word {
            word: MESSAGE,
            item_kinds: TYPE_KINDS,
        },
        Word {
            word: OBSERVER,
            item_kinds: FUNCTION_KINDS,
        },
        Word {
            word: PLUGIN,
            item_kinds: PLUGIN_KINDS,
        },
        Word {
            word: REFLECT,
            item_kinds: TYPE_KINDS,
        },
        Word {
            word: RESOURCE,
            item_kinds: TYPE_KINDS,
        },
        Word {
            word: SYSTEM,
            item_kinds: FUNCTION_KINDS,
        },
    ],
    sources: &[DERIVE, IMPL, FN, CALL],
    categories: &[
        CFG_ATTR,
        UncertainCategory::MacroInArguments.as_str(),
        UncertainCategory::Metavariable.as_str(),
        UncertainCategory::UnknownMethod.as_str(),
        UncertainCategory::Expression.as_str(),
        UncertainCategory::Arguments.as_str(),
        UncertainCategory::NestingTooDeep.as_str(),
        UncertainCategory::ParseError.as_str(),
    ],
};

/// Every framework, by name.
pub const FRAMEWORKS: &[Framework] = &[BEVY];

/// The names `[code] frameworks` may list, byte order.
pub const KNOWN: &[&str] = &[BEVY.name];

/// The framework named `name`.
#[must_use]
pub fn framework(name: &str) -> Option<&'static Framework> {
    FRAMEWORKS.iter().find(|framework| framework.name == name)
}

/// Every word of the listed frameworks (unknown names skipped), byte
/// order, each once.
#[must_use]
pub fn words(frameworks: &[String]) -> Vec<&'static str> {
    item_kinds(frameworks).into_keys().collect()
}

/// Each word of the listed frameworks with the item kinds its subject may
/// name (every listed framework's, in data order, each once).
#[must_use]
pub fn item_kinds(frameworks: &[String]) -> BTreeMap<&'static str, Vec<&'static str>> {
    let mut kinds: BTreeMap<&'static str, Vec<&'static str>> = BTreeMap::new();
    for name in frameworks {
        let Some(framework) = framework(name) else {
            continue;
        };
        for word in framework.words {
            let entry = kinds.entry(word.word).or_default();
            for kind in word.item_kinds {
                if !entry.contains(kind) {
                    entry.push(kind);
                }
            }
        }
    }
    kinds
}

/// One role fact of a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleFact {
    /// A [`Word::word`].
    pub word: &'static str,
    /// One of [`Framework::sources`].
    pub source: &'static str,
    /// How the subject or the registered leaf is written (`path`,
    /// `closure`, `factory`); `None` for a derive, an impl, a function and
    /// an uncertain fact.
    pub form: Option<&'static str>,
    /// Where it was read (`code`, `macro_rules`, `macro_call`).
    pub origin: &'static str,
    /// The group a leaf is registered in (Bevy: its schedule label),
    /// whitespace dropped, capped.
    pub group: Option<String>,
    /// The item it sits on: its index among the file's items.
    pub symbol_ord: Option<usize>,
    /// What it names, for a read to link ([`Subject::text`]).
    pub subject: Option<String>,
    /// The subject's last segment ([`Subject::name`]).
    pub subject_name: Option<String>,
    /// [`CERTAIN`] or [`UNCERTAIN`]; a fact with a subject is [`CERTAIN`].
    pub certainty: &'static str,
    /// Why it is [`UNCERTAIN`]: one of [`Framework::categories`].
    pub category: Option<&'static str>,
    /// 1-based.
    pub line: usize,
    /// The first byte it was read at: its place in source order (never
    /// stored).
    pub start: usize,
}

/// The role facts of one parsed file under the listed frameworks (unknown
/// names skipped, repeats read once), `items` the file's items
/// ([`crate::bind_tree`]'s): in source order (by [`RoleFact::start`], a
/// `pipe` argument after its leaf), facts read at one byte as each
/// framework reads them, frameworks in the order listed. A pure function
/// of the tree, the text and the list.
#[must_use]
pub fn role_facts(
    frameworks: &[String],
    tree: &Tree,
    source: &str,
    items: &[ItemRecord],
) -> Vec<RoleFact> {
    let places = Places::new(items);
    let mut facts = Vec::new();
    let mut read: Vec<&str> = Vec::new();
    for name in frameworks {
        if read.contains(&name.as_str()) {
            continue;
        }
        read.push(name);
        if name == BEVY.name {
            bevy_facts(&bevy::detect(tree, source), &places, &mut facts);
        }
    }
    facts.sort_by_key(|fact| fact.start);
    facts
}

/// Where a fact sits among the file's items.
struct Places {
    /// Each item's byte range → its index.
    items: BTreeMap<(usize, usize), usize>,
    /// Each function item's byte range and index.
    functions: Vec<(Range<usize>, usize)>,
}

impl Places {
    fn new(items: &[ItemRecord]) -> Self {
        let mut by_range = BTreeMap::new();
        let mut functions = Vec::new();
        for (at, item) in items.iter().enumerate() {
            by_range
                .entry((item.byte_range.start, item.byte_range.end))
                .or_insert(at);
            if FUNCTION_KINDS.contains(&item.kind.as_str()) {
                functions.push((item.byte_range.clone(), at));
            }
        }
        Self {
            items: by_range,
            functions,
        }
    }

    /// The item whose node spans `range` exactly.
    fn item(&self, range: &Range<usize>) -> Option<usize> {
        self.items.get(&(range.start, range.end)).copied()
    }

    /// The innermost function item holding the byte `at`.
    fn enclosing_function(&self, at: usize) -> Option<usize> {
        self.functions
            .iter()
            .filter(|(range, _)| range.start <= at && at < range.end)
            .max_by_key(|(range, _)| range.start)
            .map(|(_, ord)| *ord)
    }
}

/// A fact with nothing filled but its word, source, origin and place
/// (line, first byte): [`CERTAIN`] on nothing.
fn bare(
    word: &'static str,
    source: &'static str,
    origin: Origin,
    (line, start): (usize, usize),
) -> RoleFact {
    RoleFact {
        word,
        source,
        form: None,
        origin: origin.as_str(),
        group: None,
        symbol_ord: None,
        subject: None,
        subject_name: None,
        certainty: CERTAIN,
        category: None,
        line,
        start,
    }
}

/// `fact` naming `subject` as written in `form`.
fn naming(fact: RoleFact, form: Option<Form>, subject: &Subject) -> RoleFact {
    RoleFact {
        form: form.map(Form::as_str),
        subject: Some(subject.text.clone()),
        subject_name: Some(subject.name.clone()),
        ..fact
    }
}

/// `fact` made [`UNCERTAIN`] for `category`.
fn uncertain(fact: RoleFact, category: &'static str) -> RoleFact {
    RoleFact {
        certainty: UNCERTAIN,
        category: Some(category),
        ..fact
    }
}

/// `fact` naming the subject read, or [`UNCERTAIN`] with why it was not.
fn read_subject(
    fact: RoleFact,
    form: Option<Form>,
    read: &Result<Subject, UncertainCategory>,
) -> RoleFact {
    match read {
        Ok(subject) => naming(fact, form, subject),
        Err(category) => uncertain(fact, category.as_str()),
    }
}

/// A registered leaf: a closure sits on its innermost enclosing function
/// item; a path or a factory names its subject; anything else is
/// [`UNCERTAIN`].
fn leaf(
    fact: RoleFact,
    read: Result<(Form, Option<&Subject>), UncertainCategory>,
    places: &Places,
) -> RoleFact {
    match read {
        Ok((Form::Closure, _)) => RoleFact {
            form: Some(Form::Closure.as_str()),
            symbol_ord: places.enclosing_function(fact.start),
            ..fact
        },
        Ok((form, Some(subject))) => naming(fact, Some(form), subject),
        Ok((_, None)) => uncertain(fact, UncertainCategory::Expression.as_str()),
        Err(category) => uncertain(fact, category.as_str()),
    }
}

/// Bevy's facts of one analysis.
fn bevy_facts(analysis: &BevyAnalysis, places: &Places, out: &mut Vec<RoleFact>) {
    for derive in &analysis.derives {
        let word = match derive.role {
            DeriveRole::Component => COMPONENT,
            DeriveRole::Resource => RESOURCE,
            DeriveRole::Message => MESSAGE,
            DeriveRole::Event => EVENT,
            DeriveRole::EntityEvent => ENTITY_EVENT,
            DeriveRole::Reflect => REFLECT,
        };
        let fact = RoleFact {
            symbol_ord: places.item(&derive.item),
            ..bare(word, DERIVE, Origin::Code, (derive.line, derive.start))
        };
        out.push(if derive.parse_error {
            uncertain(fact, UncertainCategory::ParseError.as_str())
        } else if derive.cfg_attr {
            uncertain(fact, CFG_ATTR)
        } else {
            fact
        });
    }
    for implemented in &analysis.impls {
        let word = match implemented.implemented {
            ImplTrait::Component => COMPONENT,
            ImplTrait::Plugin => PLUGIN,
        };
        let at = (implemented.line, implemented.start);
        let fact = bare(word, IMPL, implemented.origin, at);
        out.push(read_subject(fact, None, &implemented.subject));
    }
    for plugin in &analysis.plugins {
        if plugin.kind != PluginKind::FnApp {
            continue;
        }
        out.push(RoleFact {
            symbol_ord: plugin.item.as_ref().and_then(|range| places.item(range)),
            ..bare(PLUGIN, FN, plugin.origin, (plugin.line, plugin.start))
        });
    }
    for registration in &analysis.registrations {
        let word = match registration.target {
            Target::System => SYSTEM,
            Target::Observer => OBSERVER,
        };
        let at = (registration.line, registration.start);
        let fact = RoleFact {
            group: registration.schedule.clone(),
            ..bare(word, CALL, registration.origin, at)
        };
        let read = Ok((registration.form, registration.subject.as_ref()));
        out.push(leaf(fact, read, places));
        // Each piped system is a system of the registration's group.
        for piped in &registration.pipes {
            let fact = RoleFact {
                group: registration.schedule.clone(),
                ..bare(SYSTEM, CALL, registration.origin, (piped.line, piped.start))
            };
            let read = piped
                .read
                .as_ref()
                .map(|(form, subject)| (*form, subject.as_ref()))
                .map_err(|category| *category);
            out.push(leaf(fact, read, places));
        }
    }
    for using in &analysis.plugin_uses {
        let fact = bare(PLUGIN, CALL, using.origin, (using.line, using.start));
        out.push(match &using.subject {
            Some(subject) => naming(fact, using.form, subject),
            None => uncertain(fact, UncertainCategory::Expression.as_str()),
        });
    }
    for message in &analysis.messages {
        let fact = bare(MESSAGE, CALL, message.origin, (message.line, message.start));
        out.push(read_subject(fact, Some(Form::Path), &message.subject));
    }
    for unread in &analysis.uncertain {
        let word = match unread.call {
            ADD_SYSTEMS => SYSTEM,
            ADD_OBSERVER => OBSERVER,
            ADD_PLUGINS => PLUGIN,
            // A method this module gives no word.
            _ => continue,
        };
        out.push(uncertain(
            bare(word, CALL, unread.origin, (unread.line, unread.start)),
            unread.category.as_str(),
        ));
    }
}
