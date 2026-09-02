//! The sound reorder mechanism (docs/design/morphe.md §15): the opt-in,
//! order-changing transformation that organizes a program's signature the way
//! `rustfmt` organizes imports, but conservatively (§15.5). This module builds
//! the *mechanism* — pure text rearrangement, the tier's tree never mutated
//! (§15.2). A top-level statement with the trivia the tier binds to it is a
//! **unit** (§15.1); an order-sensitive statement or a program-level Dangling
//! comment is a **barrier** the reorder never moves and never permutes a unit
//! across (§15.1, §15.4). `units_and_barriers` reads the top level into that
//! item stream, and `bucket_emit` is the stable bucket-sort that, within each
//! region the barriers divide, emits each `Section` of the active `ORDER` in
//! turn with its units in source order (§15.2).

use themelios_syntax::ast::{ConstPolicy, Program, Statement};
use themelios_syntax::base::diagnostic::{Diagnostic, DiagnosticId, Label, Severity};
use themelios_syntax::base::source::{Source, SourceId};
use themelios_syntax::equiv::{Certificate, non_whitespace_tokens};
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::{AstNode, SyntaxElement, SyntaxToken, TextRange};

use crate::comments::Plan;
use crate::options::{FormatError, FormatOptions};
use crate::{Formatted, format};

/// The source id for the ephemeral one-statement slice each unit is formatted as
/// (§15.3): the slice is not a file, so its id never surfaces — a named zero, not
/// a bare one (§10.4), as `certify`'s `REPARSE_SOURCE_ID` is for its candidate.
const UNIT_SOURCE_ID: SourceId = SourceId::new(0);

/// A bucket of the reorder's `ORDER` (§15.2): the stable bucket-sort emits the
/// buckets in ORDER's sequence, each bucket's units in source order. `section`
/// maps every reorderable statement to one; the codomain law (§13) binds its
/// codomain to the sections the active ORDER holds, so no unit is dropped.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub(crate) enum Section {
    /// `#include`, hoisted to the front under `reorder_includes` (§15.4).
    Include,
    /// `#defined` — the input signature (§15.2).
    Defined,
    /// A plain (non-override) `#const` — the parameters (§15.2, §15.4).
    Const,
    /// Rules and every other order-insensitive statement, in source order
    /// (§15.2).
    Body,
    /// `#show` — the output signature (§15.2).
    Show,
}

/// The default section order (§15.2): `#defined` → `#const` → Body → `#show`.
/// `#include` is a barrier (§15.4), so it has no default bucket.
pub(crate) const ORDER: &[Section] = &[
    Section::Defined,
    Section::Const,
    Section::Body,
    Section::Show,
];

/// The section order under `reorder_includes` (§15.4): `#include` is hoisted to
/// a leading `Include` bucket ahead of the default order.
pub(crate) const ORDER_WITH_INCLUDES: &[Section] = &[
    Section::Include,
    Section::Defined,
    Section::Const,
    Section::Body,
    Section::Show,
];

/// The bucket a reorderable statement emits into (§15.2), or `None` for the
/// inherently order-sensitive kinds that are always barriers and never
/// bucketed — `#program`, `#script`, and the query (§15.4). `Body` is the
/// catch-all for rules and every other order-insensitive statement. The match
/// is exhaustive over the tier's statement kinds, so a new kind is a compile
/// error here rather than a silently mis-bucketed unit (§15.1's checkable
/// completeness).
pub(crate) fn section(statement: &Statement) -> Option<Section> {
    match statement {
        Statement::Defined(_) => Some(Section::Defined),
        Statement::Const(_) => Some(Section::Const),
        Statement::Show(_) => Some(Section::Show),
        Statement::Include(_) => Some(Section::Include),
        Statement::Rule(_)
        | Statement::WeakConstraint(_)
        | Statement::Optimize(_)
        | Statement::Project(_)
        | Statement::Edge(_)
        | Statement::Heuristic(_)
        | Statement::External(_)
        | Statement::TheoryDefinition(_) => Some(Section::Body),
        Statement::ProgramPart(_) | Statement::Script(_) | Statement::Query(_) => None,
    }
}

/// Whether `statement` is a barrier — a top-level element the reorder holds in
/// place and never permutes a unit across (§15.1). The classification is
/// syntactic (§15.4): `#program`, `#script`, and the query are always
/// order-sensitive; `#include` is a barrier unless `reorder_includes` downgrades
/// it to a hoistable unit; an override `#const` resolves by position, while a
/// plain or `[default]` one is a global. Every other kind is reorderable. The
/// match is exhaustive, so a new statement kind is a compile error here rather
/// than silently reorderable (§15.1).
pub(crate) fn is_barrier(statement: &Statement, reorder_includes: bool) -> bool {
    match statement {
        Statement::ProgramPart(_) | Statement::Script(_) | Statement::Query(_) => true,
        Statement::Include(_) => !reorder_includes,
        Statement::Const(constant) => constant.policy() == Some(ConstPolicy::Override),
        Statement::Rule(_)
        | Statement::WeakConstraint(_)
        | Statement::Optimize(_)
        | Statement::Show(_)
        | Statement::Project(_)
        | Statement::Defined(_)
        | Statement::Edge(_)
        | Statement::Heuristic(_)
        | Statement::External(_)
        | Statement::TheoryDefinition(_) => false,
    }
}

/// A top-level element in the reorder's item stream (§15.1): a reorderable unit,
/// or a held barrier.
pub(crate) enum Item {
    /// A reorderable statement, tagged with the bucket it emits into (§15.2).
    Unit(Unit),
    /// A held element the reorder never moves and never crosses (§15.1, §15.4).
    Barrier(Barrier),
}

/// A reorderable unit (§15.1): a top-level statement with the trivia the tier
/// binds to it. Its `%!` documentation is structural — leading `DOC_COMMENT`
/// children of the statement node (syntax.md §8) — so it rides inside
/// `statement`'s own text; `leading` and `trailing` are the surrounding trivia
/// comments the tier attaches to it (§8), carried so that moving the statement
/// carries its comments (§15.1).
pub(crate) struct Unit {
    section: Section,
    statement: Statement,
    leading: Vec<SyntaxToken>,
    trailing: Vec<SyntaxToken>,
}

/// A held top-level element (§15.1, §15.4). A barrier does not move, so — unlike
/// a unit — it carries no bundled trivia: its comments stay in place with it.
pub(crate) enum Barrier {
    /// An order-sensitive statement, held in source position (§15.4).
    Statement { statement: Statement },
    /// A program-level Dangling comment, floating with no owner (§15.1).
    Dangling(SyntaxToken),
}

impl Item {
    /// The bucket of a unit, or `None` for a barrier (§15.2).
    fn section(&self) -> Option<Section> {
        match self {
            Item::Unit(unit) => Some(unit.section),
            Item::Barrier(_) => None,
        }
    }

    /// Whether this item is a barrier that bounds a region (§15.1).
    fn is_barrier(&self) -> bool {
        matches!(self, Item::Barrier(_))
    }

    /// The item's start offset in the source — the statement's, or a Dangling
    /// barrier's comment's. Reordering compares the offset sequence before and
    /// after `bucket_emit` to learn whether any unit actually moved (§15.3).
    pub(crate) fn source_offset(&self) -> u32 {
        let range = match self {
            Item::Unit(unit) => unit.statement.syntax().text_range(),
            Item::Barrier(Barrier::Statement { statement }) => statement.syntax().text_range(),
            Item::Barrier(Barrier::Dangling(comment)) => comment.text_range(),
        };
        u32::from(range.start())
    }

    /// The item's non-whitespace token-and-comment subsequence in source order
    /// (§15.3): a statement's leading trivia, its own significant tokens and
    /// `%!` docs, then its trailing trivia; a Dangling barrier's single comment.
    /// The permutation certificate matches units by the identity of this
    /// subsequence, so a unit's comments are part of what must travel with it
    /// (§15.1). A unit carries its trivia; a held barrier's is read from `plan`.
    pub(crate) fn subsequence(&self, plan: &Plan) -> Vec<SyntaxToken> {
        match self {
            Item::Unit(unit) => {
                statement_subsequence(&unit.statement, &unit.leading, &unit.trailing)
            }
            Item::Barrier(Barrier::Statement { statement }) => {
                let element = SyntaxElement::Node(statement.syntax().clone());
                statement_subsequence(statement, plan.leading(&element), plan.trailing(&element))
            }
            Item::Barrier(Barrier::Dangling(comment)) => vec![comment.clone()],
        }
    }
}

/// The source ranges of the `#include` units `reorder_includes` made movable —
/// the statements whose section is [`Section::Include`] (§15.4). Empty unless
/// `reorder_includes` is set, since `#include` is otherwise a barrier, not a
/// unit. `reorder` reads these before the emit to learn whether an include was
/// actually hoisted (its position moved), and where to point the note if so.
pub(crate) fn include_unit_ranges(items: &[Item]) -> Vec<TextRange> {
    items
        .iter()
        .filter_map(|item| match item {
            Item::Unit(unit) if matches!(unit.section, Section::Include) => {
                Some(unit.statement.syntax().text_range())
            }
            _ => None,
        })
        .collect()
}

/// The warning that `reorder_includes` hoisted `count` `#include`(s) past other
/// statements (§15.4): the one reorder that can change answer sets, resting on
/// the caller's assertion that the includes are pure — which morphe cannot
/// verify, never resolving them — so the hoist is surfaced, never silent (the
/// config-discovery posture, threat model §3.4). `at` locates the first hoisted
/// include in the caller's own source (§11).
pub(crate) fn include_hoist_note(
    input: &Parse<Program>,
    at: TextRange,
    count: usize,
) -> Diagnostic {
    let message = if count == 1 {
        "reorder_includes moved an `#include` past other statements; morphe does \
         not resolve includes, so this rests on your assertion that they are \
         order-independent and is not certified meaning-preserving"
            .to_owned()
    } else {
        format!(
            "reorder_includes moved {count} `#include` directives past other \
             statements; morphe does not resolve includes, so this rests on your \
             assertion that they are order-independent and is not certified \
             meaning-preserving"
        )
    };
    Diagnostic::new(
        DiagnosticId::new("morphe", "reorder-includes"),
        Severity::Warning,
        message,
        Label {
            location: input.location(at),
            message: None,
        },
    )
    .expect("the reorder-includes note message is never empty")
}

/// A statement's non-whitespace subsequence: its leading trivia, its own
/// significant tokens and structural `%!` docs (`non_whitespace_tokens` descends
/// the node), then its trailing trivia (§15.3).
fn statement_subsequence(
    statement: &Statement,
    leading: &[SyntaxToken],
    trailing: &[SyntaxToken],
) -> Vec<SyntaxToken> {
    leading
        .iter()
        .cloned()
        .chain(non_whitespace_tokens(statement.syntax()))
        .chain(trailing.iter().cloned())
        .collect()
}

/// Read the program's top level into the reorder's item stream, in source order
/// (§15.1): a `Unit` per reorderable statement carrying the Leading and Trailing
/// trivia the `Plan` attaches to it (§8), a `Barrier::Statement` per
/// order-sensitive statement (§15.4), and a `Barrier::Dangling` per
/// program-level Dangling comment (§15.1). A statement becomes a unit only when
/// it has a section and is not a barrier; anything else is held, so an
/// unclassifiable statement fails safe as a barrier rather than being moved.
pub(crate) fn units_and_barriers(
    program: &Program,
    plan: &Plan,
    reorder_includes: bool,
) -> Vec<Item> {
    let program_element = SyntaxElement::Node(program.syntax().clone());
    let mut ordered: Vec<(u32, Item)> = Vec::new();

    for statement in program.statements() {
        let offset = u32::from(statement.syntax().text_range().start());
        let item = match section(&statement) {
            Some(section) if !is_barrier(&statement, reorder_includes) => {
                let element = SyntaxElement::Node(statement.syntax().clone());
                Item::Unit(Unit {
                    section,
                    leading: plan.leading(&element).to_vec(),
                    trailing: plan.trailing(&element).to_vec(),
                    statement,
                })
            }
            _ => Item::Barrier(Barrier::Statement { statement }),
        };
        ordered.push((offset, item));
    }

    for comment in plan.dangling(&program_element) {
        let offset = u32::from(comment.text_range().start());
        ordered.push((offset, Item::Barrier(Barrier::Dangling(comment.clone()))));
    }

    ordered.sort_by_key(|(offset, _)| *offset);
    ordered.into_iter().map(|(_, item)| item).collect()
}

/// The stable bucket-sort of §15.2: split the item stream on barriers and,
/// within each region, emit each `Section` of the active `order` in turn with
/// its units in source order — a bucket-sort driven by the `order` data, not a
/// comparator. Every barrier is woven back at its held position, and the tree is
/// never touched — the reorder works in text (§15.2).
pub(crate) fn bucket_emit(items: Vec<Item>, order: &[Section]) -> Vec<Item> {
    let mut out = Vec::with_capacity(items.len());
    let mut region: Vec<Item> = Vec::new();
    for item in items {
        if item.is_barrier() {
            drain_region(&mut region, order, &mut out);
            out.push(item);
        } else {
            region.push(item);
        }
    }
    drain_region(&mut region, order, &mut out);
    out
}

/// Emit `region`'s units into `out` in the active `order`'s section sequence,
/// each section's units kept in source order (a stable distribution, §15.2).
/// Total: every unit is emitted exactly once. The trailing `order.len()` bucket
/// is the totality guard — it catches any unit whose section is absent from
/// `order`, a state the codomain law (§13) forbids for the active ORDER, and
/// appends it after the ordered sections rather than dropping it, so the emit
/// never loses a unit even were the law violated.
fn drain_region(region: &mut Vec<Item>, order: &[Section], out: &mut Vec<Item>) {
    let mut buckets: Vec<Vec<Item>> = std::iter::repeat_with(Vec::new)
        .take(order.len() + 1)
        .collect();
    for item in region.drain(..) {
        let rank = item
            .section()
            .and_then(|section| order.iter().position(|slot| *slot == section))
            .unwrap_or(order.len());
        buckets[rank].push(item);
    }
    for bucket in buckets {
        out.extend(bucket);
    }
}

/// The reassembled reorder output: the concatenated per-unit texts, the
/// weakest-wins certificate over them, and the aggregated embedded notes.
pub(crate) struct Assembled {
    /// The formatted, reordered program text.
    pub(crate) text: String,
    /// The uniform claim class every unit earned — `UpToSpelling` if any unit
    /// canonicalized, else `LayoutOnly` (weakest wins, §4.2, §15.3).
    pub(crate) certificate: Certificate,
    /// Per-unit embedded outcomes and other warnings (§9.4).
    pub(crate) notes: Vec<Diagnostic>,
}

/// Render each item to certified text and concatenate them in the reordered
/// order under §7.1's one-statement-per-line shape — the `assemble(house_spacing)`
/// of §15.2, run after the reorder (§15.2's `map(render_unit)`). Each unit and
/// each held barrier statement is formatted as its own one-statement program
/// sliced from `source` (§15.3): its always-on certificate is that unit's proof,
/// and a per-unit break surfaces as a [`FormatError`] the caller carries out as
/// `PermutationBroke`. A Dangling barrier renders its comment verbatim (§7.3).
/// The tree is never mutated — the reorder works in text (§15.2).
///
/// # Errors
///
/// A per-unit [`FormatError::CertificateBroke`] — a morphe bug (§5.3), never
/// reached for a member unit.
pub(crate) fn assemble(
    items: &[Item],
    source: &str,
    plan: &Plan,
    options: &FormatOptions,
) -> Result<Assembled, FormatError> {
    let mut text = String::new();
    let mut certificate = Certificate::LayoutOnly;
    let mut notes = Vec::new();
    for item in items {
        match item {
            Item::Unit(unit) => {
                let piece = render_statement(
                    &unit.statement,
                    &unit.leading,
                    &unit.trailing,
                    source,
                    options,
                )?;
                certificate = weaker(certificate, piece.certificate);
                notes.extend(piece.notes);
                text.push_str(&piece.text);
            }
            Item::Barrier(Barrier::Statement { statement }) => {
                let element = SyntaxElement::Node(statement.syntax().clone());
                let piece = render_statement(
                    statement,
                    plan.leading(&element),
                    plan.trailing(&element),
                    source,
                    options,
                )?;
                certificate = weaker(certificate, piece.certificate);
                notes.extend(piece.notes);
                text.push_str(&piece.text);
            }
            Item::Barrier(Barrier::Dangling(comment)) => {
                // A blank line before and after keeps the comment floating —
                // re-parsing to `Dangling(PROGRAM)`, not re-attaching as the
                // Leading or Trailing of a now-adjacent unit (§15.3's barrier
                // hold, syntax.md §9.2's four-facts hazard). Its trailing
                // horizontal whitespace is layout, trimmed (syntax.md §11.1).
                push_dangling(&mut text, comment.text().trim_end());
            }
        }
    }
    // A single trailing newline terminates the file; a Dangling barrier's blank
    // never doubles it (§7.1).
    while text.ends_with("\n\n") {
        text.pop();
    }
    Ok(Assembled {
        text,
        certificate,
        notes,
    })
}

/// Emit a program-level Dangling comment with a blank line on each side (§15.3):
/// a blank before (unless the output is empty or already blank-terminated) and a
/// blank after, so the re-parse keeps it Dangling rather than re-attaching it.
fn push_dangling(text: &mut String, comment: &str) {
    if !text.is_empty() && !text.ends_with("\n\n") {
        text.push('\n');
    }
    text.push_str(comment);
    text.push_str("\n\n");
}

/// Format one top-level statement, with the leading and trailing trivia the tier
/// binds to it, as a one-statement program (§15.3): the source slice spanning the
/// unit's first token through its last is itself a member, and `format`'s
/// always-on certificate proves that unit's layout. The slice is real source, so
/// its `%!` documentation and comments are carried exactly (§15.1, §7.3).
fn render_statement(
    statement: &Statement,
    leading: &[SyntaxToken],
    trailing: &[SyntaxToken],
    source: &str,
    options: &FormatOptions,
) -> Result<Formatted, FormatError> {
    let node = statement.syntax().text_range();
    let start = leading
        .first()
        .map_or(node.start(), |token| token.text_range().start());
    let end = trailing
        .last()
        .map_or(node.end(), |token| token.text_range().end());
    let slice = &source[u32::from(start) as usize..u32::from(end) as usize];
    let unit = Source::new(UNIT_SOURCE_ID, slice.to_owned())
        .expect("a one-statement slice of a member is within Source::MAX_LEN");
    format(&unit, options)
}

/// The weaker of two certificates (§4.2): `UpToSpelling` (a token may be
/// respelled) is weaker than `LayoutOnly` (only layout differs), so any
/// canonicalized unit weakens the whole reorder's claim.
fn weaker(left: Certificate, right: Certificate) -> Certificate {
    match (left, right) {
        (Certificate::UpToSpelling, _) | (_, Certificate::UpToSpelling) => {
            Certificate::UpToSpelling
        }
        (Certificate::LayoutOnly, Certificate::LayoutOnly) => Certificate::LayoutOnly,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::collections::HashSet;

    use themelios_syntax::base::source::{Source, SourceId};
    use themelios_syntax::dialect::Dialect;
    use themelios_syntax::parse::parse;

    fn source(text: &str) -> Source {
        Source::new(SourceId::new(0), text.to_owned()).expect("a small test source")
    }

    fn program_of(text: &str, dialect: Dialect) -> Program {
        let parsed = parse(&source(text), dialect);
        assert!(
            !parsed.has_errors(),
            "the test source parses cleanly: {:?}",
            parsed.diagnostics()
        );
        parsed.tree()
    }

    /// The first statement of `text` — for classifying a single kind.
    fn first(text: &str, dialect: Dialect) -> Statement {
        program_of(text, dialect)
            .statements()
            .next()
            .expect("a statement")
    }

    /// The item stream `units_and_barriers` reads from `text` (§15.1).
    fn items_of(text: &str, dialect: Dialect, reorder_includes: bool) -> Vec<Item> {
        let parsed = parse(&source(text), dialect);
        assert!(
            !parsed.has_errors(),
            "the test source parses cleanly: {:?}",
            parsed.diagnostics()
        );
        let program = parsed.tree();
        let plan = crate::comments::plan(&parsed.syntax());
        units_and_barriers(&program, &plan, reorder_includes)
    }

    /// Each item's identifying source text — a held statement or unit by its
    /// statement's text, a Dangling barrier by its comment's text.
    fn tags(items: &[Item]) -> Vec<String> {
        items
            .iter()
            .map(|item| match item {
                Item::Unit(unit) => unit.statement.syntax().text().to_string().trim().to_owned(),
                Item::Barrier(Barrier::Statement { statement }) => {
                    statement.syntax().text().to_string().trim().to_owned()
                }
                Item::Barrier(Barrier::Dangling(comment)) => comment.text().trim().to_owned(),
            })
            .collect()
    }

    #[test]
    fn is_barrier_holds_exactly_for_the_order_sensitive_kinds() {
        // #program, #script, override #const, #include (unless reorder_includes),
        // and the ASP-Core-2 query are order-sensitive barriers; a plain (or
        // [default]) #const, #defined, #show, and a rule are reorderable (§15.4).
        assert!(is_barrier(&first("#program base.", Dialect::Clingo), false));
        assert!(is_barrier(
            &first("#script (lua) x #end.", Dialect::Clingo),
            false
        ));
        assert!(is_barrier(
            &first("#const n = 1. [override]", Dialect::Clingo),
            false
        ));
        assert!(is_barrier(
            &first("#include \"f\".", Dialect::Clingo),
            false
        ));
        assert!(is_barrier(&first("p(1)?", Dialect::AspCore2), false));

        assert!(!is_barrier(&first("#const n = 1.", Dialect::Clingo), false));
        assert!(!is_barrier(
            &first("#const n = 1. [default]", Dialect::Clingo),
            false
        ));
        assert!(!is_barrier(&first("#defined p/1.", Dialect::Clingo), false));
        assert!(!is_barrier(&first("#show p/1.", Dialect::Clingo), false));
        assert!(!is_barrier(&first("p :- q.", Dialect::Clingo), false));

        // The inherently order-sensitive kinds have no reorder bucket (§15.4).
        assert_eq!(section(&first("#program base.", Dialect::Clingo)), None);
        assert_eq!(
            section(&first("#script (lua) x #end.", Dialect::Clingo)),
            None
        );
        assert_eq!(section(&first("p(1)?", Dialect::AspCore2)), None);
    }

    #[test]
    fn a_top_level_dangling_comment_is_a_barrier_dividing_the_stream() {
        // A program-level Dangling comment is a barrier that divides the unit
        // stream (§15.1): the two rules fall on either side of it.
        let items = items_of("p.\n\n% floating\n\nq.\n", Dialect::Clingo, false);
        assert!(matches!(
            items.as_slice(),
            [
                Item::Unit(_),
                Item::Barrier(Barrier::Dangling(_)),
                Item::Unit(_),
            ]
        ));
    }

    #[test]
    fn section_maps_every_reorderable_kind_into_order_and_nothing_outside() {
        // The tier's one-of-every-family program (themelios-syntax ast tests), in
        // source order — every reorderable kind present.
        let text = "p. :~ q. [1@2, x] #minimize { 1 : p }. #show p/1. #project p. #defined p/1. #edge (a,b). #heuristic a. [1,sign] #external p. [true] #const n = 1. [default] #script (lua) x #end. #include \"f\". #program base. #theory t { }.";
        let program = program_of(text, Dialect::Clingo);

        // The codomain law (§13): the codomain of `section` over the reorderable
        // kinds is exactly the set of sections ORDER holds — nothing maps outside,
        // every bucket is reachable, so bucket_emit drops no unit (§15.2).
        let mut hit: HashSet<Section> = HashSet::new();
        for statement in program.statements() {
            if is_barrier(&statement, false) {
                continue;
            }
            let section =
                section(&statement).expect("a reorderable statement has a section (§15.2)");
            assert!(
                ORDER.contains(&section),
                "no section maps outside ORDER (§15.2)"
            );
            hit.insert(section);
        }
        assert_eq!(hit, ORDER.iter().copied().collect::<HashSet<Section>>());

        // Under reorder_includes, #include joins the reorderable kinds and the
        // codomain is exactly ORDER_WITH_INCLUDES' index set (§15.4).
        let mut hit_includes: HashSet<Section> = HashSet::new();
        for statement in program.statements() {
            if is_barrier(&statement, true) {
                continue;
            }
            let section =
                section(&statement).expect("a reorderable statement has a section (§15.2)");
            assert!(ORDER_WITH_INCLUDES.contains(&section));
            hit_includes.insert(section);
        }
        assert_eq!(
            hit_includes,
            ORDER_WITH_INCLUDES
                .iter()
                .copied()
                .collect::<HashSet<Section>>()
        );
    }

    #[test]
    fn bucket_emit_is_stable_within_a_barrier_region() {
        // Two regions divided by a #program barrier; within each, #defined then
        // Body then #show, each section's units in source order — stable, no
        // comparator (§15.2). The barrier holds its position between the regions.
        let text = concat!(
            "#show s1/1.\n",
            "r1.\n",
            "#defined d1/1.\n",
            "r2.\n",
            "#program part.\n",
            "#show s2/1.\n",
            "#defined d2/1.\n",
            "r3.\n",
        );
        let items = bucket_emit(items_of(text, Dialect::Clingo, false), ORDER);
        assert_eq!(
            tags(&items),
            [
                "#defined d1/1.",
                "r1.",
                "r2.",
                "#show s1/1.",
                "#program part.",
                "#defined d2/1.",
                "r3.",
                "#show s2/1.",
            ]
        );
    }

    #[test]
    fn reorder_includes_hoists_include_to_a_leading_section() {
        // By default #include is a held barrier dividing the program; under
        // reorder_includes it is a reorderable Include unit hoisted to the
        // leading section (§15.4).
        let text = concat!("r1.\n", "#include \"lib.lp\".\n", "#defined d/1.\n");

        let default = bucket_emit(items_of(text, Dialect::Clingo, false), ORDER);
        assert_eq!(
            tags(&default),
            ["r1.", "#include \"lib.lp\".", "#defined d/1."]
        );
        assert!(is_barrier(
            &first("#include \"f\".", Dialect::Clingo),
            false
        ));

        let hoisted = bucket_emit(items_of(text, Dialect::Clingo, true), ORDER_WITH_INCLUDES);
        assert_eq!(
            tags(&hoisted),
            ["#include \"lib.lp\".", "#defined d/1.", "r1."]
        );
        assert!(!is_barrier(
            &first("#include \"f\".", Dialect::Clingo),
            true
        ));
    }
}
