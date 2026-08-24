//! The document IR and its renderer (docs/design/morphe.md §6): the small
//! Wadler/Lindig document morphe lowers the AST into, and the width-driven,
//! explicit-stack renderer that lays it out. The renderer is oracle-oblivious
//! — `emit` fixes every significant-token adjacency into the document at build
//! time, and breaking a group only strengthens a separator (§5.5).

use std::borrow::Cow;

/// The indent unit — kallos's private `4`, named here (§6, §10.4). The one
/// layout constant the renderer holds; morphe carries no neck-width constant,
/// a broken continuation being indented by `INDENT` like any other break.
pub(crate) const INDENT: usize = 4;

/// A layout-independent document. Its text is owned: rowan's token text is
/// cursor-scoped, not tree-lifetime, so the walk owns each spelling as it
/// lowers (§6; §16 reserves a borrowing `Doc<'a>` for a future tier accessor).
/// Rendered to text at a target width by `render`, which chooses each group
/// flat or broken.
// The full §6 IR. The house style constructs every variant save `SoftLine` (see
// there); `render` handles all eight.
pub(crate) enum Doc {
    /// Verbatim text — one token's owned spelling, a fixed separator (a
    /// `'static` literal, borrowed without allocating), or a whole verbatim
    /// region (a doc line, a script body). Any newlines it contains are
    /// *content*, not layout: `render` emits them exactly and advances the
    /// column past the last one.
    Text(Cow<'static, str>),
    /// A soft break: a space when its group is flat, a newline (at the current
    /// indent) when broken.
    Line,
    /// A soft break that is *nothing* when flat, a newline when broken — the
    /// break a bracketed construct places just inside its `(`/`}` so it explodes
    /// nothing-flat, newline-broken around its contents (§5.5, §7.2). The oracle
    /// floor sets tight adjacencies bare, never soft (§5.5), so this is the
    /// house style's own node, constructed by the term families.
    SoftLine,
    /// An unconditional newline — for a line comment, a doc line, one
    /// statement ending before the next begins.
    HardLine,
    /// A blank line kept as one (§7.3): two newlines when it stands.
    BlankLine,
    /// Increase the indent of everything within by one level.
    Nest(Box<Doc>),
    /// A choice point: render flat if it fits the remaining width, otherwise
    /// broken.
    Group(Box<Doc>),
    /// Concatenation.
    Concat(Vec<Doc>),
}

/// Whether a group renders on one line or breaks (§6). A group's children
/// inherit its choice; `HardLine`/`BlankLine` break regardless.
#[derive(Clone, Copy)]
enum Mode {
    Flat,
    Break,
}

/// Render `doc` at the target `width`, laying each group flat if it fits the
/// remaining columns and broken otherwise (§6). Explicit-stack, never
/// recursive on a `Doc` child (§5.6); cost linear in the document.
pub(crate) fn render(doc: &Doc, width: usize) -> String {
    let mut out = String::new();
    let mut col = 0usize;
    // The root renders flat: a soft break with no enclosing group takes its
    // flat form (a `SoftLine` → nothing, a `Line` → space), there being no
    // group to break it. Statement separation is `HardLine`s (unconditional);
    // a group within computes its own mode by `fits`. This is what lets an
    // ungrouped lowering and a bare separator (§5.5) render flat.
    let mut stack: Vec<(usize, Mode, &Doc)> = vec![(0, Mode::Flat, doc)];
    while let Some((indent, mode, node)) = stack.pop() {
        match node {
            Doc::Text(s) => {
                out.push_str(s);
                // A content newline resets the tracked column to the width of
                // the final line; otherwise the column advances past the text.
                col = match s.rfind('\n') {
                    Some(nl) => s[nl + 1..].chars().count(),
                    None => col + s.chars().count(),
                };
            }
            Doc::Line => match mode {
                Mode::Flat => {
                    out.push(' ');
                    col += 1;
                }
                Mode::Break => col = newline_indent(&mut out, indent),
            },
            Doc::SoftLine => {
                if let Mode::Break = mode {
                    col = newline_indent(&mut out, indent);
                }
            }
            Doc::HardLine => col = newline_indent(&mut out, indent),
            Doc::BlankLine => {
                out.push('\n');
                col = newline_indent(&mut out, indent);
            }
            Doc::Nest(inner) => stack.push((indent + INDENT, mode, inner)),
            Doc::Group(inner) => {
                let mode = if fits(width, col, inner) {
                    Mode::Flat
                } else {
                    Mode::Break
                };
                stack.push((indent, mode, inner));
            }
            Doc::Concat(items) => {
                for item in items.iter().rev() {
                    stack.push((indent, mode, item));
                }
            }
        }
    }
    out
}

/// Emit a newline and `indent` spaces, returning the new column.
fn newline_indent(out: &mut String, indent: usize) -> usize {
    out.push('\n');
    for _ in 0..indent {
        out.push(' ');
    }
    indent
}

/// Whether the group `doc`, laid out flat from column `start`, stays within
/// `width` on every line (§6). A `HardLine`/`BlankLine` within forces the
/// break, so the group cannot be flat; a content newline in a `Text` resets
/// the line, since it is content the group does not reflow (§5.5, §7.3).
/// Explicit-stack, never recursive (§5.6); a bounded forward scan.
fn fits(width: usize, start: usize, doc: &Doc) -> bool {
    let mut col = start;
    let mut stack: Vec<&Doc> = vec![doc];
    while let Some(node) = stack.pop() {
        match node {
            Doc::Text(s) => {
                for ch in s.chars() {
                    if ch == '\n' {
                        col = 0;
                    } else {
                        col += 1;
                        if col > width {
                            return false;
                        }
                    }
                }
            }
            Doc::Line => {
                col += 1;
                if col > width {
                    return false;
                }
            }
            Doc::SoftLine => {}
            Doc::HardLine | Doc::BlankLine => return false,
            Doc::Nest(inner) | Doc::Group(inner) => stack.push(inner),
            Doc::Concat(items) => {
                for item in items.iter().rev() {
                    stack.push(item);
                }
            }
        }
    }
    col <= width
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    fn text(s: &'static str) -> Doc {
        Doc::Text(Cow::Borrowed(s))
    }

    #[test]
    fn a_group_renders_flat_when_it_fits_and_broken_when_it_does_not() {
        // "p(" a "," Line b ")" — Line is a space flat, a newline broken.
        let g = Doc::Group(Box::new(Doc::Concat(vec![
            text("p("),
            text("a"),
            text(","),
            Doc::Line,
            text("b"),
            text(")"),
        ])));
        assert_eq!(render(&g, 100), "p(a, b)");
        assert_eq!(render(&g, 4), "p(a,\nb)");
    }

    #[test]
    fn nest_indents_every_break_within_by_one_level() {
        let g = Doc::Group(Box::new(Doc::Concat(vec![
            text("h :-"),
            Doc::Nest(Box::new(Doc::Concat(vec![
                Doc::Line,
                text("a,"),
                Doc::Line,
                text("b."),
            ]))),
        ])));
        assert_eq!(
            render(&g, 4),
            format!("h :-\n{0}a,\n{0}b.", " ".repeat(INDENT))
        );
    }

    #[test]
    fn softline_is_nothing_flat_and_a_newline_broken() {
        let g = Doc::Group(Box::new(Doc::Concat(vec![
            text("0"),
            Doc::SoftLine,
            text("x1"),
        ])));
        assert_eq!(render(&g, 100), "0x1");
    }

    #[test]
    fn hardline_and_blankline_are_unconditional() {
        let d = Doc::Concat(vec![
            text("a."),
            Doc::HardLine,
            text("b."),
            Doc::BlankLine,
            text("c."),
        ]);
        assert_eq!(render(&d, 100), "a.\nb.\n\nc.");
    }

    #[test]
    fn a_text_region_carrying_newlines_is_content_and_resets_the_column() {
        // A verbatim region (a #script body): its newlines are emitted exactly
        // and the tracked column is the width of its last line.
        let body = Doc::Text(Cow::Owned("def f():\n    return 1".to_owned()));
        let d = Doc::Group(Box::new(Doc::Concat(vec![
            body,
            Doc::SoftLine,
            text("#end"),
        ])));
        // The group's flat width is dominated by content it cannot reflow;
        // #end still follows immediately (SoftLine → nothing) because the
        // column after the body's last line ("    return 1") is what counts.
        assert_eq!(render(&d, 100), "def f():\n    return 1#end");
    }
}
