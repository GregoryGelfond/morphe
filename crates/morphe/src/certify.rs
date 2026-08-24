//! The certificate gate (docs/design/morphe.md §5.2, §5.3): morphe re-parses
//! its own candidate output and asks the tier whether it stands in the target
//! relation to the input — layout-only, or up-to-spelling. The gate is an
//! internal invariant, always run (§5.3); it decides nothing about spellings
//! or layout, it only *checks*, and an `Err` is always a morphe defect, never
//! the input's fault. It is built and proven before any real lowering exists,
//! so the invariant that guards every later task stands first (§5.2).

use std::collections::HashMap;

use themelios_syntax::ast::Program;
use themelios_syntax::base::source::{Source, SourceId};
use themelios_syntax::dialect::Dialect;
use themelios_syntax::equiv::{
    Certificate, Mismatch, Side, canonical_spelling, equivalent, non_whitespace_tokens,
};
use themelios_syntax::parse::{Parse, parse};
use themelios_syntax::tree::{SyntaxKind, SyntaxToken};

use crate::comments::{Plan, plan};
use crate::options::{BijectionBreach, CertificateBreak, FormatOptions};
use crate::reorder::{Item, units_and_barriers};

/// The source id for the ephemeral candidate re-parse: the candidate is not a
/// file, so its id never enters the certificate — a named zero, not a bare one
/// (§10.4).
const REPARSE_SOURCE_ID: SourceId = SourceId::new(0);

/// The certificate the lowering's canonicalization dictates (§5.3):
/// `LayoutOnly` when spellings are preserved, `UpToSpelling` when they are
/// normalized. The target is never an independent knob — canonicalizing and
/// then asking for `LayoutOnly` would (correctly) be refused. Total; O(1).
pub(crate) fn target(preserve_spellings: bool) -> Certificate {
    if preserve_spellings {
        Certificate::LayoutOnly
    } else {
        Certificate::UpToSpelling
    }
}

/// Re-parse `candidate_text` through the baked-in `parse` door at the input's
/// dialect (§5.6) and check it stands in the `target` relation to `input`
/// (§5.2): the granted certificate, or a [`CertificateBreak`] naming why not — a
/// token `Mismatch` when the candidate is a member whose tokens diverge, or
/// `NotAMember` carrying the tier's diagnostics when morphe's own output does not
/// re-parse (guarded here so that never-should-happen defect reports the syntax
/// error, not a misleading token witness). Total; `Err` is always a morphe defect
/// (§5.3), which the pipeline surfaces as `CertificateBroke` (§10.3). Cost is the
/// certificate's: one parse of the candidate, then `O(|input| + |candidate|)` to
/// compare.
pub(crate) fn certify(
    input: &Parse<Program>,
    candidate_text: &str,
    target: Certificate,
) -> Result<Certificate, CertificateBreak> {
    let candidate = reparse(candidate_text, input.dialect());
    if candidate.has_errors() {
        return Err(CertificateBreak::NotAMember(
            candidate.diagnostics().to_vec(),
        ));
    }
    equivalent(input, &candidate, target)
        .map(|()| target)
        .map_err(CertificateBreak::TokenMismatch)
}

/// Bind the embedded pass to the certificate (§9.3 step 3): re-parse `final_text`
/// and confirm its non-whitespace token sequence is the `skeleton`'s exactly —
/// same kinds, same order — save that a `SCRIPT_BODY` token's content may differ,
/// the one thing the embedded pass changes. A divergence anywhere else means a
/// backend's output disturbed the ASP structure (e.g. re-lexed a script
/// terminator early); the tier's `Mismatch` witness locates it, so the pass
/// reverts that body to verbatim (§9.4). The ASP tokens are compared by raw text:
/// the skeleton and `final_text` differ only inside `SCRIPT_BODY` spans by
/// construction, so exact equality there is necessary and sufficient — a
/// structural check that the splice left the certified skeleton (§5.2) intact,
/// not a re-derivation of it. Total; `O(|skeleton| + |final_text|)`: one parse of
/// `final_text`, then a single zip over two lazy token walks.
pub(crate) fn equal_except_script_bodies(
    skeleton: &Parse<Program>,
    final_text: &str,
) -> Result<(), Mismatch> {
    let candidate = reparse(final_text, skeleton.dialect());
    let skeleton_root = skeleton.syntax();
    let candidate_root = candidate.syntax();
    let mut skeleton_tokens = non_whitespace_tokens(&skeleton_root);
    let mut candidate_tokens = non_whitespace_tokens(&candidate_root);
    let mut index = 0usize;
    loop {
        match (skeleton_tokens.next(), candidate_tokens.next()) {
            (None, None) => return Ok(()),
            (left, right) => {
                let same = match (&left, &right) {
                    (Some(left), Some(right)) => {
                        left.kind() == right.kind()
                            && (left.kind() == SyntaxKind::SCRIPT_BODY
                                || left.text() == right.text())
                    }
                    _ => false,
                };
                if !same {
                    let side = |token: SyntaxToken, parse: &Parse<Program>| Side {
                        kind: token.kind(),
                        content: token.text().to_owned(),
                        location: parse.location(token.text_range()),
                    };
                    return Err(Mismatch {
                        index,
                        left: left.map(|token| side(token, skeleton)),
                        right: right.map(|token| side(token, &candidate)),
                    });
                }
                index += 1;
            }
        }
    }
}

/// Re-parse `text` at `dialect` through the baked-in `parse` door (§5.6) — the
/// certificate's candidate, and the embedded pass's skeleton (§9.3). A `text`
/// past the tier's ceiling (`Source::MAX_LEN`, ~4 GiB) — reachable only for an
/// input already near that ceiling, whose maximally-reflowed candidate crosses
/// it, well outside a leaf client's envelope (§12) — falls back to the empty
/// source, which the certificate then reports as a divergence, so the gate
/// refuses rather than passing an unverified candidate. The empty fallback
/// always admits (length 0 ≤ `Source::MAX_LEN`), so this is total.
pub(crate) fn reparse(text: &str, dialect: Dialect) -> Parse<Program> {
    let source = Source::new(REPARSE_SOURCE_ID, text.to_owned())
        .or_else(|_| Source::new(REPARSE_SOURCE_ID, String::new()))
        .expect("the empty fallback source is always within Source::MAX_LEN");
    parse(&source, dialect)
}

/// Prove `output_text` is a pure permutation of `input` — the permutation
/// certificate of §15.3, the reorder's own guarantee one layer above the layout
/// certificate (which correctly refuses a reordered whole program). It is a
/// *structural* check, never a second `equivalent` run: re-parse the output,
/// re-derive its top-level units and barriers, and verify that within each region
/// the barriers divide, the output's units are the input's exactly — a both-ways
/// multiset count, so nothing added or dropped (a split or duplicated unit
/// surfaces as an add and a drop) — and that every barrier holds its place and
/// neighbours. Token identity is read under `options.preserve_spellings` —
/// the same relation each unit's own certificate earned — a comment by its
/// content (syntax.md §11.1). The per-unit layout proof is discharged separately,
/// by each unit's `format`; this proves only the permutation.
///
/// # Errors
///
/// A [`BijectionBreach`] naming the first structural break — always a morphe bug
/// (§15.3), which the reorder surfaces as `ReorderError::PermutationBroke` and the
/// CLI as `Outcome::InternalError` (§10.3).
pub(crate) fn permutation_certificate(
    input: &Parse<Program>,
    output_text: &str,
    options: &FormatOptions,
) -> Result<(), BijectionBreach> {
    let output = reparse(output_text, input.dialect());
    if output.has_errors() {
        // A reassembly that does not re-parse as a member is a bijection breach
        // (§15.3): morphe emitted invalid ASP; the tier's diagnostics locate it,
        // where a unit index would mislead. Unreachable for a concatenation of
        // member units under §7 spacing.
        return Err(BijectionBreach::NotAMember(output.diagnostics().to_vec()));
    }
    let relation = target(options.preserve_spellings);
    let input_plan = plan(&input.syntax());
    let output_plan = plan(&output.syntax());
    let input_items = units_and_barriers(&input.tree(), &input_plan, options.reorder_includes);
    let output_items = units_and_barriers(&output.tree(), &output_plan, options.reorder_includes);

    let input_regions = regions(&input_items);
    let output_regions = regions(&output_items);

    // Every barrier holds — the same barriers, in the same order (§15.3).
    if let Some(at) = barrier_moved(
        &input_regions,
        &input_plan,
        &output_regions,
        &output_plan,
        relation,
    ) {
        return Err(BijectionBreach::BarrierMoved { at });
    }

    // Within each region, the output's units are the input's exactly — a
    // block-permutation covering every unit once (§15.3).
    for (input_units, output_units) in input_regions.units.iter().zip(&output_regions.units) {
        if let Some(breach) = region_breach(
            input_units,
            &input_plan,
            output_units,
            &output_plan,
            relation,
        ) {
            return Err(breach);
        }
    }
    Ok(())
}

/// The top level split into the regions its barriers divide (§15.3): the units of
/// each region, and the barriers between them. `units` holds one more entry than
/// `barriers` — before the first barrier, between each pair, and after the last
/// (any may be empty).
struct Regions<'a> {
    units: Vec<Vec<&'a Item>>,
    barriers: Vec<&'a Item>,
}

fn regions(items: &[Item]) -> Regions<'_> {
    let mut units: Vec<Vec<&Item>> = Vec::new();
    let mut barriers = Vec::new();
    let mut current: Vec<&Item> = Vec::new();
    for item in items {
        if matches!(item, Item::Barrier(_)) {
            units.push(std::mem::take(&mut current));
            barriers.push(item);
        } else {
            current.push(item);
        }
    }
    units.push(current);
    Regions { units, barriers }
}

/// The index of the first barrier that did not hold — a differing count, or a
/// barrier whose identity changed (§15.3) — else `None`.
fn barrier_moved(
    input: &Regions,
    input_plan: &Plan,
    output: &Regions,
    output_plan: &Plan,
    relation: Certificate,
) -> Option<usize> {
    if input.barriers.len() != output.barriers.len() {
        return Some(input.barriers.len().min(output.barriers.len()));
    }
    input
        .barriers
        .iter()
        .zip(&output.barriers)
        .position(|(left, right)| {
            signature(left, input_plan, relation) != signature(right, output_plan, relation)
        })
}

/// The first way one region's units differ from the other's as a multiset of
/// unit identities (§15.3): an input unit with no output match is dropped, an
/// output unit with no input match added — a split or duplication surfaces as
/// one of these — else `None`.
fn region_breach(
    input_units: &[&Item],
    input_plan: &Plan,
    output_units: &[&Item],
    output_plan: &Plan,
    relation: Certificate,
) -> Option<BijectionBreach> {
    let input_signatures: Vec<Signature> = input_units
        .iter()
        .map(|item| signature(item, input_plan, relation))
        .collect();
    let output_signatures: Vec<Signature> = output_units
        .iter()
        .map(|item| signature(item, output_plan, relation))
        .collect();

    let mut balance: HashMap<&Signature, i64> = HashMap::new();
    for signature in &input_signatures {
        *balance.entry(signature).or_default() += 1;
    }
    for signature in &output_signatures {
        *balance.entry(signature).or_default() -= 1;
    }
    if balance.values().all(|count| *count == 0) {
        return None;
    }
    if let Some(at) = input_signatures
        .iter()
        .position(|signature| balance[signature] > 0)
    {
        return Some(BijectionBreach::UnitDropped { at });
    }
    let at = output_signatures
        .iter()
        .position(|signature| balance[signature] < 0)
        .unwrap_or(0);
    Some(BijectionBreach::UnitAdded { at })
}

/// A unit's or barrier's identity for the bijection (§15.3): its non-whitespace
/// token-and-comment subsequence, each token read under `relation` — respelled to
/// canonical under `UpToSpelling`, verbatim under `LayoutOnly` — a comment by its
/// content, its trailing horizontal whitespace trimmed (syntax.md §11.1).
type Signature = Vec<(SyntaxKind, String)>;

fn signature(item: &Item, plan: &Plan, relation: Certificate) -> Signature {
    item.subsequence(plan)
        .iter()
        .map(|token| {
            let content = content_of(token);
            let spelling = match relation {
                Certificate::LayoutOnly => content,
                Certificate::UpToSpelling => {
                    canonical_spelling(token.kind(), &content).into_owned()
                }
            };
            (token.kind(), spelling)
        })
        .collect()
}

/// A token's content for comparison (syntax.md §11.1): its text, with a comment's
/// trailing horizontal whitespace trimmed — the one thing about a comment that is
/// layout, not content.
fn content_of(token: &SyntaxToken) -> String {
    if token.kind().is_comment() {
        token.text().trim_end().to_owned()
    } else {
        token.text().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A test-only member helper (source construction lives in the CLI, §10.1).
    fn member(text: &str) -> Parse<Program> {
        reparse(text, Dialect::Clingo)
    }

    #[test]
    fn a_layout_only_respacing_is_granted() {
        let input = member("p(X):-q(X).\n");
        let got = certify(&input, "p(X) :- q(X).\n", Certificate::LayoutOnly);
        assert!(matches!(got, Ok(Certificate::LayoutOnly)));
    }

    #[test]
    fn a_divergent_member_candidate_breaks_as_a_token_mismatch() {
        // A candidate that re-parses cleanly but whose tokens diverge is the
        // token-mismatch class, carrying the tier's first-divergence witness.
        let input = member("p.\n");
        assert!(matches!(
            certify(&input, "q.\n", Certificate::LayoutOnly),
            Err(CertificateBreak::TokenMismatch(_))
        ));
    }

    #[test]
    fn a_non_member_candidate_breaks_as_not_a_member() {
        // morphe never emits invalid ASP for a member; were it to, the break
        // names it and carries the tier's own syntax diagnostics — not a
        // misleading token witness (§5.3, §10.3).
        let input = member("p.\n");
        match certify(&input, "p(\n", Certificate::LayoutOnly) {
            Err(CertificateBreak::NotAMember(diagnostics)) => {
                assert!(
                    !diagnostics.is_empty(),
                    "the tier's diagnostics are carried"
                );
            }
            other => panic!("expected NotAMember, got {other:?}"),
        }
    }

    #[test]
    fn target_is_up_to_spelling_by_default_and_layout_only_when_preserving() {
        // The target is dictated by whether the lowering canonicalizes (§5.3).
        assert_eq!(target(false), Certificate::UpToSpelling); // default canonicalizes
        assert_eq!(target(true), Certificate::LayoutOnly); // preserve_spellings
    }

    #[test]
    fn a_changed_significant_token_is_refused() {
        let input = member("p.\n");
        assert!(certify(&input, "q.\n", Certificate::LayoutOnly).is_err());
    }

    #[test]
    fn a_comment_moved_across_a_token_is_refused() {
        // kallos's transposition: the comment jumps the token it led.
        let input = member("% c\np.\n");
        let moved = "p. % c\n";
        assert!(certify(&input, moved, Certificate::LayoutOnly).is_err());
    }

    #[test]
    fn identical_text_passes_the_equal_except_script_bodies_check() {
        let skeleton = member("#script(python)\nx = 1\n#end.\np.\n");
        assert_eq!(
            equal_except_script_bodies(&skeleton, "#script(python)\nx = 1\n#end.\np.\n"),
            Ok(())
        );
    }

    #[test]
    fn a_script_bodys_content_is_the_one_thing_allowed_to_differ() {
        // The one exemption (§9.3 step 3): a SCRIPT_BODY's content may change,
        // the ASP skeleton around it may not.
        let skeleton = member("#script(python)\nx = 1\n#end.\np.\n");
        assert_eq!(
            equal_except_script_bodies(&skeleton, "#script(python)\ny = 2\n#end.\np.\n"),
            Ok(())
        );
    }

    #[test]
    fn a_changed_asp_token_fails_the_equal_except_script_bodies_check() {
        // The fact after the script changed p → q: outside a SCRIPT_BODY, that
        // is a divergence the check must catch (§9.3 step 3).
        let skeleton = member("#script(python)\nx = 1\n#end.\np.\n");
        assert!(
            equal_except_script_bodies(&skeleton, "#script(python)\nx = 1\n#end.\nq.\n").is_err()
        );
    }

    #[test]
    fn a_body_that_re_lexes_a_terminator_early_fails_the_check() {
        // A spliced body smuggling `#end` splits the script region early, so ASP
        // tokens appear where the skeleton had a single opaque body (§9.3 step 3).
        let skeleton = member("#script(python)\nx = 1\n#end.\np.\n");
        assert!(
            equal_except_script_bodies(&skeleton, "#script(python)\nx\n#end.\nq.\n#end.\np.\n")
                .is_err()
        );
    }

    #[test]
    fn the_permutation_certificate_holds_for_a_pure_permutation() {
        // #defined moved before #show, each formatted — a pure permutation (§15.3).
        let input = member("#show s/1.\n#defined d/1.\n");
        assert!(
            permutation_certificate(
                &input,
                "#defined d/1.\n#show s/1.\n",
                &FormatOptions::default()
            )
            .is_ok()
        );
    }

    #[test]
    fn a_dropped_unit_breaks_the_permutation_certificate() {
        // #show is gone from the reassembly — a unit dropped (§15.3).
        let input = member("#defined d/1.\n#show s/1.\n");
        assert!(matches!(
            permutation_certificate(&input, "#defined d/1.\n", &FormatOptions::default()),
            Err(BijectionBreach::UnitDropped { .. })
        ));
    }

    #[test]
    fn an_added_unit_breaks_the_permutation_certificate() {
        // #show appears in the reassembly but not the input — a unit added (§15.3).
        let input = member("#defined d/1.\n");
        assert!(matches!(
            permutation_certificate(
                &input,
                "#defined d/1.\n#show s/1.\n",
                &FormatOptions::default()
            ),
            Err(BijectionBreach::UnitAdded { .. })
        ));
    }

    #[test]
    fn a_moved_barrier_breaks_the_permutation_certificate() {
        // Two #program barriers swapped — a barrier must hold its place (§15.3).
        let input = member("#program a.\n#program b.\n");
        assert!(matches!(
            permutation_certificate(
                &input,
                "#program b.\n#program a.\n",
                &FormatOptions::default()
            ),
            Err(BijectionBreach::BarrierMoved { .. })
        ));
    }

    #[test]
    fn a_non_member_reassembly_breaks_as_not_a_member() {
        // A reassembly that does not re-parse as a member carries the tier's
        // diagnostics, not a misleading unit index (§15.3).
        let input = member("p.\n");
        match permutation_certificate(&input, "p(\n", &FormatOptions::default()) {
            Err(BijectionBreach::NotAMember(diagnostics)) => {
                assert!(
                    !diagnostics.is_empty(),
                    "the tier's diagnostics are carried"
                );
            }
            other => panic!("expected NotAMember, got {other:?}"),
        }
    }
}
