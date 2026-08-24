//! The trailing comma, preserved as authored (docs/design/morphe.md §7.5,
//! docs/design/syntax.md §8.2): a tuple's trailing comma is a real token the
//! format keeps and hugs to the last element — never synthesized, never
//! stripped — so width alone decides the break. A tuple explodes when it does
//! not fit and stays flat, comma and all, when it does; a one-tuple `(a,)` is
//! arity-distinct from the parenthesized `(a)`, its comma meaning-bearing, so
//! it is preserved verbatim. Exercised across widths, off the width-100 golden
//! harness.

use morphe::{FormatOptions, format};
use morphe_test_support::source;

fn at(width: usize, text: &str) -> String {
    let opts = FormatOptions {
        line_width: width,
        ..FormatOptions::default()
    };
    format(&source(text), &opts).expect("a member").text
}

#[test]
fn a_trailing_comma_hugs_the_last_element_when_the_tuple_fits() {
    // The redundant trailing comma is kept and hugged; the tuple stays flat
    // because it fits — the comma is layout the format reads, not a forced
    // break (§7.5).
    assert_eq!(at(100, "p(X) :- X = (a,b,).\n"), "p(X) :- X = (a, b,).\n");
}

#[test]
fn a_trailing_comma_tuple_explodes_only_when_it_does_not_fit() {
    // Narrow: the tuple explodes one term per line, the trailing comma still
    // hugging the last term — the same break a comma-free tuple takes by width.
    assert_eq!(
        at(14, "p(X) :- X = (a, b, c,).\n"),
        "p(X) :-\n    X = (\n        a,\n        b,\n        c,\n    ).\n"
    );
}

#[test]
fn a_one_tuple_comma_is_meaning_bearing_and_preserved() {
    // `(a,)` is a one-tuple, arity-distinct from the parenthesized `(a)`
    // (syntax.md §8.2); the comma is meaning, preserved verbatim and never
    // exploded, its counterpart `(a)` left untouched.
    assert_eq!(at(100, "p(X) :- X = (a,).\n"), "p(X) :- X = (a,).\n");
    assert_eq!(at(100, "p(X) :- X = (a).\n"), "p(X) :- X = (a).\n");
}

#[test]
fn the_trailing_comma_layout_is_idempotent() {
    // The exploded layout re-formats to itself — the certificate's every-token
    // preservation carries the comma through unchanged (§5.4).
    let once = at(14, "p(X) :- X = (a, b, c,).\n");
    assert_eq!(at(14, &once), once);
}
