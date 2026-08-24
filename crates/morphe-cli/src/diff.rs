//! The `--diff` unified diff (docs/design/morphe.md §10.1): render the change
//! from an input to its formatted text as a unified diff for the user to read.
//! It is advisory output, never part of the certificate. This is the only
//! module that names any `similar` type, so a version move or a swap of the
//! diff engine touches this file alone (§12).

use similar::TextDiff;

use crate::color::{CYAN, GREEN, RED, RESET};

/// A unified diff from `original` to `formatted`, headed with the two labels
/// (the input's path, or a stdin marker). The empty string when the texts are
/// equal; ANSI-coloured line by line when `colored`.
pub(crate) fn unified(
    original: &str,
    formatted: &str,
    original_label: &str,
    formatted_label: &str,
    colored: bool,
) -> String {
    let plain = TextDiff::from_lines(original, formatted)
        .unified_diff()
        .header(original_label, formatted_label)
        .to_string();
    if colored { colorize(&plain) } else { plain }
}

/// Colour a unified diff by line prefix (§10.1): a `+`/`-` line is an addition
/// or deletion, a `@@` line a hunk header, and the `+++`/`---` file headers stay
/// plain. Each coloured line's content is wrapped and reset, the line break left
/// outside the colour so a copied diff carries no stray codes.
fn colorize(diff: &str) -> String {
    let mut out = String::with_capacity(diff.len());
    for line in diff.split_inclusive('\n') {
        let (content, newline) = match line.strip_suffix('\n') {
            Some(content) => (content, "\n"),
            None => (line, ""),
        };
        let color = if content.starts_with("@@") {
            CYAN
        } else if content.starts_with("+++") || content.starts_with("---") {
            ""
        } else if content.starts_with('+') {
            GREEN
        } else if content.starts_with('-') {
            RED
        } else {
            ""
        };
        if color.is_empty() {
            out.push_str(content);
        } else {
            out.push_str(color);
            out.push_str(content);
            out.push_str(RESET);
        }
        out.push_str(newline);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_colored_diff_wraps_changes_in_ansi_and_resets_them() {
        let colored = unified("a\n", "b\n", "x", "x", true);
        assert!(colored.contains("\u{1b}[32m"), "an addition is green");
        assert!(colored.contains("\u{1b}[31m"), "a deletion is red");
        assert!(colored.contains("\u{1b}[0m"), "each colour is reset");
    }

    #[test]
    fn an_uncolored_diff_carries_no_ansi() {
        let plain = unified("a\n", "b\n", "x", "x", false);
        assert!(!plain.contains('\u{1b}'), "no ANSI when colour is off");
        assert!(plain.contains("+b"), "the change is still shown");
    }
}
