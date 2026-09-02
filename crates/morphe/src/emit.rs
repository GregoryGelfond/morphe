//! The oracle floor (docs/design/morphe.md §5.5): the minimum separator any
//! correct formatter must leave between two significant tokens. The house
//! style decides the whitespace it *wants*; the fusion oracle decides the
//! floor; `emit` returns the maximum. Queried at Doc-build time on the
//! *canonical* texts morphe will emit (§7.4), so `render` stays
//! oracle-oblivious and breaking a group only strengthens a separator (§5.5).

use std::borrow::Cow;

use themelios_syntax::dialect::Dialect;
use themelios_syntax::fusion::{LexContext, Separator, lex_mode_of, separator_between};
use themelios_syntax::tree::SyntaxToken;

use crate::doc::Doc;

/// The whitespace the house style *wants* between two significant tokens, the
/// minimum before the oracle floor is applied (§5.5): `Abut` for tokens the
/// style would set tight, `Space` for tokens it separates.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Wish {
    /// No space wanted — set the tokens tight (the oracle may still force one).
    Abut,
    /// A space wanted between the tokens.
    Space,
}

/// The separator `emit` places between two significant tokens: the maximum of
/// the style's `wish` and the oracle floor for the emitted texts (§5.5). The
/// floor is read from `left`'s lex mode and the two *canonical* texts (already
/// canonicalized by `spelling`, §7.4) via `separator_between` — never the
/// token-level `separator`, which reads `left.text()` and so cannot serve a
/// canonicalizing formatter (§5.5). Total; O(1) beyond the oracle.
pub(crate) fn separator(
    left: &SyntaxToken,
    left_text: &str,
    right_text: &str,
    wish: Wish,
    dialect: Dialect,
) -> Doc {
    let context = LexContext {
        dialect,
        mode: lex_mode_of(left),
    };
    floor_to_doc(separator_between(left_text, right_text, context), wish)
}

/// Map an oracle `floor` and a style `wish` to a document node (§5.5). A
/// significant adjacency's floor is only `Nothing` or `Whitespace`, so a
/// `Space` wish already stands at or above it. `LineBreak` never arises for a
/// significant adjacency — it is a line-comment/doc/shebang property carried by
/// a `HardLine` (§8.2) — so it is handled as a non-panicking fallback,
/// treated as `Whitespace`: a mis-query can then only *widen* a separator, and
/// the certificate (§5.3) refuses if the query was ever wrong (§12; §5.5's
/// "the certificate is the whole-text backstop should the floor be mis-queried").
///
/// A permitted abutment is **bare adjacency**, not a soft break (§5.5's "or bare
/// adjacency"): the floor is the minimum the oracle demands and never a break of
/// its own, so a group breaking never turns an abutment into a newline. The
/// house style owns every break — the body's `Line`s (§7.2), the bracket
/// interior's `SoftLine`s (§7.2) — and a hug it sets tight stays tight
/// wherever its group falls.
pub(crate) fn floor_to_doc(floor: Separator, wish: Wish) -> Doc {
    match (floor, wish) {
        // Abut a Nothing floor: the tokens sit tight, no separator at all.
        (Separator::Nothing, Wish::Abut) => nothing(),
        // Every other case forces a space — a Space wish (already at or above
        // the floor), or an Abut the oracle forbids (Whitespace; or the
        // never-arising LineBreak, the non-panicking fallback named above).
        _ => space(),
    }
}

/// Bare adjacency: no separator, so two tokens sit tight (§5.5).
fn nothing() -> Doc {
    Doc::Concat(Vec::new())
}

/// A forced single space, as a document node.
fn space() -> Doc {
    Doc::Text(Cow::Borrowed(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doc::render;
    use themelios_syntax::base::source::{Source, SourceId};
    use themelios_syntax::equiv::token_stream;
    use themelios_syntax::parse::parse;

    // Parse an inline member and return its significant tokens in order, so
    // `left`'s mode and dialect are real (`lex_mode_of` reads the token).
    fn sig_tokens(text: &str) -> Vec<SyntaxToken> {
        let source = Source::new(SourceId::new(0), text.to_owned()).expect("a small test source");
        token_stream(&parse(&source, Dialect::Clingo).syntax()).collect()
    }

    // The byte `emit` places between `lt` and `rt`, with `toks[i]` as `left`.
    fn between(member: &str, i: usize, lt: &str, rt: &str, wish: Wish) -> String {
        let toks = sig_tokens(member);
        render(&separator(&toks[i], lt, rt, wish, Dialect::Clingo), 100)
    }

    #[test]
    fn the_floor_forces_space_where_abutment_would_fuse() {
        // `q :- 0 < x1.`: abutting the NUMBER "0" and "x1" would fuse to "0x1",
        // so the wish Abut is upgraded to a space. Token index 2 is the "0".
        assert_eq!(between("q :- 0 < x1.\n", 2, "0", "x1", Wish::Abut), " ");
    }

    #[test]
    fn the_floor_permits_abutment_the_oracle_allows() {
        // `p(a).`: "p"/"(" abut — a Nothing floor is bare adjacency, no
        // separator, so the two sit tight (and stay tight under any group).
        assert_eq!(between("p(a).\n", 0, "p", "(", Wish::Abut), "");
    }

    #[test]
    fn a_wish_for_space_is_already_at_or_above_the_floor() {
        // `h :- b.`: the style wants a space at the neck; the floor is at most
        // Whitespace, so the space stands unchanged.
        assert_eq!(between("h :- b.\n", 0, "h", ":-", Wish::Space), " ");
    }

    #[test]
    fn a_linebreak_floor_maps_to_whitespace() {
        // No significant adjacency yields LineBreak (a comment/doc/shebang
        // property, §8.2); if a query ever did, `floor_to_doc` maps it to a
        // forced space — never a panic (§12; §5.5's certificate backstop).
        assert_eq!(
            render(&floor_to_doc(Separator::LineBreak, Wish::Abut), 100),
            " "
        );
    }
}
