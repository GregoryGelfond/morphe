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

#[test]
fn a_whitespace_only_script_body_keeps_its_token_and_is_certified() {
    // A `#script` body of only blanks/tabs has an empty value() — the SCRIPT_BODY
    // token the certificate's interleaved sequence counts (§5.2) would vanish if
    // morphe emitted nothing between `)` and `#end`. morphe emits a single space,
    // which re-lexes to a SCRIPT_BODY whose value() is still empty: the member now
    // formats (it was refused before), is granted a certificate, and is idempotent.
    for input in [
        "#script(python) #end.\n",
        "#script(python)   #end.\n",
        "#script(python)\t#end.\n",
        "#script(lua) #end.\n",
    ] {
        let out = format(&source(input), &FormatOptions::default())
            .unwrap_or_else(|err| panic!("{input:?} should format, got {err:?}"));
        assert_eq!(out.certificate, Certificate::UpToSpelling, "{input:?}");
        // The whitespace-only body collapses to the one space that preserves the
        // token; `#end` abuts it (§7.2).
        let lang = if input.contains("lua") {
            "lua"
        } else {
            "python"
        };
        assert_eq!(out.text, format!("#script({lang}) #end.\n"), "{input:?}");
        // Idempotent: a second format finds the same text (§5.4).
        let again =
            format(&source(&out.text), &FormatOptions::default()).expect("the output re-formats");
        assert_eq!(again.text, out.text, "{input:?} not idempotent");
    }
}

#[test]
fn a_body_less_script_synthesizes_no_token() {
    // `#script(python)#end.` carries no SCRIPT_BODY token at all (nothing stands
    // between `)` and `#end`), so the whitespace-only rule must not fire: morphe
    // leaves it as it is, never synthesizing a token the input did not have.
    let input = "#script(python)#end.\n";
    let out = format(&source(input), &FormatOptions::default()).expect("a member formats");
    assert_eq!(out.text, input);
    assert!(!out.changed);
}

#[test]
fn a_whitespace_script_body_with_a_newline_is_kept_byte_exact() {
    // A whitespace body that *contains* a newline has a non-empty value() — the
    // newline is content (§6, §7.2) — so it is emitted byte-exact, never collapsed
    // to the single space an empty-value() body takes. The empty-value() collapse
    // is scoped to blanks/tabs with no newline; this pins the other side of that
    // line (a member that would refuse if the collapse over-reached).
    for input in [
        "#script(python)\n\n\n#end.\n",
        "#script(python)  \n  \n#end.\n",
    ] {
        let out = format(&source(input), &FormatOptions::default())
            .unwrap_or_else(|err| panic!("{input:?} should format, got {err:?}"));
        assert_eq!(out.text, input, "{input:?} must be byte-exact");
    }
}
