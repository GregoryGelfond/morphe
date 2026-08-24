//! Shared helpers for the morphe integration suites (docs/design/morphe.md §13):
//! a `Source` over an inline program, options for a dialect, and the vendored
//! corpus classified through the tier morphe trusts (§5.7). A dev-dependency
//! library — its helpers are public API, so a suite that uses only a subset of
//! them pays no dead-code allow.
#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};

use morphe::themelios_syntax::base::source::{Source, SourceId};
use morphe::themelios_syntax::equiv::non_whitespace_tokens;
use morphe::themelios_syntax::parse::parse;
use morphe::{Dialect, FormatOptions};

/// A `Source` for `text`; the ephemeral id is irrelevant to formatting.
///
/// # Panics
///
/// Panics if `text` exceeds `Source::MAX_LEN` (~4 GiB) — no test input does.
#[must_use]
pub fn source(text: &str) -> Source {
    Source::new(SourceId::new(0), text.to_owned()).expect("a test input within Source::MAX_LEN")
}

/// The format options for `dialect`, defaults otherwise.
#[must_use]
pub fn options(dialect: Dialect) -> FormatOptions {
    FormatOptions {
        dialect,
        ..FormatOptions::default()
    }
}

/// One vendored corpus input, classified by parsing it (§5.7).
pub struct Case {
    /// The input's path, for diagnostics.
    pub path: PathBuf,
    /// The input text, as vendored.
    pub text: String,
    /// The dialect the corpus is read under.
    pub dialect: Dialect,
    /// The tier admits it (no parse errors).
    pub is_member: bool,
    /// A member carrying no comment token of any kind (trivia or `%!` docs) —
    /// the subset the thin lowering granted before comments were woven (§13).
    pub comment_free: bool,
}

/// Every `.lp` input under the corpus, classified, in path order.
///
/// # Panics
///
/// Panics if the corpus directory cannot be read, or if it holds no input.
#[must_use]
pub fn cases() -> Vec<Case> {
    let mut cases = Vec::new();
    collect(&corpus_dir(), &mut cases);
    assert!(!cases.is_empty(), "the corpus is empty");
    cases
}

/// The corpus root — `tests/corpus/` beside the `morphe` crate, this crate's
/// sibling.
fn corpus_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../morphe/tests/corpus")
}

fn collect(dir: &Path, out: &mut Vec<Case>) {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
        .unwrap_or_else(|err| panic!("read {}: {err}", dir.display()))
        .map(|entry| entry.expect("a corpus dir entry").path())
        .collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "lp") {
            let text = std::fs::read_to_string(&path).expect("read a corpus input");
            // The vendored corpus is clingo throughout (§13; ASP-Core-2 later).
            let dialect = Dialect::Clingo;
            let parsed = parse(&source(&text), dialect);
            let root = parsed.syntax();
            let is_member = !parsed.has_errors();
            let comment_free =
                is_member && non_whitespace_tokens(&root).all(|token| !token.kind().is_comment());
            out.push(Case {
                path,
                text,
                dialect,
                is_member,
                comment_free,
            });
        }
    }
}
