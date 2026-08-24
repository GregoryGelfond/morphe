//! Rendering morphe's diagnostics (docs/design/morphe.md §11.1): the tier's own
//! parse-error `SyntaxError`s and morphe's warning notes, lowered through base's
//! human view — the same channel and quality bar the tier uses. A parse-error
//! refusal forwards the tier's diagnostics unchanged; morphe adds no parse
//! diagnostics of its own (§11.1).

use morphe::SyntaxError;
use morphe::themelios_syntax::base::diagnostic::{Diagnostic, ToDiagnostic};
use morphe::themelios_syntax::base::source::SourceSet;
use morphe::themelios_syntax::base::view;

/// Render `diagnostics` through base's human view (§11.1) against a one-file
/// catalog naming `label` with the input `text`. Each diagnostic's labels point
/// at the one source morphe parsed (`INPUT_SOURCE_ID`); the first source a
/// `SourceSet` admits is minted that same identity, so the catalog resolves
/// every label. The renderings are concatenated in order, each already
/// newline-terminated.
pub(crate) fn render(label: &str, text: &str, diagnostics: &[Diagnostic]) -> String {
    let mut catalog = SourceSet::new();
    // The input was already admitted as a `Source` before it was parsed, so it
    // is within the catalog's own admission ceiling and this add cannot fail;
    // were it somehow to, the human view degrades to a named placeholder rather
    // than panicking, so the outcome stays total either way (§11.1, §12).
    let _ = catalog.add(label.to_owned(), text.to_owned());
    diagnostics
        .iter()
        .map(|diagnostic| view::human(diagnostic, &catalog))
        .collect()
}

/// The tier's parse-error `SyntaxError`s lowered to base diagnostics, forwarded
/// unchanged (§11.1) — morphe adds none of its own.
pub(crate) fn parse_errors(errors: &[SyntaxError]) -> Vec<Diagnostic> {
    errors.iter().map(ToDiagnostic::to_diagnostic).collect()
}
