//! The ASP-Core-2 query (docs/design/morphe.md §7.1, §15.4): its trailing `?`
//! hugs its atom like the `.` terminator, and it is the program's final
//! statement. Read under the ASP-Core-2 dialect, where `?` is the query
//! terminator; in clingo it is the bitwise-or operator, so the query has no
//! clingo golden and is exercised here instead.

use morphe::{Dialect, format};
use morphe_test_support::{options, source};

fn formatted(text: &str) -> String {
    format(&source(text), &options(Dialect::AspCore2))
        .expect("a member")
        .text
}

#[test]
fn the_query_terminator_hugs_its_atom() {
    // The `?` hugs its atom, like the `.` terminator (§7.1).
    assert_eq!(formatted("a?\n"), "a?\n");
    // The atom's own arguments still take the house style around it.
    assert_eq!(formatted("a( X )?\n"), "a(X)?\n");
    // Pinned last, on its own line after the statements that precede it.
    assert_eq!(formatted("p:-q. a?\n"), "p :- q.\na?\n");
}

#[test]
fn a_comment_among_a_querys_doc_lines_keeps_source_order() {
    // A plain comment interleaved with a query's `%!` documentation lines stays
    // in source order: the query lowers through the generic child walk, which
    // emits a pre-statement comment where it meets it rather than deferring it
    // past the docs — which would move it across a token and break the
    // certificate (§8.2, §5.2).
    assert_eq!(
        formatted("%! documented\n% a plain comment\n%! more docs\na?\n"),
        "%! documented\n% a plain comment\n%! more docs\na?\n"
    );
}
