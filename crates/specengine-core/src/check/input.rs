//! What the check runs over: a set of parses, each with its path, size and
//! bytes, plus what the walk could not do. The check reads no file: a
//! loader (the store's `check_input`, later the index) builds this.

use specengine_model::{IdScheme, ParsedFile};

/// The files of one walk and the walk's problems.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CheckInput {
    pub files: Vec<CheckFile>,
    pub problems: Vec<Problem>,
}

/// One walked file.
#[derive(Debug, Clone, PartialEq)]
pub struct CheckFile {
    /// Root-relative, `/`-separated.
    pub path: String,
    /// Bytes of the whole file (BOM and front-matter included); 0 when not
    /// read.
    pub size: u64,
    /// The parse; `None` when the file was not read or not parsed.
    pub parsed: Option<ParsedFile>,
    /// Why the file was not read or not parsed: the check cannot vouch for
    /// the corpus (exit 2).
    pub read_error: Option<String>,
    /// The bytes the parse was made from: lines of spans, the front-matter
    /// keys as written, a parser diagnostic's span text. Empty: every such
    /// line is 1 and every key is judged by the parse alone.
    pub bytes: Vec<u8>,
}

impl CheckFile {
    /// A read file: parsed here.
    pub fn parse(path: impl Into<String>, bytes: Vec<u8>, scheme: &IdScheme) -> Self {
        let path = path.into();
        let parsed = crate::parse(&path, &bytes, scheme);
        Self::parsed(path, bytes, parsed)
    }

    /// A read file with its parse.
    pub fn parsed(path: impl Into<String>, bytes: Vec<u8>, parsed: ParsedFile) -> Self {
        Self {
            path: path.into(),
            size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            parsed: Some(parsed),
            read_error: None,
            bytes,
        }
    }

    /// A listed file that could not be read or parsed.
    pub fn unreadable(path: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            size: 0,
            parsed: None,
            read_error: Some(error.into()),
            bytes: Vec::new(),
        }
    }
}

/// Something the walk could not do.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Problem {
    pub kind: ProblemKind,
    /// Root-relative; `""` for the root itself or an unnamed entry.
    pub path: String,
}

/// What went wrong in the walk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ProblemKind {
    /// A configured root names no directory and no `.md` file: "cannot
    /// check" when `[paths] roots` was written, ignored for a default role
    /// root.
    MissingRoot,
    /// A directory (or the root) could not be listed: "cannot check".
    UnreadableDir,
    /// A directory or `.md` name that is not UTF-8 was skipped: the warning
    /// `name-skipped`.
    SkippedName,
}
