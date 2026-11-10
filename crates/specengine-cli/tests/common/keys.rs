//! One compact JSON document with some object members taken out, every other
//! byte kept: how a golden recorded before a slice that only adds keys is
//! compared after it (flow-scenario, RM-0094: the four keys of
//! [`FLOW_SCENARIO_KEYS`], its test churn). The scan reads strings with their
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

/// The one a `spec show` document gains.
pub const SHOW_KEYS: [&str; 1] = ["ancestors"];

/// `text` (one compact JSON document, then anything, e.g. its line end) with
/// every member named by `keys` removed, at any depth, with its one comma.
pub fn without_keys(text: &str, keys: &[&str]) -> String {
    let bytes = text.as_bytes();
    let mut at = 0;
    let mut out = String::with_capacity(text.len());
    value(text, bytes, &mut at, keys, &mut out);
    out.push_str(&text[at..]);
    out
}

fn value(text: &str, bytes: &[u8], at: &mut usize, keys: &[&str], out: &mut String) {
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

fn object(text: &str, bytes: &[u8], at: &mut usize, keys: &[&str], out: &mut String) {
    expect(bytes, *at, b'{');
    *at += 1;
    out.push('{');
    let mut first = true;
    if bytes.get(*at) == Some(&b'}') {
        *at += 1;
        out.push('}');
        return;
    }
    loop {
        let key_start = *at;
        string_end(bytes, at);
        let key = &text[key_start + 1..*at - 1];
        expect(bytes, *at, b':');
        *at += 1;
        let mut member = String::new();
        member.push_str(&text[key_start..*at]);
        value(text, bytes, at, keys, &mut member);
        if !keys.contains(&key) {
            if !first {
                out.push(',');
            }
            out.push_str(&member);
            first = false;
        }
        match bytes.get(*at) {
            Some(b',') => *at += 1,
            Some(b'}') => {
                *at += 1;
                out.push('}');
                return;
            }
            _ => panic!("compact JSON expected `,` or `}}` at byte {at}"),
        }
    }
}

fn array(text: &str, bytes: &[u8], at: &mut usize, keys: &[&str], out: &mut String) {
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
