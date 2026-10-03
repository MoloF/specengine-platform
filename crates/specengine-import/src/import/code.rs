//! The code scan (`docs/features/import-records.md` AC-09): files under the
//! `[code]` roots are read as bytes, never built, and searched for each
//! document path and each form of it minus a `strip` prefix; an occurrence
//! counts when no letter, digit, `_`, `-` or `.` precedes it and no letter,
//! digit or `_` follows it, once, as its longest form. Dot-directories and
//! build directories (a name starting `target`) are not entered.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use serde::Serialize;

use crate::census::Diagnostic;
use crate::config::CodeConfig;
use crate::walk;

/// One occurrence of a document path in a code file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Citation {
    /// The document cited, corpus-relative.
    pub document: String,
    /// The code file, corpus-relative.
    pub file: String,
    /// 1-based line of the occurrence.
    pub line: usize,
}

/// What the code scan found.
#[derive(Debug, Clone, Default)]
pub struct CodeScan {
    /// Code files read.
    pub files: usize,
    /// Configured code roots that could not be read.
    pub roots_missing: usize,
    /// In file, then line order.
    pub citations: Vec<Citation>,
}

impl CodeScan {
    /// Documents cited at least once.
    pub fn documents_cited(&self) -> usize {
        self.citations
            .iter()
            .map(|citation| citation.document.as_str())
            .collect::<BTreeSet<_>>()
            .len()
    }
}

/// Every form of every document, by length, longest first.
struct Forms {
    by_length: Vec<(usize, HashMap<Vec<u8>, usize>)>,
    /// The extensions of the documents as written, with the dot: where an
    /// occurrence can end.
    suffixes: Vec<Vec<u8>>,
}

impl Forms {
    /// A document's own path beats a stripped form of another document;
    /// otherwise the first document in path order keeps a shared form.
    fn new(documents: &[String], strip: &[String]) -> Self {
        let mut forms: HashMap<Vec<u8>, (usize, bool)> = HashMap::new();
        let mut suffixes = BTreeSet::new();
        for (index, document) in documents.iter().enumerate() {
            let name = document.rsplit('/').next().unwrap_or(document);
            if let Some(dot) = name.rfind('.') {
                suffixes.insert(name.as_bytes()[dot..].to_vec());
            }
            let full = forms
                .entry(document.as_bytes().to_vec())
                .or_insert((index, true));
            if !full.1 {
                *full = (index, true);
            }
            for prefix in strip {
                if let Some(rest) = document.strip_prefix(prefix.as_str())
                    && !rest.is_empty()
                {
                    forms
                        .entry(rest.as_bytes().to_vec())
                        .or_insert((index, false));
                }
            }
        }
        let mut by_length: BTreeMap<Reverse<usize>, HashMap<Vec<u8>, usize>> = BTreeMap::new();
        for (form, (index, _)) in forms {
            by_length
                .entry(Reverse(form.len()))
                .or_default()
                .insert(form, index);
        }
        Self {
            by_length: by_length
                .into_iter()
                .map(|(Reverse(length), forms)| (length, forms))
                .collect(),
            suffixes: suffixes.into_iter().collect(),
        }
    }

    /// Bounded occurrences in `bytes`: (start, end, document), leftmost
    /// first, the longest at a start, never overlapping.
    fn occurrences(&self, bytes: &[u8]) -> Vec<(usize, usize, usize)> {
        let mut candidates = Vec::new();
        for (dot, _) in bytes.iter().enumerate().filter(|&(_, &byte)| byte == b'.') {
            for suffix in &self.suffixes {
                if !bytes[dot..].starts_with(suffix) {
                    continue;
                }
                let end = dot + suffix.len();
                if !right_bounded(bytes, end) {
                    continue;
                }
                for (length, forms) in &self.by_length {
                    if *length > end {
                        continue;
                    }
                    let start = end - length;
                    if let Some(&document) = forms.get(&bytes[start..end])
                        && left_bounded(bytes, start)
                    {
                        candidates.push((start, end, document));
                        break;
                    }
                }
            }
        }
        candidates.sort_by_key(|&(start, end, _)| (start, Reverse(end)));
        let mut kept: Vec<(usize, usize, usize)> = Vec::new();
        for candidate in candidates {
            if kept.last().is_none_or(|last| candidate.0 >= last.1) {
                kept.push(candidate);
            }
        }
        kept
    }
}

/// The name every build directory starts with (Cargo's `target`, and the
/// `target-*` variants of a second build).
const BUILD_DIRECTORY: &str = "target";

/// Scans the code roots for the documents' paths.
pub(crate) fn scan(
    root: &Path,
    code: &CodeConfig,
    documents: &[String],
    diagnostics: &mut Vec<Diagnostic>,
) -> CodeScan {
    let mut result = CodeScan::default();
    if code.roots.is_empty() {
        return result;
    }
    let (files, missing) = walk::files(
        root,
        &code.roots,
        &|name, relative| code.keeps(name, relative),
        &|name| !name.starts_with(BUILD_DIRECTORY),
        diagnostics,
    );
    result.roots_missing = missing;
    let forms = Forms::new(documents, &code.strip);
    for file in files {
        let bytes = match fs::read(root.join(&file)) {
            Ok(bytes) => bytes,
            Err(error) => {
                diagnostics.push(Diagnostic {
                    path: file,
                    line: None,
                    message: format!("code file skipped: {error}"),
                });
                continue;
            }
        };
        result.files += 1;
        let mut line = 1;
        let mut counted_to = 0;
        for (start, _, document) in forms.occurrences(&bytes) {
            line += bytes[counted_to..start]
                .iter()
                .filter(|&&byte| byte == b'\n')
                .count();
            counted_to = start;
            result.citations.push(Citation {
                document: documents[document].clone(),
                file: file.clone(),
                line,
            });
        }
    }
    result
}

fn left_bounded(bytes: &[u8], start: usize) -> bool {
    !char_before(bytes, start).is_some_and(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

fn right_bounded(bytes: &[u8], end: usize) -> bool {
    !char_at(bytes, end).is_some_and(|c| c.is_alphanumeric() || c == '_')
}

/// The char ending at `at`; an undecodable byte reads as no letter.
fn char_before(bytes: &[u8], at: usize) -> Option<char> {
    (1..=at.min(4)).find_map(|length| {
        std::str::from_utf8(&bytes[at - length..at])
            .ok()
            .and_then(|text| text.chars().next_back())
    })
}

/// The char starting at `at`; an undecodable byte reads as no letter.
fn char_at(bytes: &[u8], at: usize) -> Option<char> {
    (1..=bytes.len().saturating_sub(at).min(4)).find_map(|length| {
        std::str::from_utf8(&bytes[at..at + length])
            .ok()
            .and_then(|text| text.chars().next())
    })
}
