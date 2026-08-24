//! The corpus harness (docs/design/morphe.md §13): over the vendored corpus,
//! every member is formatted and granted a certificate — comment-bearing
//! members included, their comments woven back to the same anchors (§8) — and
//! every non-member is refused (§5.7). This whole-corpus grant is the §13 law
//! the shipped fuzz target now enforces.

use morphe::{Certificate, FormatError, format};
use morphe_test_support::{cases, options, source};

#[test]
fn every_member_is_formatted_and_granted() {
    let mut granted = 0;
    for case in cases().into_iter().filter(|case| case.is_member) {
        let out = format(&source(&case.text), &options(case.dialect))
            .unwrap_or_else(|err| panic!("{}: {err:?}", case.path.display()));
        assert!(
            matches!(
                out.certificate,
                Certificate::LayoutOnly | Certificate::UpToSpelling
            ),
            "{}",
            case.path.display()
        );
        granted += 1;
    }
    assert!(granted > 0, "the corpus has no member to grant");
}

#[test]
fn every_non_member_is_refused_not_formatted() {
    let mut refused = 0;
    for case in cases().into_iter().filter(|case| !case.is_member) {
        match format(&source(&case.text), &options(case.dialect)) {
            Err(FormatError::HasParseErrors(diagnostics)) => assert!(!diagnostics.is_empty()),
            other => panic!(
                "{}: expected HasParseErrors, got {other:?}",
                case.path.display()
            ),
        }
        refused += 1;
    }
    assert!(refused > 0, "the corpus has no non-member to refuse");
}

#[test]
fn the_corpus_has_comment_bearing_members() {
    // The comment-torture shape (§3.3) is the adversary the whole-corpus grant
    // must clear — members whose comments are woven back to their anchors and
    // slots (§8). Their presence gives `every_member_is_formatted_and_granted`
    // its teeth.
    let comment_bearing = cases()
        .into_iter()
        .filter(|case| case.is_member && !case.comment_free)
        .count();
    assert!(
        comment_bearing > 0,
        "the corpus has no comment-bearing member"
    );
}
