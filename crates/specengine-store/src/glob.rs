//! The census glob of `[paths] exclude` (the rule of `specengine-import`'s
//! census, without a regex engine): `*` any run without `/`, `**` any run,
//! `**/` any directories (also none), `?` one character but `/`; every other
//! character literal and case-sensitive; anchored at both ends.

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Literal(char),
    /// `?`.
    One,
    /// `*`.
    Star,
    /// `**` not followed by `/`.
    AnyRun,
    /// `**/`.
    AnyDirs,
}

/// One compiled glob.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Glob {
    tokens: Vec<Token>,
}

impl Glob {
    pub(crate) fn new(glob: &str) -> Self {
        let chars: Vec<char> = glob.chars().collect();
        let mut tokens = Vec::with_capacity(chars.len());
        let mut index = 0;
        while index < chars.len() {
            match chars[index] {
                '*' if chars.get(index + 1) == Some(&'*') => {
                    if chars.get(index + 2) == Some(&'/') {
                        tokens.push(Token::AnyDirs);
                        index += 3;
                    } else {
                        tokens.push(Token::AnyRun);
                        index += 2;
                    }
                }
                '*' => {
                    tokens.push(Token::Star);
                    index += 1;
                }
                '?' => {
                    tokens.push(Token::One);
                    index += 1;
                }
                other => {
                    tokens.push(Token::Literal(other));
                    index += 1;
                }
            }
        }
        Self { tokens }
    }

    /// The whole of `path` matches. Dynamic programming over (token, char):
    /// linear in their product, no backtracking blow-up.
    pub(crate) fn matches(&self, path: &str) -> bool {
        let text: Vec<char> = path.chars().collect();
        let width = text.len() + 1;
        // `next[j]`: tokens[i + 1..] match text[j..]; `here[j]`: tokens[i..].
        let mut next = vec![false; width];
        next[text.len()] = true;
        for token in self.tokens.iter().rev() {
            let mut here = vec![false; width];
            // For `**/`: some '/' at or after j closes a run that `next` continues.
            let mut dirs_from = false;
            for j in (0..width).rev() {
                let current = text.get(j).copied();
                here[j] = match token {
                    Token::Literal(expected) => current == Some(*expected) && next[j + 1],
                    Token::One => current.is_some_and(|c| c != '/') && next[j + 1],
                    Token::Star => next[j] || (current.is_some_and(|c| c != '/') && here[j + 1]),
                    Token::AnyRun => next[j] || (current.is_some() && here[j + 1]),
                    Token::AnyDirs => {
                        if current == Some('/') && next[j + 1] {
                            dirs_from = true;
                        }
                        next[j] || (current.is_some() && dirs_from)
                    }
                };
            }
            next = here;
        }
        next[0]
    }
}
