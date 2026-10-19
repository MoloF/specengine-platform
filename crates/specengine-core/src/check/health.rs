//! Row 5's candidates (task spec `health-code`, "`[[health.bound]]`",
//! "Candidates"): the nodes each [`BoundRule`] selects. A candidate is a
//! live node ([`is_live`]) with an ID in a file whose front-matter reads
//! ([`front_matter_failure`]): a document by its own fields (its kind: its
//! `kind:`, else its ID's prefix kind), a `{#ID}` section by its ID's
//! prefix kind and its document's class, path and front-matter. Selectors
//! AND, the items of a list OR; `when` reads the front-matter as written,
//! as `[[check.rules]]` reads it (a key written as a string equal to one
//! of its strings; absent or no string: not selected).
//!
//! Pure: it reads the given files and names no kind, class, path or key
//! of any project. What it selects is no finding.

use specengine_model::Node;

use super::config::DocClass;
use super::engine::written_values;
use super::input::CheckFile;
use super::render::is_live;
use super::text::{FileText, Written, front_matter_failure};
use crate::glob::Glob;
use crate::health_toml::BoundRule;

/// One selected node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundCandidate {
    /// Its file: an index into the files given.
    pub file: usize,
    /// Its position in that file's parse (0: the document).
    pub ord: usize,
    /// The rules selecting it, 0-based, in config order; never empty.
    pub rules: Vec<usize>,
}

/// Every node of `files` some rule of `rules` selects, by file (the order
/// given), then position.
pub fn bound_candidates(files: &[&CheckFile], rules: &[BoundRule]) -> Vec<BoundCandidate> {
    let globs: Vec<Vec<Glob>> = rules
        .iter()
        .map(|rule| rule.paths.iter().map(|glob| Glob::new(glob)).collect())
        .collect();
    let mut out = Vec::new();
    if rules.is_empty() {
        return out;
    }
    for (index, file) in files.iter().enumerate() {
        let Some(parsed) = &file.parsed else {
            continue;
        };
        if front_matter_failure(parsed).is_some() || !is_live(parsed) {
            continue;
        }
        let Some(document) = parsed.document() else {
            continue;
        };
        let mut judged = Judged::new(file, document);
        for (ord, node) in parsed.nodes.iter().enumerate() {
            if node.id.is_none() {
                continue;
            }
            let selecting: Vec<usize> = rules
                .iter()
                .zip(&globs)
                .enumerate()
                .filter(|(_, (rule, globs))| judged.selects(node, rule, globs))
                .map(|(at, _)| at)
                .collect();
            if !selecting.is_empty() {
                out.push(BoundCandidate {
                    file: index,
                    ord,
                    rules: selecting,
                });
            }
        }
    }
    out
}

/// One file's document as the selectors read it; its front-matter values
/// read once, when first needed.
struct Judged<'a> {
    file: &'a CheckFile,
    document: &'a Node,
    class: Option<DocClass>,
    values: Option<Vec<(String, Written)>>,
}

impl<'a> Judged<'a> {
    fn new(file: &'a CheckFile, document: &'a Node) -> Self {
        let class = document
            .fields
            .as_ref()
            .and_then(|fields| fields.class.as_deref())
            .and_then(DocClass::parse);
        Self {
            file,
            document,
            class,
            values: None,
        }
    }

    /// `rule` selects `node` of this file.
    fn selects(&mut self, node: &Node, rule: &BoundRule, globs: &[Glob]) -> bool {
        if !rule.kinds.is_empty()
            && !node
                .kind
                .as_ref()
                .is_some_and(|kind| rule.kinds.contains(kind))
        {
            return false;
        }
        if !rule.classes.is_empty()
            && !self
                .class
                .is_some_and(|class| rule.classes.contains(&class))
        {
            return false;
        }
        if !globs.is_empty() && !globs.iter().any(|glob| glob.matches(&self.file.path)) {
            return false;
        }
        if rule.when.is_empty() {
            return true;
        }
        let (file, document) = (self.file, self.document);
        let values = self
            .values
            .get_or_insert_with(|| written_values(&FileText::new(&file.bytes), document));
        rule.when.iter().all(|(key, allowed)| {
            let value = values
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value);
            matches!(value, Some(Written::Str(text)) if allowed.contains(text))
        })
    }
}
