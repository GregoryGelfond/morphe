//! Idempotence and the certificate grant as laws (docs/design/morphe.md §5.4,
//! §13): over every member of the corpus — comment-bearing included — `format`
//! returns a granted certificate and `format(format(x)) == format(x)`. Every
//! member is checked deterministically, so a member that grants yet reshapes on
//! re-format — a non-idempotent layout the grant alone would pass — is caught,
//! not merely sampled.

use morphe::{Certificate, FormatOptions, format};
use morphe_test_support::{cases, source};

#[test]
fn every_member_is_granted_and_idempotent() {
    let mut checked = 0;
    for case in cases().into_iter().filter(|case| case.is_member) {
        let first = format(&source(&case.text), &FormatOptions::default())
            .unwrap_or_else(|err| panic!("{}: {err:?}", case.path.display()));
        assert!(
            matches!(
                first.certificate,
                Certificate::LayoutOnly | Certificate::UpToSpelling
            ),
            "{}",
            case.path.display()
        );
        let second = format(&source(&first.text), &FormatOptions::default())
            .unwrap_or_else(|err| panic!("{} (re-format): {err:?}", case.path.display()));
        assert_eq!(first.text, second.text, "{}", case.path.display()); // §5.4
        checked += 1;
    }
    assert!(checked > 0, "the corpus has no member to check");
}
