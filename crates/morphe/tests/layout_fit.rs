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
