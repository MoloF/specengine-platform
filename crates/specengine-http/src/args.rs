//! The query string and the path's parts (docs/features/daemon-read.md
//! "Data"): query names are the MCP arguments; an array repeats its key
//! (`kinds=a&kinds=b`), an integer is decimal, a boolean `true` or
//! `false`, `with` only `links`. 400: a name the endpoint does not take
//! (listing its names), a scalar given twice, a bad value, a required
//! name missing. A REF is the rest of the path after `nodes/`,
//! percent-decoded once as UTF-8 (a bad `%` or non-UTF-8: 400).
//!
//! The query string is read from the raw URI and decoded strictly, as a
//! form (axum's `Query` reads it so: `+` a space), but a `%` not followed
//! by two hex digits or bytes that are not UTF-8 are a 400 naming the
//! parameter, never passed on as written or as U+FFFD.

/// An endpoint's query, its names checked.
pub(crate) struct Args {
    pairs: Vec<(String, String)>,
}

impl Args {
    /// The pairs of `query` (the raw query string, if any) when each
    /// decodes and every name is one of `names` (`endpoint` names the
    /// endpoint in the message).
    pub(crate) fn new(endpoint: &str, names: &[&str], query: Option<&str>) -> Result<Self, String> {
        let pairs = query_pairs(query.unwrap_or(""))?;
        if let Some((name, _)) = pairs
            .iter()
            .find(|(name, _)| !names.contains(&name.as_str()))
        {
            let takes = if names.is_empty() {
                "takes no query name".to_owned()
            } else {
                format!("takes {}", names.join(", "))
            };
            return Err(format!(
                "`{name}` is no query name of {endpoint}: it {takes}"
            ));
        }
        Ok(Self { pairs })
    }

    /// Every value of `name`, in order (an array).
    pub(crate) fn texts(&self, name: &str) -> Vec<String> {
        self.pairs
            .iter()
            .filter(|(key, _)| key == name)
            .map(|(_, value)| value.clone())
            .collect()
    }

    /// The one value of `name` (a scalar), if given.
    pub(crate) fn text(&self, name: &str) -> Result<Option<String>, String> {
        let mut values = self.texts(name);
        match values.len() {
            0 => Ok(None),
            1 => Ok(values.pop()),
            _ => Err(format!(
                "`{name}` is given {} times: it takes one value",
                values.len()
            )),
        }
    }

    /// The one value of `name`, which must be given.
    pub(crate) fn required_text(&self, name: &str) -> Result<String, String> {
        self.text(name)?
            .ok_or_else(|| format!("`{name}` is required"))
    }

    /// Every value of `name`, at least one.
    pub(crate) fn required_texts(&self, name: &str) -> Result<Vec<String>, String> {
        let values = self.texts(name);
        if values.is_empty() {
            return Err(format!("`{name}` is required (repeat it for more)"));
        }
        Ok(values)
    }

    /// The one value of `name` as a decimal integer (`-` allowed), if given.
    pub(crate) fn integer(&self, name: &str) -> Result<Option<i64>, String> {
        let Some(value) = self.text(name)? else {
            return Ok(None);
        };
        let digits = value.strip_prefix('-').unwrap_or(&value);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(format!("`{name}={value}`: not a decimal integer"));
        }
        value
            .parse()
            .map(Some)
            .map_err(|_| format!("`{name}={value}`: out of range"))
    }

    /// The one value of `name` as `true` or `false`, if given.
    pub(crate) fn boolean(&self, name: &str) -> Result<Option<bool>, String> {
        match self.text(name)?.as_deref() {
            None => Ok(None),
            Some("true") => Ok(Some(true)),
            Some("false") => Ok(Some(false)),
            Some(value) => Err(format!("`{name}={value}`: not `true` or `false`")),
        }
    }

    /// `with`: each value `links`; given at least once.
    pub(crate) fn with_links(&self) -> Result<bool, String> {
        let values = self.texts("with");
        if let Some(value) = values.iter().find(|value| value.as_str() != "links") {
            return Err(format!("`with={value}`: the only value is `links`"));
        }
        Ok(!values.is_empty())
    }
}

/// The raw (still percent-encoded) path segments after
/// `/api/projects/`: the slug's, then the rest of the path whole.
pub(crate) fn project_path(path: &str) -> Option<(&str, &str)> {
    let rest = path.strip_prefix("/api/projects/")?;
    Some(rest.split_once('/').unwrap_or((rest, "")))
}

/// The pairs of a raw query string, in order: pieces between `&` (an
/// empty one skipped), a name split from its value at the first `=` (none:
/// the value is empty), each decoded as a form ([`decode`], `+` a space).
fn query_pairs(query: &str) -> Result<Vec<(String, String)>, String> {
    let mut pairs = Vec::new();
    for piece in query.split('&').filter(|piece| !piece.is_empty()) {
        let (raw_name, raw_value) = piece.split_once('=').unwrap_or((piece, ""));
        let name =
            decode(raw_name, true).map_err(|why| format!("the query name `{raw_name}`: {why}"))?;
        let value = decode(raw_value, true)
            .map_err(|why| format!("the value of `{name}` (`{raw_value}`): {why}"))?;
        pairs.push((name, value));
    }
    Ok(pairs)
}

/// `raw` percent-decoded once: each `%` must start two hex digits, the
/// bytes must be UTF-8.
pub(crate) fn percent_decode(raw: &str) -> Result<String, String> {
    decode(raw, false).map_err(|why| format!("`{raw}`: {why}"))
}

/// `raw` percent-decoded once (`plus`: a `+` is a space, as in a form),
/// else why not.
fn decode(raw: &str, plus: bool) -> Result<String, String> {
    let bytes = raw.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut at = 0;
    while at < bytes.len() {
        match bytes[at] {
            b'%' => {
                let high = bytes.get(at + 1).copied().and_then(hex_value);
                let low = bytes.get(at + 2).copied().and_then(hex_value);
                let (Some(high), Some(low)) = (high, low) else {
                    return Err(format!(
                        "the `%` at byte {at} is not followed by two hex digits"
                    ));
                };
                out.push(high << 4 | low);
                at += 3;
            }
            b'+' if plus => {
                out.push(b' ');
                at += 1;
            }
            byte => {
                out.push(byte);
                at += 1;
            }
        }
    }
    String::from_utf8(out).map_err(|_| "percent-decoded, it is not UTF-8".to_owned())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_once_and_refuses_a_bad_escape_or_non_utf8() {
        assert_eq!(percent_decode("A%2FB%23C").as_deref(), Ok("A/B#C"));
        assert_eq!(percent_decode("X%2523Y").as_deref(), Ok("X%23Y"));
        assert!(percent_decode("X%2").is_err());
        assert!(percent_decode("X%zz").is_err());
        assert!(percent_decode("%FF").is_err());
    }

    #[test]
    fn splits_the_project_path() {
        assert_eq!(
            project_path("/api/projects/a/nodes/x%2Fy"),
            Some(("a", "nodes/x%2Fy"))
        );
        assert_eq!(project_path("/api/projects/a"), Some(("a", "")));
        assert_eq!(project_path("/api/other"), None);
    }
}
