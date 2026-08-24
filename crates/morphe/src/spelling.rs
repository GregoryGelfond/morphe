//! Spelling canonicalization (docs/design/morphe.md §7.4): unless spellings
//! are preserved, each synonym token is normalized to the tier's canonical
//! form. The synonym table is language knowledge and lives once, in the tier;
//! morphe only *reads* it and decides nothing about which spellings are
//! synonyms. Canonicalization is idempotent (§5.4).

use std::borrow::Cow;

use themelios_syntax::equiv::canonical_spelling;
use themelios_syntax::tree::SyntaxKind;

/// The spelling to emit for a `kind` token whose text is `text`: the tier's
/// canonical form when `!preserve` (§7.4), the verbatim text when `preserve`.
/// A one-line delegation to `canonical_spelling` — the point of the module is
/// that the synonym table lives once, in the tier, and morphe only reads it.
/// Total; O(1); borrows on the common path (the identity on non-synonym
/// kinds), owning only where the tier respells a synonym.
pub(crate) fn spell(kind: SyntaxKind, text: &str, preserve: bool) -> Cow<'_, str> {
    if preserve {
        Cow::Borrowed(text)
    } else {
        canonical_spelling(kind, text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserve_returns_the_verbatim_text() {
        assert_eq!(spell(SyntaxKind::NEQ, "<>", true), "<>");
        assert_eq!(
            spell(SyntaxKind::KW_MINIMIZE, "#minimise", true),
            "#minimise"
        );
    }

    #[test]
    fn default_normalizes_to_the_tiers_canonical_spelling() {
        // The canonical forms of §7.4: "=", "!=", "#inf", "#sup", "#minimize",
        // "#maximize" — the exact strings are the tier's.
        assert_eq!(
            spell(SyntaxKind::NEQ, "<>", false),
            canonical_spelling(SyntaxKind::NEQ, "<>")
        );
        assert_ne!(
            spell(SyntaxKind::KW_MINIMIZE, "#minimise", false),
            "#minimise"
        );
    }

    #[test]
    fn canonicalization_is_idempotent() {
        for (kind, text) in [
            (SyntaxKind::NEQ, "<>"),
            (SyntaxKind::KW_MAXIMIZE, "#maximise"),
        ] {
            let once = spell(kind, text, false).into_owned();
            assert_eq!(spell(kind, &once, false), once);
        }
    }
}
