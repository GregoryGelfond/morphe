//! The typed-AST → document lowering (docs/design/morphe.md §7): the house
//! style, as code. A walk of the tree emits each construct by the §7.2 catalog
//! — necks and relations spaced, argument lists hugging their parens, aggregate
//! braces spaced inside, operator chains flat, a directive a space past its
//! keyword, the `#script` body byte-exact — while weaving every trivia comment
//! to the anchor and slot the tier gave it (§8), so a re-parse recovers the same
//! non-whitespace sequence and the certificate grants (§5.2, §5.3). A group per
//! bracketed construct stays flat when it fits and explodes one element per line
//! when it does not.
//!
//! The walk mirrors the tier's own shape (docs/design/syntax.md §6): recursive
//! descent over the bounded outer grammar — statements, rules, bodies — holding
//! no more stack than the tree is deep, and an explicit frame stack for the one
//! genuine depth source, the bracketed term families, so their nesting is the
//! stack's height and never call depth, as the tier's `terms` parser does. The
//! file door bounds both at `DEFAULT` (§5.6).
//!
//! Comment text and canonical spellings are owned here: rowan's token text is
//! cursor-scoped, not tree-lifetime, so the walk owns each spelling (§6).

use std::borrow::Cow;
use std::collections::HashSet;

use themelios_syntax::ast::{AstToken, Comment, Program, ScriptBody};
use themelios_syntax::attach::empty_line_between;
use themelios_syntax::dialect::Dialect;
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::{SyntaxElement, SyntaxKind, SyntaxNode, SyntaxToken, TokenRole, role};

use crate::comments::{self, Plan};
use crate::doc::Doc;
use crate::emit::{self, Wish};
use crate::options::FormatOptions;
use crate::spelling::spell;

/// The separator the house style places before the next emitted element.
/// Variants are ordered weakest-to-strongest, so `a.max(b)` takes the stronger:
/// a comment that owes the next element a separator raises the style's gap to at
/// least what the comment owes (§8.2).
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Gap {
    /// Nothing — the first element of the output.
    None,
    /// Abut: the oracle floor decides (nothing where it permits abutment, a
    /// space where abutment would fuse) (§5.5).
    Tight,
    /// A hugging width break: nothing when its group is flat, a newline when
    /// broken — the break after a tight term separator and inside a hugging
    /// bracket (§7.2). Weaker than a space, so a comment that owes one still
    /// wins the flat gap (§8.2).
    Wrap,
    /// A space the style wants — a neck, a relation, a pipe (§7.2); or the least
    /// a block comment leaves before what follows it on its line (§8.2).
    Space,
    /// A width break: a space when its group is flat, a newline when broken —
    /// the one-element-per-line body (§7.2).
    Soft,
    /// An unconditional newline — one statement, or the line a line comment ends
    /// (§7.1, §8.2).
    Hard,
    /// A single blank line, kept as one (§7.3).
    Blank,
}

/// Lower a parsed program to a document (§5.1 step 2): the house style over the
/// typed tree, canonically spelled (§7.4) or verbatim where a comment or doc
/// line (§7.3), the fusion oracle the floor under every significant adjacency
/// (§5.5), and every comment woven to its `(anchor, slot)` (§8). Total; linear
/// in the tree.
pub(crate) fn lower(parse: &Parse<Program>, options: &FormatOptions) -> Doc {
    let root = parse.syntax();
    let mut lowering = Lowering {
        dialect: parse.dialect(),
        preserve: options.preserve_spellings,
        plan: comments::plan(&root),
        last: None,
        prev_element: None,
        pending: None,
        bracket_depth: 0,
        woven: HashSet::new(),
    };
    let mut out = Vec::new();
    lowering.program(&root, &mut out);
    out.push(Doc::HardLine); // the file's single trailing newline (§7.1)
    Doc::Concat(out)
}

/// The walk's state: the dialect and spelling mode it lowers under, the comment
/// placement plan it consults per anchor (§8.1), the last significant token it
/// emitted (the oracle floor's left side, §5.5), the last element it emitted of
/// any kind (the reference for a source-derived break, so a blank between two
/// comments is read from the comment that precedes it, §7.3/§8.2), the separator
/// a just-emitted comment owes whatever follows it, and the bracket-nesting depth
/// that decides separator and operator spacing (§7.2).
struct Lowering {
    dialect: Dialect,
    preserve: bool,
    plan: Plan,
    last: Option<(SyntaxToken, String)>,
    prev_element: Option<SyntaxElement>,
    pending: Option<Gap>,
    /// The count of term and aggregate brackets currently open (a `(`/`[`/`{`/`|`
    /// of a term or aggregate — not a directive's parens). A separator is spaced
    /// at depth ≤ 1 and tight at ≥ 2; a term operator is spaced only at depth 0
    /// (§7.2).
    bracket_depth: usize,
    /// Comments already emitted where the walk met them, so `weave_leading` does
    /// not emit them again at their anchor. A leading comment interleaved among a
    /// statement's `%!` documentation lines is emitted in source order there (a
    /// doc line and a plain comment the anchor-time weave alone would reorder),
    /// then skipped when its anchor is reached (§8.2).
    woven: HashSet<SyntaxToken>,
}

impl Lowering {
    /// The program: each statement on its own line(s) (§7.1), a single author
    /// blank between two preserved as one (§7.3), documentation and top-level
    /// comments woven to their anchors (§8). Leading and trailing file blanks
    /// fall away — the walk emits nothing before the first anchor or after the
    /// last.
    fn program(&mut self, root: &SyntaxNode, out: &mut Vec<Doc>) {
        let root_el = SyntaxElement::Node(root.clone());
        for child in root.children_with_tokens() {
            match &child {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
                SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                    // Top-level leading/trailing comments are woven at their
                    // statement; only a dangling one stands on its own here.
                    if self.is_dangling(&root_el, token) {
                        let gap = break_between(self.prev_element.as_ref(), &child);
                        self.comment(out, token, gap);
                    }
                }
                SyntaxElement::Node(node) => {
                    let gap = break_between(self.prev_element.as_ref(), &child);
                    self.weave_leading(out, &child);
                    self.node(out, node, gap);
                    self.weave_trailing(out, &child);
                }
                SyntaxElement::Token(_) => {}
            }
        }
    }

    /// Lower one node under the incoming `gap`, dispatching on its kind to the
    /// house style it earns (§7.2); the structural forms with no bracket of their
    /// own — literals, atoms, signatures, the theory sublanguage — fall to the
    /// faithful tight walk, where the oracle floor sets each adjacency.
    fn node(&mut self, out: &mut Vec<Doc>, node: &SyntaxNode, gap: Gap) {
        match node.kind() {
            SyntaxKind::RULE | SyntaxKind::WEAK_CONSTRAINT => self.rule(out, node, gap),
            SyntaxKind::BODY => self.body(out, node, gap),
            // Spaced relations and guards (§7.2): a disjunction `|`, a comparison,
            // and an aggregate or theory guard each space their operator.
            SyntaxKind::DISJUNCTION
            | SyntaxKind::COMPARISON
            | SyntaxKind::GUARD
            | SyntaxKind::THEORY_GUARD => self.children(out, node, gap, Gap::Space),
            // Spaced-brace blocks (§7.2, syntax.md §5.8): aggregates and optimize;
            // the `#theory` directive and its `{ … }` term/operator definitions;
            // a theory atom's `{ elements }`. A keyword hugs its brace, a name
            // (a theory definition) is spaced from it; elements `; `-separated.
            SyntaxKind::FUNCTION_AGGREGATE
            | SyntaxKind::SET_AGGREGATE
            | SyntaxKind::OPTIMIZE_STATEMENT
            | SyntaxKind::THEORY_DEFINITION
            | SyntaxKind::TERM_DEFINITION
            | SyntaxKind::THEORY_ELEMENTS => self.aggregate(out, node, gap),
            // Spaced element lists (§7.2): a condition's conjunction, the
            // aggregate/optimize elements whose weight/tuple commas take a space,
            // and the theory element (`opterms : condition`) and definitions.
            SyntaxKind::CONDITION
            | SyntaxKind::BODY_AGGREGATE_ELEMENT
            | SyntaxKind::HEAD_AGGREGATE_ELEMENT
            | SyntaxKind::OPTIMIZE_ELEMENT
            | SyntaxKind::EDGE
            | SyntaxKind::THEORY_ELEMENT
            | SyntaxKind::OP_DEFINITION
            | SyntaxKind::ATOM_DEFINITION => self.spaced_list(out, node, gap),
            // The directive families (§7.2): a space after the keyword, the
            // family punctuation spaced, the dot hugging, a trailing annotation
            // spaced.
            SyntaxKind::SHOW_STATEMENT
            | SyntaxKind::PROGRAM_STATEMENT
            | SyntaxKind::CONST_STATEMENT
            | SyntaxKind::EXTERNAL_STATEMENT
            | SyntaxKind::EDGE_STATEMENT
            | SyntaxKind::PROJECT_STATEMENT
            | SyntaxKind::DEFINED_STATEMENT
            | SyntaxKind::INCLUDE_STATEMENT
            | SyntaxKind::HEURISTIC_STATEMENT => self.directive(out, node, gap),
            // The `#script` header renders through the `)`; the body is byte-exact.
            SyntaxKind::SCRIPT_STATEMENT => self.script(out, node, gap),
            // The term and annotation families — the bracket depth source — on
            // the explicit stack; separator and operator spacing follow the
            // bracket depth (§7.2, §5.6).
            SyntaxKind::ARGUMENTS
            | SyntaxKind::ANNOTATION
            | SyntaxKind::BINARY_TERM
            | SyntaxKind::UNARY_TERM
            | SyntaxKind::POOL
            | SyntaxKind::TUPLE
            | SyntaxKind::FUNCTION_TERM
            | SyntaxKind::EXTERNAL_TERM
            | SyntaxKind::ABS_TERM
            | SyntaxKind::CONSTANT_TERM
            | SyntaxKind::VARIABLE_TERM
            | SyntaxKind::SPLICE_TERM
            // The theory term families (syntax.md §5.8): a set/list/tuple/function
            // hugs its bracket, an opterm is a flat always-spaced operator run.
            | SyntaxKind::THEORY_SET
            | SyntaxKind::THEORY_LIST
            | SyntaxKind::THEORY_TUPLE
            | SyntaxKind::THEORY_FUNCTION
            | SyntaxKind::THEORY_OPTERM => self.term_engine(out, node, gap),
            // A theory atom (`&name(args){ elements } guard`, syntax.md §5.8):
            // `&name` and its arguments hug, the `{ elements }` block is spaced
            // and hugs the name, the guard is spaced after the `}`.
            SyntaxKind::THEORY_ATOM => self.theory_atom(out, node, gap),
            _ => self.children(out, node, gap, Gap::Tight),
        }
    }

    /// A rule or weak constraint (§7.2): the head, neck (`:-`/`:~`), body, dot,
    /// and — for a weak constraint — the annotation, one group. The head and neck
    /// lead; the body (its terminating dot folded in so the fit counts it, §7.1)
    /// nests one level. When the rule does not fit the neck breaks and the body
    /// drops to its own indented line; the head is itself a group, so it explodes
    /// only when it alone is too wide — kallos's cascade, a boundless choice head
    /// that explodes carrying the neck onto its own line.
    fn rule(&mut self, out: &mut Vec<Doc>, node: &SyntaxNode, first_gap: Gap) {
        let parent = SyntaxElement::Node(node.clone());
        let mut first = true;
        // The head and neck (before the nest); the tail — body, dangling-before-
        // dot, dot, annotation — nested within the same group.
        let mut head: Vec<Doc> = Vec::new();
        let mut tail: Vec<Doc> = Vec::new();
        let mut in_tail = false;
        // Whether the neck has been emitted: a dangling comment after it, before
        // the body, rides with the body (nested to its indent), not the head.
        let mut seen_neck = false;
        // Whether the first significant element (the head) is out: a comment
        // before it emits in source order among the doc lines, not at its anchor.
        let mut head_seen = false;
        for child in node.children_with_tokens() {
            match &child {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
                SyntaxElement::Token(token) if role(token) == TokenRole::Documentation => {
                    // Documentation stands above the statement, before the group.
                    self.doc_line(out, token, if first { first_gap } else { Gap::Hard });
                    first = false;
                }
                SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                    // A dangling comment before the dot stays inside the tail,
                    // before the dot, so it is not reordered across it (§8.2).
                    if in_tail {
                        if self.is_dangling(&parent, token) {
                            let gap = self.break_before(token);
                            self.comment(&mut tail, token, gap);
                        }
                    } else if seen_neck {
                        // A dangling comment between the neck and the body rides
                        // with the body — woven into the nested tail so it lands
                        // at the body's indent, the column a re-format also gives
                        // a comment now leading the body (§7.3, §8.2).
                        self.weave_dangling(&mut tail, node, token);
                    } else if !head_seen {
                        // Before the head: emit in source order among the doc
                        // lines it interleaves, not deferred to the head's anchor
                        // (which would order it after them all) (§8.2, §7.3).
                        self.weave_in_place(out, token);
                    } else {
                        self.weave_dangling(&mut head, node, token);
                    }
                }
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::DOT && in_tail => {
                    self.weave_leading(&mut tail, &child);
                    self.tok(&mut tail, token, Gap::Tight);
                    self.weave_trailing(&mut tail, &child);
                }
                SyntaxElement::Token(token) => {
                    let gap = if first {
                        first_gap
                    } else {
                        match token.kind() {
                            SyntaxKind::NECK | SyntaxKind::WEAK_NECK => Gap::Space,
                            _ => Gap::Tight,
                        }
                    };
                    self.weave_leading(&mut head, &child);
                    self.tok(&mut head, token, gap);
                    self.weave_trailing(&mut head, &child);
                    seen_neck |= matches!(token.kind(), SyntaxKind::NECK | SyntaxKind::WEAK_NECK);
                    head_seen = true;
                    first = false;
                }
                SyntaxElement::Node(inner) => {
                    head_seen = true;
                    match inner.kind() {
                        SyntaxKind::BODY => {
                            self.weave_leading(&mut tail, &child);
                            self.body_inner(&mut tail, inner, Gap::Soft);
                            self.weave_trailing(&mut tail, &child);
                            in_tail = true;
                            first = false;
                        }
                        SyntaxKind::ANNOTATION => {
                            let target = if in_tail { &mut tail } else { &mut head };
                            self.weave_leading(target, &child);
                            self.node(target, inner, Gap::Space);
                            self.weave_trailing(target, &child);
                            first = false;
                        }
                        _ => {
                            self.weave_leading(&mut head, &child);
                            self.node(&mut head, inner, if first { first_gap } else { Gap::Tight });
                            self.weave_trailing(&mut head, &child);
                            first = false;
                        }
                    }
                }
            }
        }
        if in_tail {
            // The body nests one level under the head/neck line; the whole rule
            // is one group, so the neck breaks with it (kallos's cascade).
            head.push(Doc::Nest(Box::new(Doc::Concat(tail))));
            out.push(as_group(head));
        } else {
            // A fact: the head and its dot, no body group.
            out.extend(head);
        }
    }

    /// A body-list as a group (§7.2): the neck-to-first-element break and each
    /// inter-element break are soft, so the group stays flat — `:- e1, e2.` —
    /// when it fits and explodes — the neck on the head's line, then one element
    /// per line indented one level — when it does not. The separators hug the
    /// element they follow.
    fn body(&mut self, out: &mut Vec<Doc>, node: &SyntaxNode, first_gap: Gap) {
        let mut inner = Vec::new();
        self.body_inner(&mut inner, node, first_gap);
        if !inner.is_empty() {
            out.push(group_nest(inner));
        }
    }

    /// Lower a body's elements into `inner` — the neck-to-first break and each
    /// inter-element break soft — without wrapping them in a group, so the rule
    /// tail can fold its terminating dot into the same group its fit test counts
    /// (§7.1).
    fn body_inner(&mut self, inner: &mut Vec<Doc>, node: &SyntaxNode, first_gap: Gap) {
        let mut gap = first_gap;
        for child in node.children_with_tokens() {
            match &child {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
                SyntaxElement::Token(token) if role(token) == TokenRole::Documentation => {
                    self.doc_line(inner, token, gap);
                    gap = Gap::Soft;
                }
                SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                    self.weave_dangling(inner, node, token);
                }
                SyntaxElement::Token(token) => {
                    // A `,`/`;` separator hugs the element it follows.
                    self.weave_leading(inner, &child);
                    self.tok(inner, token, Gap::Tight);
                    self.weave_trailing(inner, &child);
                    gap = Gap::Soft;
                }
                SyntaxElement::Node(inner_node) => {
                    self.weave_leading(inner, &child);
                    self.node(inner, inner_node, gap);
                    self.weave_trailing(inner, &child);
                    gap = Gap::Soft;
                }
            }
        }
    }

    /// Walk a node's children under one `between` gap (§7.2): `Space` for the
    /// disjunction and comparison chains it spaces, `Tight` for the faithful
    /// generic fallback, where the oracle floor sets each adjacency. The first
    /// significant child takes the incoming `first_gap`; comments weave to their
    /// anchors as the walk reaches them (§8.2).
    fn children(&mut self, out: &mut Vec<Doc>, node: &SyntaxNode, first_gap: Gap, between: Gap) {
        let mut gap = first_gap;
        // Whether the first significant child is out: a comment before it (a
        // top-level query's leading run among its doc lines) emits in source
        // order, not deferred to its anchor (§8.2, §7.3).
        let mut head_seen = false;
        for child in node.children_with_tokens() {
            match &child {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
                SyntaxElement::Token(token) if role(token) == TokenRole::Documentation => {
                    self.doc_line(out, token, gap);
                    gap = between;
                }
                SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                    self.weave_statement_trivia(out, node, token, !head_seen);
                }
                SyntaxElement::Token(token) => {
                    // A condition colon is spaced on both sides (§7.2); every
                    // other token takes the walk's running gap.
                    let colon = token.kind() == SyntaxKind::COLON;
                    self.weave_leading(out, &child);
                    self.tok(out, token, if colon { Gap::Space } else { gap });
                    self.weave_trailing(out, &child);
                    head_seen = true;
                    gap = if colon { Gap::Space } else { between };
                }
                SyntaxElement::Node(inner) => {
                    head_seen = true;
                    self.weave_leading(out, &child);
                    self.node(out, inner, gap);
                    self.weave_trailing(out, &child);
                    gap = between;
                }
            }
        }
    }

    /// Lower a term-rooted or argument-list subtree — the one genuine depth
    /// source, bracket nesting — on an explicit frame stack, so the input's
    /// bracket depth is the stack's height and never this walk's call depth
    /// (§5.6; the tier's `terms` frame loop, syntax.md §6.2). A frame per node
    /// accumulates its lowered children; a bracket frame wraps its interior in
    /// a group that hugs its brackets and explodes one element per line when it
    /// does not fit (§7.2), a flat frame concatenates its children. Separator and
    /// operator spacing follow the ambient bracket depth (`self.bracket_depth`),
    /// which the bracket frames raise as they open and lower as they close (§7.2,
    /// kallos's depth rule): a `,`/`;` is spaced at depth ≤ 1, an operator only
    /// at depth 0.
    fn term_engine(&mut self, out: &mut Vec<Doc>, entry: &SyntaxNode, first_gap: Gap) {
        let mut frames = vec![Frame::open(entry, first_gap)];
        loop {
            let top = frames.len() - 1;
            if let Some(child) = frames[top].children.next() {
                self.term_child(&mut frames, top, &child);
                continue;
            }
            // The frame's children are spent: close it into its parent, or, at
            // the base, emit the finished document and return. The stack holds
            // at least the entry frame until that base return, so the pop always
            // yields one — the invariant the type cannot express.
            let frame = frames
                .pop()
                .expect("the engine holds at least the entry frame");
            let element = frame.element.clone();
            let doc = frame.finish();
            let depth = self.bracket_depth;
            let Some(parent) = frames.last_mut() else {
                out.push(doc);
                return;
            };
            parent.interior.push(doc);
            // The just-closed child's trailing comments follow it; its leading
            // were woven before its frame opened.
            self.weave_trailing(&mut parent.interior, &element);
            parent.gap = element_gap(parent.kind, depth);
        }
    }

    /// Process one source-order child of the frame at `top`: skip whitespace,
    /// weave a documentation or dangling comment as the outer walk does (§8.2),
    /// emit a significant token, or open a child frame for a node.
    fn term_child(&mut self, frames: &mut Vec<Frame>, top: usize, child: &SyntaxElement) {
        match child {
            SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
            SyntaxElement::Token(token) if role(token) == TokenRole::Documentation => {
                let gap = frames[top].gap;
                self.doc_line(&mut frames[top].interior, token, gap);
                frames[top].gap = element_gap(frames[top].kind, self.bracket_depth);
            }
            SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                let parent = frames[top].element.clone();
                if self.is_dangling(&parent, token) {
                    let gap = self.break_before(token);
                    self.comment(&mut frames[top].interior, token, gap);
                }
            }
            SyntaxElement::Token(_) => self.term_token(frames, top, child),
            SyntaxElement::Node(node) => {
                self.weave_leading(&mut frames[top].interior, child);
                let gap = frames[top].gap;
                frames.push(Frame::open(node, gap));
            }
        }
    }

    /// Emit one significant token within the frame at `top`: an open or close
    /// bracket into the frame's own slots (the oracle floor keeps them
    /// abutting their neighbors, §5.5); a `,`/`;` separator hugging the element
    /// it follows, its own break carried after it; or a leaf or operator token
    /// into the interior under the frame's running gap.
    fn term_token(&mut self, frames: &mut [Frame], top: usize, child: &SyntaxElement) {
        let SyntaxElement::Token(token) = child else {
            return;
        };
        let kind = frames[top].kind;
        if let Some((open, close)) = brackets(kind) {
            if !frames[top].opened && token.kind() != open {
                // A name before the open bracket (a theory function's `f`): the
                // prefix rides in the open slot, ahead of the bracket, so `f(`
                // stays outside the group's nest.
                let gap = frames[top].gap;
                self.weave_leading(&mut frames[top].open, child);
                self.tok(&mut frames[top].open, token, gap);
                self.weave_trailing(&mut frames[top].open, child);
                frames[top].gap = Gap::Tight;
                return;
            }
            if !frames[top].opened && token.kind() == open {
                let gap = frames[top].gap;
                self.weave_leading(&mut frames[top].open, child);
                self.tok(&mut frames[top].open, token, gap);
                self.weave_trailing(&mut frames[top].open, child);
                frames[top].opened = true;
                frames[top].gap = Gap::Tight;
                // The bracket is open: its interior sits one level deeper (§7.2).
                self.bracket_depth += 1;
                return;
            }
            if frames[top].opened && token.kind() == close {
                // Back to the enclosing depth before the closer.
                self.bracket_depth = self.bracket_depth.saturating_sub(1);
                self.weave_leading(&mut frames[top].close, child);
                self.tok(&mut frames[top].close, token, Gap::Tight);
                self.weave_trailing(&mut frames[top].close, child);
                return;
            }
        }
        if is_list(kind) && matches!(token.kind(), SyntaxKind::COMMA | SyntaxKind::SEMICOLON) {
            self.weave_leading(&mut frames[top].interior, child);
            self.tok(&mut frames[top].interior, token, Gap::Tight);
            self.weave_trailing(&mut frames[top].interior, child);
            // The element after a `,`/`;` takes the list's break — a space when
            // flat at bracket depth ≤ 1 (`p(X, Y)`, `f(a; b)`), nothing when flat
            // deeper (`p(f(a,b))`), a newline when the list explodes (§7.2,
            // kallos's depth rule). Carrying it as the gap lets `separate`
            // reconcile it with any space a trailing comment on the separator
            // owes (§8.2).
            frames[top].gap = if self.bracket_depth <= 1 {
                Gap::Soft
            } else {
                Gap::Wrap
            };
            return;
        }
        let gap = frames[top].gap;
        self.weave_leading(&mut frames[top].interior, child);
        self.tok(&mut frames[top].interior, token, gap);
        self.weave_trailing(&mut frames[top].interior, child);
        frames[top].gap = element_gap(kind, self.bracket_depth);
    }

    /// Emit one significant child under `gap`: a token by its canonical
    /// spelling, a node by its house style.
    fn emit_child(&mut self, out: &mut Vec<Doc>, child: &SyntaxElement, gap: Gap) {
        match child {
            SyntaxElement::Token(token) => self.tok(out, token, gap),
            SyntaxElement::Node(node) => self.node(out, node, gap),
        }
    }

    /// An aggregate (§7.2): a leading negation and the guards spaced, the
    /// keyword hugging its brace, the braces spaced inside, the elements
    /// `; `-separated, the whole exploding one element per line when it does not
    /// fit. The set form carries no keyword; both forms take an optional guard
    /// on either side (syntax.md §5.3). The braces bound the group so a right
    /// guard stays on the aggregate's line.
    fn aggregate(&mut self, out: &mut Vec<Doc>, node: &SyntaxNode, first_gap: Gap) {
        let mut open = Vec::new();
        let mut interior = Vec::new();
        let mut close = Vec::new();
        // Content after the braces that is *not* folded into the group — a right
        // guard, spaced and kept on the aggregate's line (§7.2). The terminating
        // dot, by contrast, folds into `close` so the group's fit counts it (§7.1).
        let mut after_braces: Vec<Doc> = Vec::new();
        let mut inside = false;
        let mut done = false;
        // Whether the first significant element (the keyword or brace) is out: a
        // comment before it emits in source order among the doc lines (§8.2).
        let mut head_seen = false;
        let mut gap = first_gap;
        for child in node.children_with_tokens() {
            match &child {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
                SyntaxElement::Token(token) if role(token) == TokenRole::Documentation => {
                    self.doc_line(out, token, gap);
                    gap = Gap::Tight;
                }
                SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                    if head_seen {
                        let target = if inside && !done {
                            &mut interior
                        } else if done {
                            // After the braces, before the group is assembled: a
                            // dangling comment stays in the closing slot, ahead of
                            // the folded dot, so it is not reordered across it
                            // (§8.2) — as a rule keeps a dangling-before-dot.
                            &mut close
                        } else {
                            &mut *out
                        };
                        self.weave_dangling(target, node, token);
                    } else {
                        // Before the keyword: emit among the doc lines in source
                        // order, not deferred to the anchor (§8.2, §7.3).
                        self.weave_in_place(out, token);
                    }
                }
                _ if !inside => {
                    head_seen = true;
                    if child.kind() == SyntaxKind::L_BRACE {
                        self.weave_leading(&mut open, &child);
                        self.emit_child(&mut open, &child, gap);
                        self.weave_trailing(&mut open, &child);
                        inside = true;
                        // The brace is a bracket: its elements sit one level
                        // deeper, so a nested term list within them tightens
                        // (`#sum{ assign(T,S) }`) (§7.2).
                        self.bracket_depth += 1;
                        // The first element hugs the oracle just inside the
                        // brace; the brace's own break spaces it (§7.2).
                        gap = Gap::Tight;
                    } else {
                        self.weave_leading(out, &child);
                        self.emit_child(out, &child, gap);
                        self.weave_trailing(out, &child);
                        gap = pre_brace_gap(&child);
                    }
                }
                _ if !done => {
                    if child.kind() == SyntaxKind::R_BRACE {
                        // Back to the enclosing depth before the closer. The group
                        // is assembled after the loop, once any terminating dot has
                        // folded into `close` (§7.1) — not here.
                        self.bracket_depth = self.bracket_depth.saturating_sub(1);
                        self.weave_leading(&mut close, &child);
                        self.emit_child(&mut close, &child, Gap::Tight);
                        self.weave_trailing(&mut close, &child);
                        done = true;
                    } else if child.kind() == SyntaxKind::SEMICOLON {
                        // The `;` weaves its own comments, as every other list
                        // separator does (§8.2): a comment trailing the `;` stays
                        // with it, ahead of the next element's leading run, so a
                        // re-parse recovers the same order (§5.2).
                        self.weave_leading(&mut interior, &child);
                        self.emit_child(&mut interior, &child, Gap::Tight);
                        self.weave_trailing(&mut interior, &child);
                        gap = Gap::Soft;
                    } else {
                        self.weave_leading(&mut interior, &child);
                        self.emit_child(&mut interior, &child, gap);
                        self.weave_trailing(&mut interior, &child);
                        gap = Gap::Tight;
                    }
                }
                _ => {
                    // After the braces. A terminating dot (a token, the optimize
                    // statement) folds into the closing slot so the group's fit
                    // counts it — the rule's dot rule applied here (§7.1): at the
                    // boundary the braces explode rather than emit a line one over
                    // the width. A right guard (a node) is spaced and kept out of
                    // the group, so it stays on the aggregate's line (§7.2).
                    if matches!(child, SyntaxElement::Node(_)) {
                        self.weave_leading(&mut after_braces, &child);
                        self.emit_child(&mut after_braces, &child, Gap::Space);
                        self.weave_trailing(&mut after_braces, &child);
                    } else {
                        self.weave_leading(&mut close, &child);
                        self.emit_child(&mut close, &child, Gap::Tight);
                        self.weave_trailing(&mut close, &child);
                    }
                }
            }
        }
        // The group is assembled once, after the loop: any terminating dot has by
        // now folded into `close` (§7.1), so its width falls inside the group's
        // fit; a right guard, held in `after_braces`, follows the group spaced.
        out.push(brace_group(
            std::mem::take(&mut open),
            std::mem::take(&mut interior),
            std::mem::take(&mut close),
        ));
        out.append(&mut after_braces);
    }

    /// A spaced element list (§7.2): a condition, an aggregate element, or an
    /// optimize element — each element after the first preceded by a space, the
    /// `,`/`;` hugging the element it follows, the condition `:` spaced on both
    /// sides. The space is fixed, not a width break, so an element stays on its
    /// line when the construct that holds it explodes.
    fn spaced_list(&mut self, out: &mut Vec<Doc>, node: &SyntaxNode, first_gap: Gap) {
        let mut gap = first_gap;
        for child in node.children_with_tokens() {
            match &child {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
                SyntaxElement::Token(token) if role(token) == TokenRole::Documentation => {
                    self.doc_line(out, token, gap);
                    gap = Gap::Space;
                }
                SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                    self.weave_dangling(out, node, token);
                }
                SyntaxElement::Token(token)
                    if matches!(token.kind(), SyntaxKind::COMMA | SyntaxKind::SEMICOLON) =>
                {
                    self.weave_leading(out, &child);
                    self.tok(out, token, Gap::Tight);
                    self.weave_trailing(out, &child);
                    gap = Gap::Space;
                }
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::COLON => {
                    self.weave_leading(out, &child);
                    self.tok(out, token, Gap::Space);
                    self.weave_trailing(out, &child);
                    gap = Gap::Space;
                }
                _ => {
                    self.weave_leading(out, &child);
                    self.emit_child(out, &child, gap);
                    self.weave_trailing(out, &child);
                    gap = Gap::Tight;
                }
            }
        }
    }

    /// A directive statement (§7.2): a single space after the keyword, the
    /// family's `=`/`:` punctuation spaced on both sides, its content composed
    /// through the term and body rules, the terminating dot hugging what
    /// precedes it, and a trailing annotation spaced after the dot. The oracle
    /// floor already spaces a keyword abutting a word; the house space covers
    /// the keyword that abuts a bracket or a quote (`#edge (…)`, `#include "…"`).
    fn directive(&mut self, out: &mut Vec<Doc>, node: &SyntaxNode, first_gap: Gap) {
        let mut running = first_gap;
        let mut first = true;
        for child in node.children_with_tokens() {
            match &child {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
                SyntaxElement::Token(token) if role(token) == TokenRole::Documentation => {
                    self.doc_line(out, token, running);
                }
                SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                    // A comment met before the keyword rides among the doc lines
                    // (§8.2, §7.3); after it, at its dangling slot.
                    self.weave_statement_trivia(out, node, token, first);
                }
                _ => {
                    let before = if first {
                        running
                    } else {
                        match child.kind() {
                            SyntaxKind::DOT => Gap::Tight,
                            SyntaxKind::ANNOTATION | SyntaxKind::EQ | SyntaxKind::COLON => {
                                Gap::Space
                            }
                            _ => running,
                        }
                    };
                    self.weave_leading(out, &child);
                    self.emit_child(out, &child, before);
                    self.weave_trailing(out, &child);
                    running = if first
                        || matches!(
                            child.kind(),
                            SyntaxKind::EQ
                                | SyntaxKind::COLON
                                | SyntaxKind::COMMA
                                | SyntaxKind::SEMICOLON
                        ) {
                        Gap::Space
                    } else {
                        Gap::Tight
                    };
                    first = false;
                }
            }
        }
    }

    /// A `#script(lang) … #end.` statement (§7.2): the header rendered through
    /// the `)`, then the body byte-exact from immediately after it — its value,
    /// the trailing blanks and tabs trimmed (syntax.md §11.1) — neither indented
    /// nor reflowed, its newlines content; and `#end.` after it, on its own line
    /// where the body ends in a newline. The oracle floor does not reach across
    /// the verbatim body, so nothing is fused at the `)`/body or body/`#end`
    /// boundary.
    fn script(&mut self, out: &mut Vec<Doc>, node: &SyntaxNode, first_gap: Gap) {
        let mut gap = first_gap;
        // Whether the first significant element (the `#script` keyword) is out: a
        // comment before it emits in source order among the doc lines (§8.2).
        let mut head_seen = false;
        for child in node.children_with_tokens() {
            match &child {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
                SyntaxElement::Token(token) if role(token) == TokenRole::Documentation => {
                    self.doc_line(out, token, gap);
                    gap = Gap::Tight;
                }
                SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                    self.weave_statement_trivia(out, node, token, !head_seen);
                }
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::SCRIPT_BODY => {
                    let value = ScriptBody::cast(token.clone())
                        .map_or_else(|| token.text().to_owned(), |body| body.value().to_owned());
                    if value.is_empty() {
                        // A whitespace-only body has an empty value() — trailing
                        // blanks/tabs are the value's one latitude (§7.2), and here
                        // they are the whole body. Emitting that empty value verbatim
                        // would leave nothing between `)` and `#end`, which re-lexes
                        // with no SCRIPT_BODY token at all — dropping a token the
                        // certificate's interleaved sequence requires (§5.2), the
                        // refusal a whitespace-only `#script` body would otherwise hit.
                        // Emit a single space: it re-lexes to a SCRIPT_BODY whose
                        // value() is still empty, keeping the token without altering
                        // the certified content. (A truly body-less `#script(python)#end.`
                        // carries no SCRIPT_BODY token and never reaches this arm.)
                        out.push(Doc::Text(Cow::Borrowed(" ")));
                    } else {
                        // A body that does not end its own line keeps a single blank
                        // before `#end` — layout the value trims, not a newline that
                        // would change the value and break the certificate (§5.3).
                        let inline = !value.ends_with('\n');
                        out.push(Doc::Text(Cow::Owned(value)));
                        if inline {
                            out.push(Doc::Text(Cow::Borrowed(" ")));
                        }
                    }
                    // The verbatim region ends the oracle's reach: `#end` abuts
                    // the body's last line (a newline there sets it on its own).
                    self.last = None;
                    self.prev_element = Some(child.clone());
                    gap = Gap::None;
                    head_seen = true;
                }
                _ => {
                    head_seen = true;
                    self.weave_leading(out, &child);
                    self.emit_child(out, &child, gap);
                    self.weave_trailing(out, &child);
                    gap = Gap::Tight;
                }
            }
        }
    }

    /// A theory atom (syntax.md §5.8): `&name(args){ elements } guard` — the `&`,
    /// the name, and any argument list hug (`&sum(1)`); the `{ elements }` block
    /// hugs the name and is spaced inside; the guard is spaced after the `}`. A
    /// body position may prefix `not`, which the oracle floor spaces.
    fn theory_atom(&mut self, out: &mut Vec<Doc>, node: &SyntaxNode, first_gap: Gap) {
        let mut gap = first_gap;
        for child in node.children_with_tokens() {
            match &child {
                SyntaxElement::Token(token) if token.kind() == SyntaxKind::WHITESPACE => {}
                SyntaxElement::Token(token) if role(token) == TokenRole::Documentation => {
                    self.doc_line(out, token, gap);
                    gap = Gap::Tight;
                }
                SyntaxElement::Token(token) if role(token) == TokenRole::Trivia => {
                    self.weave_dangling(out, node, token);
                }
                _ => {
                    // The `&name(args){ elements }` chain hugs; only a guard after
                    // the `}` is spaced from it.
                    let before = if child.kind() == SyntaxKind::THEORY_GUARD {
                        Gap::Space
                    } else {
                        gap
                    };
                    self.weave_leading(out, &child);
                    self.emit_child(out, &child, before);
                    self.weave_trailing(out, &child);
                    gap = Gap::Tight;
                }
            }
        }
    }

    /// Emit the comments leading `anchor` (§8.2): each on its own line directly
    /// above the anchor, the first keeping a single author blank before the run,
    /// none inside it — so a re-parse yields `Leading(anchor)`.
    fn weave_leading(&mut self, out: &mut Vec<Doc>, anchor: &SyntaxElement) {
        if self.plan.leading(anchor).is_empty() {
            return; // the common case: no run to weave, no allocation
        }
        let comments: Vec<SyntaxToken> = self
            .plan
            .leading(anchor)
            .iter()
            .filter(|comment| !self.woven.contains(*comment))
            .cloned()
            .collect();
        for (index, comment) in comments.iter().enumerate() {
            let gap = if index == 0 {
                self.break_before(comment)
            } else {
                Gap::Hard
            };
            self.comment(out, comment, gap);
        }
    }

    /// Emit a comment where the walk meets it, in source order — then record it so
    /// [`weave_leading`](Self::weave_leading) does not emit it again at its anchor.
    /// A statement's leading comment may sit among its `%!` documentation lines,
    /// which emit in the walk; weaving it only at its anchor would order it after
    /// them and move it across a token, so the pre-statement region emits it here
    /// instead (§8.2, §7.3).
    fn weave_in_place(&mut self, out: &mut Vec<Doc>, token: &SyntaxToken) {
        let gap = self.break_before(token);
        self.comment(out, token, gap);
        self.woven.insert(token.clone());
    }

    /// Weave a statement's trivia comment where the walk meets it: before the
    /// first significant element, in source order among the doc lines it
    /// interleaves (§8.2, §7.3); after it, at its dangling slot on the same
    /// output. The one home for the statement lowerings whose children share a
    /// single output — the rule (head, then a nested tail) and the aggregate
    /// (open/interior/close) split their targets and weave the two cases where
    /// each belongs.
    fn weave_statement_trivia(
        &mut self,
        out: &mut Vec<Doc>,
        node: &SyntaxNode,
        token: &SyntaxToken,
        before_head: bool,
    ) {
        if before_head {
            self.weave_in_place(out, token);
        } else {
            self.weave_dangling(out, node, token);
        }
    }

    /// Emit the comments trailing `anchor` (§8.2): on the anchor's line, after
    /// it, a space between — so a re-parse yields `Trailing(anchor)`. The line
    /// comment leaves the cursor owing a break, so whatever follows begins a new
    /// line.
    fn weave_trailing(&mut self, out: &mut Vec<Doc>, anchor: &SyntaxElement) {
        if self.plan.trailing(anchor).is_empty() {
            return; // the common case: no run to weave, no allocation
        }
        let comments = self.plan.trailing(anchor).to_vec();
        for comment in &comments {
            self.comment(out, comment, Gap::Space);
        }
    }

    /// Weave `token` if it dangles in `parent` (§8.2): on its own line, kept off
    /// what precedes it by the author's blank where one detached it — the shape
    /// rule 3 admits, derived from the comment's position. A leading or trailing
    /// comment is woven at its anchor instead and skipped here.
    fn weave_dangling(&mut self, out: &mut Vec<Doc>, parent: &SyntaxNode, token: &SyntaxToken) {
        let parent_el = SyntaxElement::Node(parent.clone());
        if self.is_dangling(&parent_el, token) {
            let gap = self.break_before(token);
            self.comment(out, token, gap);
        }
    }

    /// Whether `token` dangles in `parent` per the plan (§8.1).
    fn is_dangling(&self, parent: &SyntaxElement, token: &SyntaxToken) -> bool {
        self.plan.dangling(parent).iter().any(|c| c == token)
    }

    /// The break before a comment, from the source: one blank line where the
    /// author left one, else a plain line, and nothing at the very start of the
    /// output (§7.3, §8.2's block-aware detach). Measured from the element the
    /// walk last emitted — a preceding comment, not only a significant token —
    /// so the blank that detaches a dangling comment from a following leading run
    /// is read, not spanned.
    fn break_before(&self, comment: &SyntaxToken) -> Gap {
        break_between(
            self.prev_element.as_ref(),
            &SyntaxElement::Token(comment.clone()),
        )
    }

    /// Emit a significant token: its canonical spelling (§7.4), or verbatim text
    /// under `preserve_spellings`, preceded by the gap's separator. It becomes
    /// the oracle floor's new left side (§5.5).
    fn tok(&mut self, out: &mut Vec<Doc>, token: &SyntaxToken, gap: Gap) {
        let text = spell(token.kind(), token.text(), self.preserve).into_owned();
        self.separate(out, gap, &text);
        out.push(Doc::Text(Cow::Owned(text.clone())));
        self.last = Some((token.clone(), text));
        self.prev_element = Some(SyntaxElement::Token(token.clone()));
    }

    /// Emit a documentation line verbatim (§7.3): its bytes are content, and it
    /// stands on its own line above its statement. It is not the oracle floor's
    /// left side — a break always follows it.
    fn doc_line(&mut self, out: &mut Vec<Doc>, token: &SyntaxToken, gap: Gap) {
        let text = token.text().to_owned();
        self.separate(out, gap, &text);
        out.push(Doc::Text(Cow::Owned(text)));
        self.prev_element = Some(SyntaxElement::Token(token.clone()));
        self.pending = Some(Gap::Hard); // a doc line owns its line; the statement follows below
    }

    /// Emit a trivia comment verbatim (§7.3): a line comment without its trailing
    /// horizontal whitespace, which is layout (syntax.md §11.1); a block comment
    /// whole. A line comment runs to the line's end, so it owes whatever follows a
    /// break; a block comment ends mid-line, so it owes only a space and what
    /// follows may share its line (§8.2). It is not the oracle floor's left side.
    fn comment(&mut self, out: &mut Vec<Doc>, token: &SyntaxToken, gap: Gap) {
        let text = Comment::cast(token.clone()).map_or_else(
            || token.text().to_owned(),
            |comment| comment.content().to_owned(),
        );
        self.separate(out, gap, &text);
        out.push(Doc::Text(Cow::Owned(text)));
        self.prev_element = Some(SyntaxElement::Token(token.clone()));
        self.pending = Some(if token.kind() == SyntaxKind::BLOCK_COMMENT {
            Gap::Space
        } else {
            Gap::Hard
        });
    }

    /// Push the separator for `gap` before an element whose emitted text is
    /// `next`. A just-emitted comment owes what follows at least a break (a line
    /// comment) or a space (a block comment), so the gap is raised to the stronger
    /// of the two (§8.2). Only an abutting or spaced gap consults the oracle
    /// (§5.5); a break already clears any floor.
    fn separate(&mut self, out: &mut Vec<Doc>, gap: Gap, next: &str) {
        let gap = match self.pending.take() {
            Some(owed) => gap.max(owed),
            None => gap,
        };
        match gap {
            Gap::None => {}
            Gap::Tight | Gap::Space => {
                if let Some((token, text)) = &self.last {
                    let wish = match gap {
                        Gap::Space => Wish::Space,
                        _ => Wish::Abut,
                    };
                    out.push(emit::separator(token, text, next, wish, self.dialect));
                }
            }
            Gap::Wrap => out.push(Doc::SoftLine),
            Gap::Soft => out.push(Doc::Line),
            Gap::Hard => out.push(Doc::HardLine),
            Gap::Blank => out.push(Doc::BlankLine),
        }
    }
}

/// Wrap `body` in a group, but with any leading unconditional break — the
/// inter-statement gap (§7.1) — standing *before* the group, never inside it.
/// Within, `fits` counts that break and forces the group to explode a statement
/// that would fit; the break is not the statement's own layout (§6). A break
/// *internal* to the statement — a body element, a comment before the dot — stays,
/// breaking with the group as it must.
fn as_group(mut body: Vec<Doc>) -> Doc {
    let group = |body| Doc::Group(Box::new(Doc::Concat(body)));
    match take_leading_break(&mut body) {
        Some(brk) => Doc::Concat(vec![brk, group(body)]),
        None => group(body),
    }
}

/// Detach a leading unconditional break from `body`, descending through a leading
/// `Concat` — a grouped head element carries its own already-hoisted break there,
/// so the inter-statement break bubbles out through every enclosing group (§6).
fn take_leading_break(body: &mut Vec<Doc>) -> Option<Doc> {
    match body.first_mut() {
        Some(Doc::HardLine | Doc::BlankLine) => Some(body.remove(0)),
        Some(Doc::Concat(inner)) => {
            let taken = take_leading_break(inner);
            if inner.is_empty() {
                body.remove(0);
            }
            taken
        }
        _ => None,
    }
}

/// The break between two source-order siblings: nothing before the first, one
/// blank line where an empty line stands between them, else a plain line (§7.3).
fn break_between(prev: Option<&SyntaxElement>, cur: &SyntaxElement) -> Gap {
    match prev {
        None => Gap::None,
        Some(prev) if empty_line_between(prev, cur) => Gap::Blank,
        Some(_) => Gap::Hard,
    }
}

/// One frame of the term engine's explicit bracket stack (§5.6): one open
/// bracket context or one flat term node, innermost on top, so bracket nesting
/// is the stack's height and never call depth — the tier's `terms` discipline
/// (syntax.md §6.2). A frame accumulates its node's lowered children; a bracket
/// frame wraps the interior in a group on close, a flat frame concatenates it.
struct Frame {
    kind: SyntaxKind,
    element: SyntaxElement,
    children: std::vec::IntoIter<SyntaxElement>,
    /// The separator before the next significant child; the first takes the
    /// gap the frame was opened under.
    gap: Gap,
    /// The open bracket token, with its oracle floor — a bracket frame only.
    open: Vec<Doc>,
    /// The interior: the elements and their separators, grouped between the
    /// brackets (a bracket frame) or concatenated (a flat frame).
    interior: Vec<Doc>,
    /// The close bracket token, with its oracle floor — a bracket frame only.
    close: Vec<Doc>,
    /// Whether the open bracket has been seen (the abs bars share a kind).
    opened: bool,
}

impl Frame {
    /// Open a frame over `node`, its first child taking `gap`.
    fn open(node: &SyntaxNode, gap: Gap) -> Frame {
        Frame {
            kind: node.kind(),
            element: SyntaxElement::Node(node.clone()),
            children: node.children_with_tokens().collect::<Vec<_>>().into_iter(),
            gap,
            open: Vec::new(),
            interior: Vec::new(),
            close: Vec::new(),
            opened: false,
        }
    }

    /// The frame's document: a bracket frame groups its interior between its
    /// brackets; a flat frame concatenates its children (§7.2).
    fn finish(self) -> Doc {
        if is_bracket(self.kind) {
            bracket_group(self.open, self.interior, self.close)
        } else {
            Doc::Concat(self.interior)
        }
    }
}

/// Whether a frame owns a bracket pair to wrap its interior in — false for a
/// flat (non-bracket) frame (§7.2).
fn is_bracket(kind: SyntaxKind) -> bool {
    brackets(kind).is_some()
}

/// The open and close bracket kinds of a bracket frame (§7.2). The theory
/// families join the same stack — a theory tuple/function hugs parens, a list
/// hugs brackets, a set hugs braces (syntax.md §5.8).
fn brackets(kind: SyntaxKind) -> Option<(SyntaxKind, SyntaxKind)> {
    match kind {
        SyntaxKind::ARGUMENTS
        | SyntaxKind::POOL
        | SyntaxKind::THEORY_TUPLE
        | SyntaxKind::THEORY_FUNCTION => Some((SyntaxKind::L_PAREN, SyntaxKind::R_PAREN)),
        SyntaxKind::ANNOTATION | SyntaxKind::THEORY_LIST => {
            Some((SyntaxKind::L_BRACKET, SyntaxKind::R_BRACKET))
        }
        SyntaxKind::THEORY_SET => Some((SyntaxKind::L_BRACE, SyntaxKind::R_BRACE)),
        SyntaxKind::ABS_TERM => Some((SyntaxKind::PIPE, SyntaxKind::PIPE)),
        _ => None,
    }
}

/// Whether a frame separates its elements with `,`/`;` that hug the element
/// they follow (a term list), rather than spacing every child (an operator
/// chain) (§7.2).
fn is_list(kind: SyntaxKind) -> bool {
    matches!(
        kind,
        SyntaxKind::TUPLE
            | SyntaxKind::ARGUMENTS
            | SyntaxKind::POOL
            | SyntaxKind::ABS_TERM
            | SyntaxKind::ANNOTATION
            | SyntaxKind::THEORY_SET
            | SyntaxKind::THEORY_LIST
            | SyntaxKind::THEORY_TUPLE
            | SyntaxKind::THEORY_FUNCTION
    )
}

/// The gap a frame places after an element, before its next child: a theory
/// operator is always spaced (`x - y`, `- x`; greedy munch forbids `x-y`,
/// syntax.md §5.8); a term operator is spaced only at bracket depth 0 (`X + Y`,
/// but `p(X+Y)`) — kallos's depth rule (§7.2); every other join is tight, the
/// oracle floor deciding it and a list's break riding its separator.
fn element_gap(kind: SyntaxKind, depth: usize) -> Gap {
    match kind {
        SyntaxKind::THEORY_OPTERM => Gap::Space,
        SyntaxKind::BINARY_TERM if depth == 0 => Gap::Space,
        _ => Gap::Tight,
    }
}

/// The gap after a pre-brace child of an aggregate or theory definition (§7.2):
/// a guard (a node) is spaced from what follows it; a theory definition's name
/// is spaced from its brace (`#theory t {`); an aggregate keyword hugs its brace
/// (`#sum{`) and a negation lets the oracle space the keyword after it.
fn pre_brace_gap(child: &SyntaxElement) -> Gap {
    match child {
        SyntaxElement::Node(_) => Gap::Space,
        SyntaxElement::Token(token) if token.kind() == SyntaxKind::IDENT => Gap::Space,
        SyntaxElement::Token(_) => Gap::Tight,
    }
}

/// Wrap a body interior in a group that nests its breaks one level — flat when
/// it fits, one element per line when it does not (§7.2). A rule folds its body
/// into the whole-rule group directly (see `rule`); this serves a standalone
/// body, a directive's condition.
///
/// Unlike [`as_group`], this does not hoist a leading unconditional break out of
/// the group, so `inner` must not begin with one: a `HardLine`/`BlankLine` inside
/// a group makes it never fit, exploding what would sit on one line (§6). Its
/// callers pass body-position interiors, whose first gap is `Soft` — a width
/// break, not a hard one — so the precondition holds; a statement-initial
/// construct, carrying a hard inter-statement break, must go through [`as_group`].
fn group_nest(inner: Vec<Doc>) -> Doc {
    Doc::Group(Box::new(Doc::Nest(Box::new(Doc::Concat(inner)))))
}

/// Assemble a bracketed group (§7.2): the open bracket, the interior nested one
/// level and led by the interior break, that same break at the enclosing
/// indent, and the close bracket — flat when it fits, one element per line when
/// it does not. `make_break` builds a fresh break each side (`Doc` is not
/// `Clone`).
fn grouped(
    open: Vec<Doc>,
    interior: Vec<Doc>,
    close: Vec<Doc>,
    make_break: impl Fn() -> Doc,
) -> Doc {
    if interior.is_empty() {
        // Empty brackets abut, with no interior break to double — `()`, `{}`.
        let mut whole = open;
        whole.extend(close);
        return Doc::Concat(whole);
    }
    let mut nested = Vec::with_capacity(interior.len() + 1);
    // A leading comment on the first element already opens the interior with its
    // own hard break; the structural break would stack with it into a blank line
    // the source never had (§7.3, §8.2). Let that break stand alone.
    if !starts_with_hard_break(&interior) {
        nested.push(make_break());
    }
    nested.extend(interior);
    let mut whole = open;
    whole.push(Doc::Nest(Box::new(Doc::Concat(nested))));
    // The same guard on the close side: a dangling line comment ending the
    // interior owes the closer a break (it leads the close slot), which stands in
    // for the structural one — adding both stacks a blank before the closer
    // (§7.3, §8.2).
    if !starts_with_hard_break(&close) {
        whole.push(make_break());
    }
    whole.extend(close);
    as_group(whole)
}

/// Whether these lowered children begin with an unconditional break — a comment
/// woven on its own line ahead of them (§8.2). Such a break stands in for a
/// bracket group's structural break, which would otherwise double it into a
/// blank line the source never had (§7.3): it holds of the interior (a leading
/// comment on the first element) and of the closer (a dangling comment ending
/// the interior, whose owed break leads the close slot). An empty separator
/// (bare adjacency, §5.5) renders nothing, so the first break past any is what
/// the run begins with.
fn starts_with_hard_break(docs: &[Doc]) -> bool {
    docs.iter()
        .find(|doc| !matches!(doc, Doc::Concat(inner) if inner.is_empty()))
        .is_some_and(|doc| matches!(doc, Doc::HardLine | Doc::BlankLine))
}

/// A hugging group (§7.2): the break is a `SoftLine`, so the paren, pool, and
/// absolute-value families hug their brackets (nothing when flat) and explode
/// only when they must.
fn bracket_group(open: Vec<Doc>, interior: Vec<Doc>, close: Vec<Doc>) -> Doc {
    grouped(open, interior, close, || Doc::SoftLine)
}

/// A spaced group (§7.2): the break is a `Line`, so the aggregate and optimize
/// families keep a space inside their braces when flat (`#count{ E1; E2 }`) and
/// explode one element per line when they do not fit.
fn brace_group(open: Vec<Doc>, interior: Vec<Doc>, close: Vec<Doc>) -> Doc {
    grouped(open, interior, close, || Doc::Line)
}
