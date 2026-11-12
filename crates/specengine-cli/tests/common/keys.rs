//! One compact JSON document with some object members taken out, every other
//! byte kept: how a golden recorded before a slice that only adds keys is
//! compared after it (flow-scenario, RM-0094: the four keys of
//! [`FLOW_SCENARIO_KEYS`], its test churn; doc-modal, RM-0095: the three of
//! [`DOC_MODAL_KEYS`], each only in the object that gains it). The scan reads strings with their
//! escapes, so a key's name inside a value is never taken; a document that is
//! not compact JSON (whitespace between tokens) is refused with a panic, not
//! guessed at.

/// The keys flow-scenario (RM-0094) adds: `in_flow` on a flow node,
/// `not_in_flow` and `types` on a flow read, `ancestors` on a shown node.
pub const FLOW_SCENARIO_KEYS: [&str; 4] = ["in_flow", "not_in_flow", "types", "ancestors"];

/// The three a `spec flow` document gains. A `bundle` document has a layer
/// named `ancestors` of its own, so each command is stripped of its own
/// keys alone.
pub const FLOW_KEYS: [&str; 3] = ["in_flow", "not_in_flow", "types"];

/// The ones a `spec show` document gains: flow-scenario's `ancestors`
/// (RM-0094) and node-history's `history` (RM-0035, `null` unless asked),
/// each a shown node's.
pub const SHOW_KEYS: [&str; 2] = ["ancestors", "history"];

/// docs/features/doc-modal.md (RM-0095), its test churn: a shown link's
/// `title` and `end_kind` (D4) and a gap row's `titles` (D5, RM-0101).
pub const DOC_MODAL_KEYS: [&str; 3] = ["title", "end_kind", "titles"];

/// Whether a member is dropped: its key, and every key of its object.
type Dropped<'k> = &'k dyn Fn(&str, &[String]) -> bool;

/// `text` (one compact JSON document, then anything, e.g. its line end) with
/// every member named by `keys` removed, at any depth, with its one comma.
pub fn without_keys(text: &str, keys: &[&str]) -> String {
    without_where(text, &|key: &str, _: &[String]| keys.contains(&key))
}

/// `text` (one compact JSON document) with doc-modal's keys taken out
/// ([`DOC_MODAL_KEYS`]), every other byte kept: `title` and `end_kind` only
/// from an object that holds `end_kind` (a shown link; a node's own `title`
/// stays), `titles` only from one that holds `items` and `code` (a gap row).
pub fn without_doc_modal_keys(text: &str) -> String {
    without_where(text, &|key: &str, keys: &[String]| {
        let has = |name: &str| keys.iter().any(|key| key == name);
        match key {
            "title" | "end_kind" => has("end_kind"),
            "titles" => has("items") && has("code"),
            _ => false,
        }
    })
}

fn without_where(text: &str, keys: Dropped<'_>) -> String {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut out = String::with_capacity(text.len());
    value(text, bytes, &mut at, keys, &mut out);
    out.push_str(&text[at..]);
    out
}

fn value(text: &str, bytes: &[u8], at: &mut usize, keys: Dropped<'_>, out: &mut String) {
    match bytes.get(*at) {
        Some(b'{') => object(text, bytes, at, keys, out),
        Some(b'[') => array(text, bytes, at, keys, out),
        Some(b'"') => {
            let start = *at;
            string_end(bytes, at);
            out.push_str(&text[start..*at]);
        }
        Some(_) => {
            let start = *at;
            while let Some(&byte) = bytes.get(*at) {
                if matches!(byte, b',' | b'}' | b']') || byte.is_ascii_whitespace() {
                    break;
                }
                *at += 1;
            }
            assert!(*at > start, "a JSON value at byte {start}");
            out.push_str(&text[start..*at]);
        }
        None => panic!("the JSON document ends early"),
    }
}

/// Moves `at` past the string opening at it.
fn string_end(bytes: &[u8], at: &mut usize) {
    assert_eq!(bytes.get(*at), Some(&b'"'), "a string at byte {at}");
    *at += 1;
    loop {
        match bytes.get(*at) {
            Some(b'\\') => *at += 2,
            Some(b'"') => {
                *at += 1;
                return;
            }
            Some(_) => *at += 1,
            None => panic!("a string never closes"),
        }
    }
}

fn expect(bytes: &[u8], at: usize, byte: u8) {
    assert_eq!(
        bytes.get(at).copied(),
        Some(byte),
        "compact JSON expected `{}` at byte {at}",
        byte as char
    );
}

fn object(text: &str, bytes: &[u8], at: &mut usize, keys: Dropped<'_>, out: &mut String) {
    expect(bytes, *at, b'{');
    *at += 1;
    out.push('{');
    if bytes.get(*at) == Some(&b'}') {
        *at += 1;
        out.push('}');
        return;
    }
    // Every member first, so a drop can see its object's other keys.
    let mut members: Vec<(String, String)> = Vec::new();
    loop {
        let key_start = *at;
        string_end(bytes, at);
        let key = text[key_start + 1..*at - 1].to_owned();
        expect(bytes, *at, b':');
        *at += 1;
        let mut member = String::new();
        member.push_str(&text[key_start..*at]);
        value(text, bytes, at, keys, &mut member);
        members.push((key, member));
        match bytes.get(*at) {
            Some(b',') => *at += 1,
            Some(b'}') => {
                *at += 1;
                break;
            }
            _ => panic!("compact JSON expected `,` or `}}` at byte {at}"),
        }
    }
    let names: Vec<String> = members.iter().map(|(key, _)| key.clone()).collect();
    let mut first = true;
    for (key, member) in &members {
        if keys(key, &names) {
            continue;
        }
        if !first {
            out.push(',');
        }
        out.push_str(member);
        first = false;
    }
    out.push('}');
}

fn array(text: &str, bytes: &[u8], at: &mut usize, keys: Dropped<'_>, out: &mut String) {
    expect(bytes, *at, b'[');
    *at += 1;
    out.push('[');
    if bytes.get(*at) == Some(&b']') {
        *at += 1;
        out.push(']');
        return;
    }
    loop {
        value(text, bytes, at, keys, out);
        match bytes.get(*at) {
            Some(b',') => {
                *at += 1;
                out.push(',');
            }
            Some(b']') => {
                *at += 1;
                out.push(']');
                return;
            }
            _ => panic!("compact JSON expected `,` or `]` at byte {at}"),
        }
    }
}
