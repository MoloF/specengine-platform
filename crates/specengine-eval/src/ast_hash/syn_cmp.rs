//! `syn` 3 as a digest candidate (ADR-0021), comparison only, behind feature `syn`.
//!
//! Naive: the structural `Hash` of each item (`extra-traits`; spans cannot leak
//! in). Normalized: the item's token stream with `#[doc]` attributes and
//! trailing commas removed, hashed as text — the same normalization the
//! tree-sitter recipe applies.

use std::collections::BTreeMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::time::Instant;

use proc_macro2::{Delimiter, Group, TokenStream, TokenTree};
use quote::ToTokens;
use syn::{ImplItem, Item, TraitItem};

use super::{Perturbation, SourceFile, SynSummary};
use crate::harness::stability_percent;

#[derive(Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
struct Key {
    mod_path: Vec<String>,
    owner: Option<String>,
    kind: &'static str,
    name: String,
    ordinal: usize,
}

struct Hashes {
    naive: u64,
    normalized: u64,
}

/// Runs the comparison over the same originals and perturbed texts as the
/// tree-sitter measurement; `tree_sitter_us` is its parse-and-hash time.
pub fn run(
    files: &[SourceFile],
    perturbed: &BTreeMap<Perturbation, Vec<Option<String>>>,
    tree_sitter_us: u128,
) -> SynSummary {
    let started = Instant::now();
    let mut files_failed = 0;
    let originals: Vec<Option<BTreeMap<Key, Hashes>>> = files
        .iter()
        .map(|file| match syn::parse_file(&file.text) {
            Ok(parsed) => Some(hash_items(&parsed.items)),
            Err(_) => {
                files_failed += 1;
                None
            }
        })
        .collect();
    let syn_us = started.elapsed().as_micros();

    let mut naive_by = BTreeMap::new();
    let mut normalized_by = BTreeMap::new();
    let mut compared = 0;
    let mut naive_equal = 0;
    let mut normalized_equal = 0;
    for (perturbation, texts) in perturbed {
        let mut compared_here = 0;
        let mut naive_here = 0;
        let mut normalized_here = 0;
        for (index, original) in originals.iter().enumerate() {
            let Some(original) = original else {
                continue;
            };
            let text = texts
                .get(index)
                .and_then(|text| text.as_deref())
                .unwrap_or(&files[index].text);
            let after = syn::parse_file(text)
                .ok()
                .map(|parsed| hash_items(&parsed.items));
            for (key, before) in original {
                compared_here += 1;
                let Some(after) = after.as_ref().and_then(|items| items.get(key)) else {
                    continue;
                };
                if after.naive == before.naive {
                    naive_here += 1;
                }
                if after.normalized == before.normalized {
                    normalized_here += 1;
                }
            }
        }
        naive_by.insert(
            perturbation.as_str(),
            stability_percent(naive_here, compared_here),
        );
        normalized_by.insert(
            perturbation.as_str(),
            stability_percent(normalized_here, compared_here),
        );
        compared += compared_here;
        naive_equal += naive_here;
        normalized_equal += normalized_here;
    }

    let time_ratio = if tree_sitter_us == 0 {
        0.0
    } else {
        (syn_us as f64 / tree_sitter_us as f64 * 100.0).round() / 100.0
    };
    SynSummary {
        files_failed,
        naive_stable_pct: stability_percent(naive_equal, compared),
        normalized_stable_pct: stability_percent(normalized_equal, compared),
        time_ratio,
        naive_by_perturbation: naive_by,
        normalized_by_perturbation: normalized_by,
    }
}

fn hash_items(items: &[Item]) -> BTreeMap<Key, Hashes> {
    let mut out = BTreeMap::new();
    let mut ordinals = BTreeMap::new();
    collect(items, &mut Vec::new(), &mut ordinals, &mut out);
    out
}

type Ordinals = BTreeMap<(Vec<String>, Option<String>, &'static str, String), usize>;

fn push(
    out: &mut BTreeMap<Key, Hashes>,
    ordinals: &mut Ordinals,
    mod_path: &[String],
    owner: Option<&str>,
    kind: &'static str,
    name: String,
    node: &(impl Hash + ToTokens),
) {
    let slot = ordinals
        .entry((
            mod_path.to_vec(),
            owner.map(str::to_owned),
            kind,
            name.clone(),
        ))
        .or_insert(0);
    let key = Key {
        mod_path: mod_path.to_vec(),
        owner: owner.map(str::to_owned),
        kind,
        name,
        ordinal: *slot,
    };
    *slot += 1;
    out.insert(key, hashes_of(node));
}

fn collect(
    items: &[Item],
    mod_path: &mut Vec<String>,
    ordinals: &mut Ordinals,
    out: &mut BTreeMap<Key, Hashes>,
) {
    for item in items {
        match item {
            Item::Fn(f) => push(
                out,
                ordinals,
                mod_path,
                None,
                "fn",
                f.sig.ident.to_string(),
                f,
            ),
            Item::Struct(s) => push(
                out,
                ordinals,
                mod_path,
                None,
                "struct",
                s.ident.to_string(),
                s,
            ),
            Item::Enum(e) => push(
                out,
                ordinals,
                mod_path,
                None,
                "enum",
                e.ident.to_string(),
                e,
            ),
            Item::Union(u) => push(
                out,
                ordinals,
                mod_path,
                None,
                "union",
                u.ident.to_string(),
                u,
            ),
            Item::Const(c) => push(
                out,
                ordinals,
                mod_path,
                None,
                "const",
                c.ident.to_string(),
                c,
            ),
            Item::Static(s) => push(
                out,
                ordinals,
                mod_path,
                None,
                "static",
                s.ident.to_string(),
                s,
            ),
            Item::Type(t) => push(
                out,
                ordinals,
                mod_path,
                None,
                "type",
                t.ident.to_string(),
                t,
            ),
            Item::Macro(m) => {
                let name = m
                    .ident
                    .as_ref()
                    .map_or_else(|| "macro".to_owned(), ToString::to_string);
                push(out, ordinals, mod_path, None, "macro", name, m);
            }
            Item::Mod(m) => {
                let name = m.ident.to_string();
                push(out, ordinals, mod_path, None, "mod", name.clone(), m);
                if let Some((_, content)) = &m.content {
                    mod_path.push(name);
                    collect(content, mod_path, ordinals, out);
                    mod_path.pop();
                }
            }
            Item::Trait(t) => {
                let name = t.ident.to_string();
                push(out, ordinals, mod_path, None, "trait", name.clone(), t);
                for member in &t.items {
                    if let TraitItem::Fn(f) = member {
                        push(
                            out,
                            ordinals,
                            mod_path,
                            Some(&name),
                            "fn",
                            f.sig.ident.to_string(),
                            f,
                        );
                    }
                }
            }
            Item::Impl(i) => {
                let self_ty = compact(&i.self_ty.to_token_stream().to_string());
                let label = match &i.trait_ {
                    Some((path, _)) => {
                        format!(
                            "<{self_ty} as {}>",
                            compact(&path.to_token_stream().to_string())
                        )
                    }
                    None => self_ty,
                };
                push(out, ordinals, mod_path, None, "impl", label.clone(), i);
                for member in &i.items {
                    if let ImplItem::Fn(f) = member {
                        push(
                            out,
                            ordinals,
                            mod_path,
                            Some(&label),
                            "fn",
                            f.sig.ident.to_string(),
                            f,
                        );
                    }
                }
            }
            _ => {}
        }
    }
}

fn compact(text: &str) -> String {
    text.chars().filter(|c| !c.is_whitespace()).collect()
}

fn hashes_of(node: &(impl Hash + ToTokens)) -> Hashes {
    let mut naive = DefaultHasher::new();
    node.hash(&mut naive);
    let mut normalized = DefaultHasher::new();
    normalize_tokens(node.to_token_stream())
        .to_string()
        .hash(&mut normalized);
    Hashes {
        naive: naive.finish(),
        normalized: normalized.finish(),
    }
}

/// Drops `#[doc = ...]` / `#![doc = ...]` attributes and trailing commas
/// inside every delimited group, recursively.
fn normalize_tokens(stream: TokenStream) -> TokenStream {
    let trees: Vec<TokenTree> = stream.into_iter().collect();
    let mut out: Vec<TokenTree> = Vec::with_capacity(trees.len());
    let mut index = 0;
    while index < trees.len() {
        if let TokenTree::Punct(pound) = &trees[index]
            && pound.as_char() == '#'
        {
            let mut next = index + 1;
            if let Some(TokenTree::Punct(bang)) = trees.get(next)
                && bang.as_char() == '!'
            {
                next += 1;
            }
            if let Some(TokenTree::Group(group)) = trees.get(next)
                && group.delimiter() == Delimiter::Bracket
                && is_doc_attribute(group)
            {
                index = next + 1;
                continue;
            }
        }
        match &trees[index] {
            TokenTree::Group(group) => {
                let mut inner: Vec<TokenTree> =
                    normalize_tokens(group.stream()).into_iter().collect();
                if let Some(TokenTree::Punct(last)) = inner.last()
                    && last.as_char() == ','
                {
                    inner.pop();
                }
                out.push(TokenTree::Group(Group::new(
                    group.delimiter(),
                    inner.into_iter().collect(),
                )));
            }
            other => out.push(other.clone()),
        }
        index += 1;
    }
    out.into_iter().collect()
}

fn is_doc_attribute(group: &Group) -> bool {
    let mut tokens = group.stream().into_iter();
    matches!(tokens.next(), Some(TokenTree::Ident(ident)) if ident == "doc")
        && matches!(tokens.next(), Some(TokenTree::Punct(punct)) if punct.as_char() == '=')
}
