//! Rule layout at narrow widths (docs/design/morphe.md §7.1, §7.2): the neck
//! breaks and the body drops to its own indented line when the rule does not
//! fit. The rule is one group, so a boundless choice head that explodes carries
//! the neck onto its own line (kallos's cascade); the head is itself a group, so
//! a short head with a long body stays flat while the body breaks. Exercised
//! here at chosen widths, off the width-100 golden harness.

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
fn a_boundless_choice_head_carries_the_neck_when_it_explodes() {
    assert_eq!(
        at(
            30,
            "{ assign(T,S) : slot(S); waive(T) : optional(T) } :- task(T).\n"
        ),
        "{\n    assign(T,S) : slot(S);\n    waive(T) : optional(T)\n} :-\n    task(T).\n"
    );
}

#[test]
fn a_short_head_with_a_long_body_keeps_the_head_flat() {
    // The head is its own group: a short choice stays flat while the body breaks.
    assert_eq!(
        at(18, "{ a } :- b1, b2, b3.\n"),
        "{ a } :-\n    b1,\n    b2,\n    b3.\n"
    );
}

#[test]
fn a_rule_that_fits_stays_on_one_line() {
    assert_eq!(at(100, "{ a } :- b1, b2, b3.\n"), "{ a } :- b1, b2, b3.\n");
}

#[test]
fn a_fitting_rule_stays_flat_after_a_preceding_statement() {
    // A statement's layout is independent of what precedes it: the inter-statement
    // break stands before the rule's group, not inside it, so `fits` weighs only
    // the rule (§7.1, §6). Without it, every rule after the first over-explodes.
    assert_eq!(at(100, "q.\nh :- a, b, c.\n"), "q.\nh :- a, b, c.\n");
}
