//! Colour policy (docs/design/morphe.md §10.1): the `--color` flag selects
//! `auto` (the default), `always`, or `never`. Under `auto` colour is emitted
//! only to a terminal, and never when the `NO_COLOR` environment variable is set
//! (no-color.org); `always` and `never` force the choice, the explicit flag
//! overriding the environment. base renders morphe's diagnostics through a
//! deliberately colourless human view (§11.1); this module is where the CLI
//! paints its terminal presentation on top — the `--diff` (`diff.rs`) and the
//! diagnostics ([`diagnostics`]) — leaving the canonical text every golden
//! captures untouched.

/// The ANSI SGR codes the CLI paints with — named once here so a colour change
/// is one edit. `RESET` closes any of them.
pub(crate) const RESET: &str = "\u{1b}[0m";
pub(crate) const RED: &str = "\u{1b}[31m";
pub(crate) const GREEN: &str = "\u{1b}[32m";
pub(crate) const YELLOW: &str = "\u{1b}[33m";
pub(crate) const BLUE: &str = "\u{1b}[34m";
pub(crate) const CYAN: &str = "\u{1b}[36m";
const BOLD: &str = "\u{1b}[1m";

/// The `--color` choice (§10.1): follow the terminal, or force colour on or off.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, clap::ValueEnum)]
pub(crate) enum ColorChoice {
    /// Colour a terminal, honouring `NO_COLOR`; plain otherwise.
    #[default]
    Auto,
    /// Colour whatever the stream, `NO_COLOR` included.
    Always,
    /// Never colour.
    Never,
}

/// Whether to emit colour to a stream, given the `--color` choice and whether
/// that stream is a terminal. Reads `NO_COLOR` from the environment and defers
/// the decision to [`resolve`], so the policy is pure and tested there without
/// touching process-global state.
pub(crate) fn enabled(choice: ColorChoice, stream_is_tty: bool) -> bool {
    resolve(
        choice,
        stream_is_tty,
        std::env::var_os("NO_COLOR").is_some(),
    )
}

/// The colour policy as a pure function of the choice, whether the stream is a
/// terminal, and whether `NO_COLOR` is set (to any value — the empty string
/// included, no-color.org). Under `Auto`, colour needs a terminal and an unset
/// `NO_COLOR`; an explicit `Always` or `Never` overrides both.
fn resolve(choice: ColorChoice, stream_is_tty: bool, no_color: bool) -> bool {
    match choice {
        ColorChoice::Always => true,
        ColorChoice::Never => false,
        ColorChoice::Auto => stream_is_tty && !no_color,
    }
}

/// Paint a diagnostic already rendered by base's human view (§11.1), by line:
/// the `severity[id]:` header bold in the severity's colour, the primary `^`
/// underline (and its message) in that colour, a secondary `-` underline blue;
/// the source snippet, gutters, and locations stay plain. The rendered text is
/// unchanged apart from the added SGR codes, so the canonical output the goldens
/// hold is what remains once colour is stripped.
pub(crate) fn diagnostics(rendered: &str) -> String {
    let mut out = String::with_capacity(rendered.len() + rendered.len() / 8);
    // The colour of the diagnostic currently being painted; set by each header
    // and reused by that diagnostic's primary underlines.
    let mut severity = RED;
    for line in rendered.split_inclusive('\n') {
        let (content, newline) = match line.strip_suffix('\n') {
            Some(content) => (content, "\n"),
            None => (line, ""),
        };
        if let Some(color) = header_severity(content) {
            severity = color;
            paint_header(&mut out, content, color);
        } else if let Some((gutter, body)) = split_gutter(content) {
            match body.trim_start().as_bytes().first() {
                // A primary underline (and its message) in the severity colour.
                Some(b'^') => paint(&mut out, gutter, severity, body),
                // A secondary underline points elsewhere; blue, apart from the
                // severity so the eye separates them.
                Some(b'-') => paint(&mut out, gutter, BLUE, body),
                // A source line or a bare gutter: no colour.
                _ => out.push_str(content),
            }
        } else {
            out.push_str(content);
        }
        out.push_str(newline);
    }
    out
}

/// The severity colour of a header line — one base's human view opens each
/// diagnostic with (`severity[id]: message`) — or `None` for any other line.
fn header_severity(content: &str) -> Option<&'static str> {
    if content.starts_with("error[") {
        Some(RED)
    } else if content.starts_with("warning[") {
        Some(YELLOW)
    } else if content.starts_with("note[") {
        Some(CYAN)
    } else {
        None
    }
}

/// Paint `severity[id]:` bold in `color`, leaving the headline message plain.
fn paint_header(out: &mut String, content: &str, color: &str) {
    match content.find("]:") {
        Some(end) => {
            let (code, message) = content.split_at(end + 2);
            out.push_str(BOLD);
            out.push_str(color);
            out.push_str(code);
            out.push_str(RESET);
            out.push_str(message);
        }
        None => paint(out, "", color, content),
    }
}

/// A gutter line split at its `| ` divider into the gutter (with the divider)
/// and the content after it, or `None` when the line has no gutter.
fn split_gutter(content: &str) -> Option<(&str, &str)> {
    content.find("| ").map(|bar| content.split_at(bar + 2))
}

/// Write `prefix`, then `body` wrapped in `color` and reset.
fn paint(out: &mut String, prefix: &str, color: &str, body: &str) {
    out.push_str(prefix);
    out.push_str(color);
    out.push_str(body);
    out.push_str(RESET);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One error diagnostic in base's human-view shape, and one warning header.
    const ERROR: &str = "\
error[syntax::unexpected-token]: expected `.`
 --> f.lp:1:1
  |
1 | p(
  |  - to close this `(`
  | ^ the input ends here
";

    #[test]
    fn an_error_header_and_its_caret_take_the_error_colour() {
        let painted = diagnostics(ERROR);
        assert!(painted.contains(RED), "the error header/caret is red");
        assert!(painted.contains(BOLD), "the header is bold");
        assert!(painted.contains(BLUE), "the secondary underline is blue");
        assert!(painted.contains(RESET), "each colour is reset");
        assert!(
            painted.contains("error[syntax::unexpected-token]"),
            "the header text survives"
        );
        assert!(painted.contains("p("), "the source line survives");
    }

    #[test]
    fn a_warning_header_takes_the_warning_colour() {
        let painted = diagnostics("warning[morphe::embedded-not-formatted]: kept verbatim\n");
        assert!(painted.contains(YELLOW), "a warning header is yellow");
    }

    #[test]
    fn the_colour_policy_follows_the_choice_the_stream_and_no_color() {
        use ColorChoice::{Always, Auto, Never};
        // `Always`/`Never` force the choice, whatever the stream or `NO_COLOR`.
        assert!(
            resolve(Always, false, true),
            "always paints even piped with NO_COLOR set"
        );
        assert!(
            !resolve(Never, true, false),
            "never stays plain even on a terminal"
        );
        // `Auto` follows the terminal...
        assert!(resolve(Auto, true, false), "auto paints a terminal");
        assert!(!resolve(Auto, false, false), "auto leaves a pipe plain");
        // ...and honours `NO_COLOR` on a terminal (no-color.org) — the branch the
        // integration suite's piped (never-a-terminal) streams cannot reach.
        assert!(
            !resolve(Auto, true, true),
            "auto honours NO_COLOR even on a terminal"
        );
    }
}
