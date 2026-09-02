//! The `format` / `format_parsed` spine (docs/design/morphe.md §5.1): a
//! comment-free member formats and is granted a certificate; a non-member is
//! refused, not partially formatted (§5.7); and `format_parsed` reads the
//! dialect from the `Parse`, not from the options.

use morphe::themelios_syntax::base::source::SourceId;
use morphe::themelios_syntax::parse::parse;
use morphe::{
    Certificate, Dialect, FormatError, FormatOptions, Parse, Program, Source, format, format_parsed,
};

fn source(text: &str) -> Source {
    Source::new(SourceId::new(0), text.to_owned()).expect("a small test source")
}

fn parse_at(dialect: Dialect, text: &str) -> Parse<Program> {
    parse(&source(text), dialect)
}

#[test]
fn a_comment_free_member_is_formatted_and_granted() {
    let src = source("p(X):-q(X).\n");
    let out = format(&src, &FormatOptions::default()).expect("a member formats");
    assert_eq!(out.certificate, Certificate::UpToSpelling); // default canonicalizes
    assert!(out.text.ends_with('\n')); // §7.1 a trailing newline
    assert!(!out.embedded_changed); // no embedded pass yet (§9)
}

#[test]
fn a_non_member_is_refused_not_formatted() {
    let src = source("p(X :- q(X).\n"); // an unbalanced paren
    match format(&src, &FormatOptions::default()) {
        Err(FormatError::HasParseErrors(diags)) => assert!(!diags.is_empty()),
        other => panic!("expected HasParseErrors, got {other:?}"),
    }
}

#[test]
fn format_parsed_reads_the_dialect_from_the_parse() {
    let parsed = parse_at(Dialect::AspCore2, "p.\n");
    // options.dialect is Clingo but the parse is AspCore2; format_parsed uses the parse's.
    let opts = FormatOptions {
        dialect: Dialect::Clingo,
        ..FormatOptions::default()
    };
    assert!(format_parsed(&parsed, &opts).is_ok());
}
