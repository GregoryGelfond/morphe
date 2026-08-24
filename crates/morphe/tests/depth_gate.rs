//! Depth safety at `DEFAULT` on an ordinary thread (docs/design/morphe.md §5.6).
//! morphe enters through the file door `parse`, baked at `NestingLimit::DEFAULT`
//! (128 frames), so a program nested to the limit is a member and formats —
//! built, walked, and dropped on an ordinary stack, no `with_required_stack` and
//! no 64 MiB thread — while one nested past the limit is refused
//! (`nesting-too-deep`) as a non-member (§5.7), never formatted. `format_parsed`
//! holds two `DEFAULT`-deep trees at once (the input and the re-parsed skeleton)
//! on the same ordinary stack. Exercised on a modest thread, never the tier's
//! required-stack one.

use std::thread;

use morphe::themelios_syntax::diagnostic::SyntaxErrorKind;
use morphe::themelios_syntax::parse::{NestingLimit, parse};
use morphe::{Certificate, Dialect, FormatError, FormatOptions, format, format_parsed};
use morphe_test_support::source;

/// A modest stack — the crash-averse floor a naive consumer holds (§5.6), the
/// two mebibytes the tier proves `DEFAULT` safe on; never the tier's 64 MiB
/// required-stack thread, which morphe never owes (§5.6).
const ORDINARY_STACK_BYTES: usize = 2 * 1024 * 1024;

/// A fact nested `depth` bracket frames deep — `f(f(… f(x) …)).` — one frame per
/// open paren (syntax.md §6.2), the genuine depth source (§5.6).
fn nested_fact(depth: usize) -> String {
    format!("{}x{}.\n", "f(".repeat(depth), ")".repeat(depth))
}

/// Run `work` on a fresh modest-stack thread, returning its value; a stack
/// overflow ends the thread and fails the join, so the gate catches it (§5.6).
fn on_ordinary_stack<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
    thread::Builder::new()
        .stack_size(ORDINARY_STACK_BYTES)
        .spawn(work)
        .expect("spawn a modest-stack thread")
        .join()
        .expect("the walk completes on the ordinary stack, no overflow")
}

#[test]
fn a_program_at_the_default_limit_formats_on_an_ordinary_stack() {
    let text = nested_fact(NestingLimit::DEFAULT.frames() as usize);
    let out = on_ordinary_stack(move || format(&source(&text), &FormatOptions::default()));
    let formatted = out.expect("the limit is a member and formats");
    assert!(matches!(
        formatted.certificate,
        Certificate::LayoutOnly | Certificate::UpToSpelling
    ));
}

#[test]
fn a_program_past_the_default_limit_is_refused_not_formatted() {
    let text = nested_fact(NestingLimit::DEFAULT.frames() as usize + 1);
    match format(&source(&text), &FormatOptions::default()) {
        Err(FormatError::HasParseErrors(diagnostics)) => assert!(
            diagnostics
                .iter()
                .any(|d| matches!(d.kind(), SyntaxErrorKind::NestingTooDeep { .. })),
            "refused for nesting depth (§5.6)"
        ),
        other => panic!("expected a nesting refusal, got {other:?}"),
    }
}

#[test]
fn format_parsed_holds_two_default_deep_trees_on_an_ordinary_stack() {
    let text = nested_fact(NestingLimit::DEFAULT.frames() as usize);
    let out = on_ordinary_stack(move || {
        // The `DEFAULT`-built `Parse` the precondition names (§5.6); the pipeline
        // re-parses the skeleton to certify, so two `DEFAULT`-deep trees are held
        // and dropped at once, on the ordinary stack.
        let parsed = parse(&source(&text), Dialect::Clingo);
        format_parsed(&parsed, &FormatOptions::default())
    });
    let formatted = out.expect("a DEFAULT-built parse at the limit formats");
    assert!(matches!(
        formatted.certificate,
        Certificate::LayoutOnly | Certificate::UpToSpelling
    ));
}
