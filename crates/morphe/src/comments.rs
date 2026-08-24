//! The comment placement plan (docs/design/morphe.md §8.1): fold
//! `attachments(root)` once into per-anchor runs — leading, trailing, and (for
//! a parent) dangling — each in source order, so the lowering reads each
//! anchor's comments in O(1) as it reaches it, never the O(m²) of per-comment
//! `attachment` (syntax.md §9.3). morphe does not classify comments; it reads
//! the classification the tier already made (§8).

use std::collections::HashMap;

use themelios_syntax::attach::{Slot, attachments};
use themelios_syntax::tree::{SyntaxElement, SyntaxNode, SyntaxToken};

/// The comments each anchor owns, folded from `attachments` and keyed by the
/// anchor's tree identity (§8.1). Consulted in O(1) as the lowering reaches an
/// anchor; nothing is stored beyond the `(anchor, slot)` runs — the dangling
/// *shape* is derived at emission, not here (§8.2).
pub(crate) struct Plan {
    leading: HashMap<SyntaxElement, Vec<SyntaxToken>>,
    trailing: HashMap<SyntaxElement, Vec<SyntaxToken>>,
    dangling: HashMap<SyntaxElement, Vec<SyntaxToken>>,
}

impl Plan {
    /// The comments leading `anchor`, in source order (§8.2).
    pub(crate) fn leading(&self, anchor: &SyntaxElement) -> &[SyntaxToken] {
        self.leading.get(anchor).map_or(&[], Vec::as_slice)
    }

    /// The comments trailing `anchor`, in source order (§8.2).
    pub(crate) fn trailing(&self, anchor: &SyntaxElement) -> &[SyntaxToken] {
        self.trailing.get(anchor).map_or(&[], Vec::as_slice)
    }

    /// The comments dangling inside `parent`, in source order (§8.2).
    pub(crate) fn dangling(&self, parent: &SyntaxElement) -> &[SyntaxToken] {
        self.dangling.get(parent).map_or(&[], Vec::as_slice)
    }
}

/// Fold `attachments(root)` once into the placement plan (§8.1): each comment
/// bucketed by its anchor and slot, in the source order `attachments` yields.
/// Total; O(tree).
pub(crate) fn plan(root: &SyntaxNode) -> Plan {
    let mut plan = Plan {
        leading: HashMap::new(),
        trailing: HashMap::new(),
        dangling: HashMap::new(),
    };
    for (comment, attachment) in attachments(root) {
        let runs = match attachment.slot {
            Slot::Leading => &mut plan.leading,
            Slot::Trailing => &mut plan.trailing,
            Slot::Dangling => &mut plan.dangling,
        };
        runs.entry(attachment.anchor).or_default().push(comment);
    }
    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    use themelios_syntax::base::source::{Source, SourceId};
    use themelios_syntax::dialect::Dialect;
    use themelios_syntax::parse::parse;

    fn plan_of(text: &str) -> Plan {
        let source = Source::new(SourceId::new(0), text.to_owned()).expect("a small test source");
        plan(&parse(&source, Dialect::Clingo).syntax())
    }

    fn run_texts(runs: &HashMap<SyntaxElement, Vec<SyntaxToken>>) -> Vec<String> {
        let mut out: Vec<String> = runs
            .values()
            .flatten()
            .map(|token| token.text().to_owned())
            .collect();
        out.sort();
        out
    }

    #[test]
    fn a_leading_comment_is_planned_on_its_anchor() {
        let plan = plan_of("% lead\np.\n");
        assert_eq!(run_texts(&plan.leading), vec!["% lead".to_owned()]);
        assert!(plan.trailing.is_empty());
        assert!(plan.dangling.is_empty());
    }

    #[test]
    fn a_trailing_comment_is_planned_on_its_anchor() {
        let plan = plan_of("p. % trail\n");
        assert_eq!(run_texts(&plan.trailing), vec!["% trail".to_owned()]);
        assert!(plan.leading.is_empty());
        assert!(plan.dangling.is_empty());
    }

    #[test]
    fn a_dangling_comment_is_planned_on_its_parent() {
        // On its own line before the closer — a same-line `a % ...` would trail
        // `a` (Slot::Trailing), not dangle (syntax.md §9.2).
        let plan = plan_of("p(\n    a\n    % dangles before the closer\n).\n");
        assert_eq!(
            run_texts(&plan.dangling),
            vec!["% dangles before the closer".to_owned()]
        );
    }

    #[test]
    fn the_plan_is_built_once_over_a_multi_comment_tree() {
        // Every comment lands in exactly one slot; nothing is dropped or doubled.
        let plan = plan_of("% a\np.\nq. % b\nr(\n    c\n    % d\n).\n");
        let total = plan.leading.values().flatten().count()
            + plan.trailing.values().flatten().count()
            + plan.dangling.values().flatten().count();
        assert_eq!(total, 3); // % a leading, % b trailing, % d dangling
    }
}
