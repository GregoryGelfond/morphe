//! Fit and break at narrow widths (docs/design/morphe.md §7.1, §7.2, §6): the
//! width edges where a group must explode. Exercised at chosen widths, off the
//! width-100 golden harness (as `rule_layout`), because each case turns on the
//! exact column a construct's terminating token reaches.

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
fn an_optimize_statement_counts_its_dot_in_the_fit() {
    // The terminating `.` is folded into the brace group, as a rule's dot is
    // (§7.1): at the boundary the group explodes rather than emit a line one over.
    // `#minimize{ 1@1, T : p(T) }.` is 27 chars; at width 26 it must break.
    assert_eq!(
        at(26, "#minimize{ 1@1, T : p(T) }.\n"),
        "#minimize{\n    1@1, T : p(T)\n}.\n"
    );
    // One column wider, the whole statement — dot included — fits flat.
    assert_eq!(
        at(27, "#minimize{ 1@1, T : p(T) }.\n"),
        "#minimize{ 1@1, T : p(T) }.\n"
    );
}

#[test]
fn a_broken_rule_body_explodes_a_bare_set_element_onto_its_own_line() {
    // When the rule breaks, every body element takes its own line (§7.2). A bare
    // set `{ b }` is a body element like any other: its leading break belongs to
    // the rule's group, not to the set's own — so it breaks with the rule rather
    // than re-fitting its bracket independently and staying flat beside `a,`.
    assert_eq!(
        at(16, "long_head :- a, { b }.\n"),
        "long_head :-\n    a,\n    { b }.\n"
    );
}

#[test]
fn a_fitting_rule_body_keeps_a_bare_set_element_flat() {
    // The same body under a width that fits stays on one line — the leading break
    // is a soft `Line`, a space when its group is flat.
    assert_eq!(at(100, "long_head :- a, { b }.\n"), "long_head :- a, { b }.\n");
}
