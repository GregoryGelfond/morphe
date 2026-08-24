//! Comment placement as a property law (docs/design/morphe.md §13; syntax.md
//! §9.2's law, from the consumer side): read through the tier's PUBLIC
//! `attach::attachments`, placement is a function of *structure*, not of the
//! input — every comment survives with its content, and its `(slot, anchor)`
//! follows the program's shape, not the whitespace the author used. This is the
//! slot- and anchor-drift the certificate's linear-sequence check cannot see
//! (§5.2); drop, add, and transposition the certificate itself refuses — the two
//! together are the invariant kallos's coupled reclassification broke. `plan_of`
//! reads a parse's `(slot, content)` facts, `contents_of` its contents alone.
//!
//! The law is framed on the OUTPUT, not the input, so no corpus property is
//! load-bearing: it holds for every member, the one shape the format deliberately
//! re-slots among them — a blank-detached dangling comment between a rule's neck
//! and its body rides with the body, re-attaching as `Leading` (§8.2), converging
//! to a stable slot rather than keeping the input's (§5.4). That edge's own
//! convergence is pinned deterministically by
//! [`a_neck_to_body_dangling_comment_reaches_a_fixed_point`].

use morphe::themelios_syntax::ast::{AstToken, Comment};
use morphe::themelios_syntax::attach::{Slot, attachments};
use morphe::themelios_syntax::equiv::non_whitespace_tokens;
use morphe::themelios_syntax::parse::parse;
use morphe::{Certificate, Dialect, FormatOptions, format};
use morphe_test_support::{cases, source};
use proptest::prelude::*;

/// Comment-bearing members whose comments each fit on one line — so indenting a
/// line (the perturbation below) never rewrites a token's content.
fn corpus_members_with_comments() -> Vec<String> {
    let texts: Vec<String> = cases()
        .into_iter()
        .filter(|case| case.is_member && !case.comment_free)
        .map(|case| case.text)
        .filter(|text| !has_multiline_token(text))
        .collect();
    assert!(!texts.is_empty(), "no single-line-comment member to sample");
    texts
}

fn has_multiline_token(text: &str) -> bool {
    let parsed = parse(&source(text), Dialect::Clingo);
    non_whitespace_tokens(&parsed.syntax()).any(|token| token.text().contains('\n'))
}

/// Perturb whitespace non-trivially while preserving the four facts: indent
/// every non-blank line by two spaces. Line structure — hence `same_line`,
/// `empty_line_between`, and `line_breaks_between` — is untouched, and only
/// leading indentation is added, so no token's content changes (the members are
/// filtered to single-line tokens).
fn respace_preserving_facts(member: &str) -> String {
    let mut out = String::new();
    for line in member.split_inclusive('\n') {
        let (content, newline) = line
            .strip_suffix('\n')
            .map_or((line, ""), |content| (content, "\n"));
        if content.trim().is_empty() {
            out.push_str(line);
        } else {
            out.push_str("  ");
            out.push_str(content);
            out.push_str(newline);
        }
    }
    out
}

/// A parse's comment facts through the tier's public attachment reading: each
/// comment's `(slot, content)`, sorted — a tree-identity-independent summary
/// comparable across two parses (§9.2). The content is the certificate-relevant
/// form morphe emits — a line comment's trailing horizontal whitespace trimmed
/// (§7.3, syntax.md §11.1) — so input and output compare on what is preserved,
/// not on raw token text morphe legitimately reshapes.
fn plan_of(text: &str) -> Vec<(&'static str, String)> {
    let parsed = parse(&source(text), Dialect::Clingo);
    let mut facts: Vec<(&'static str, String)> = attachments(&parsed.syntax())
        .map(|(comment, attachment)| {
            let slot = match attachment.slot {
                Slot::Leading => "leading",
                Slot::Trailing => "trailing",
                Slot::Dangling => "dangling",
            };
            let content = Comment::cast(comment.clone())
                .map_or_else(|| comment.text().to_owned(), |c| c.content().to_owned());
            (slot, content)
        })
        .collect();
    facts.sort();
    facts
}

/// A parse's comment *contents*, sorted — the multiset every placement preserves
/// whatever it does with slots. Each trivia comment appears here once; a drop, a
/// duplicate, or a corrupted body changes it. Derived from [`plan_of`] with the
/// slot dropped, so it is stable across the one shape that re-slots (§8.2).
fn contents_of(text: &str) -> Vec<String> {
    let mut contents: Vec<String> = plan_of(text).into_iter().map(|(_, body)| body).collect();
    contents.sort();
    contents
}

proptest! {
    #[test]
    fn every_comment_survives_at_a_whitespace_independent_slot(
        member in prop::sample::select(corpus_members_with_comments()),
    ) {
        // Read through the tier's public `attachments`, comment placement is a
        // function of structure, not of the input: every comment survives with
        // its content, and its slot follows the program's shape, not the
        // whitespace the author happened to use — the slot/anchor drift the
        // certificate's linear sequence (§5.2) cannot see. Drop, add, and
        // transposition ARE the certificate's to refuse (asserted granted). Read
        // on the OUTPUT, not the input, the law needs no corpus property: it
        // holds for every member, the neck-to-body ride the format deliberately
        // re-slots (§8.2) among them.
        let formatted = format(&source(&member), &FormatOptions::default())
            .expect("a comment-bearing member formats, comments woven");
        prop_assert!(matches!(
            formatted.certificate,
            Certificate::LayoutOnly | Certificate::UpToSpelling
        ));
        // Every comment survives with its content — none dropped, duplicated, or
        // corrupted — whatever the slots.
        prop_assert_eq!(contents_of(&member), contents_of(&formatted.text));
        // And the slots follow structure, not whitespace: a fact-preserving
        // re-spacing formats to the very same facts as the member does.
        let respaced = respace_preserving_facts(&member);
        prop_assume!(respaced != member);
        let reformatted = format(&source(&respaced), &FormatOptions::default())
            .expect("the respaced member formats");
        prop_assert_eq!(plan_of(&formatted.text), plan_of(&reformatted.text));
    }
}

/// The one comment shape whose slot the format changes — a blank-detached
/// dangling comment between a rule's neck and its body — reaches a fixed point:
/// it rides with the body, re-attaching as `Leading` (§8.2), and formatting the
/// result again changes nothing. Idempotence here is convergence to that fixed
/// point, not slot-identity (§5.4); pinned deterministically so the property is
/// not the fuzz target's alone to sample.
#[test]
fn a_neck_to_body_dangling_comment_reaches_a_fixed_point() {
    let member = "a :-\n% note\n\nb.\n";
    let once = format(&source(member), &FormatOptions::default())
        .expect("the member formats, the comment riding with the body");
    let twice = format(&source(&once.text), &FormatOptions::default())
        .expect("the once-formatted member formats again");
    assert_eq!(
        once.text, twice.text,
        "the ride-with-body slot is a fixed point, idempotent by convergence"
    );
    // The comment landed once, `Leading` on the body — the stable slot the
    // convergence reaches, no longer the input's `Dangling` (§8.2).
    let plan = plan_of(&once.text);
    assert_eq!(plan.len(), 1, "the one comment appears once");
    assert_eq!(plan[0].0, "leading", "it re-parses as Leading on the body");
}
