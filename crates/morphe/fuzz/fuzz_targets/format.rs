#![no_main]

use libfuzzer_sys::fuzz_target;
use morphe::themelios_syntax::base::source::{Source, SourceId};
use morphe::{Certificate, Dialect, FormatError, FormatOptions, format};

fn make_source(text: &str) -> Option<Source> {
    Source::new(SourceId::new(0), text.to_owned()).ok()
}

fuzz_target!(|data: &[u8]| {
    let Ok(text) = std::str::from_utf8(data) else {
        return;
    };
    for dialect in [Dialect::Clingo, Dialect::AspCore2] {
        let opts = FormatOptions {
            dialect,
            ..FormatOptions::default()
        };
        let Some(src) = make_source(text) else {
            return;
        };
        match format(&src, &opts) {
            // A member formats to a granted, re-parsing, idempotent output: the
            // certificate is always on, so the output re-parses to the same
            // program and re-formats to itself (§5.3, §5.4).
            Ok(out) => {
                let Some(again_src) = make_source(&out.text) else {
                    return;
                };
                let reparsed = format(&again_src, &opts);
                assert!(
                    matches!(&reparsed, Ok(again) if again.text == out.text),
                    "idempotence"
                );
                assert!(matches!(
                    out.certificate,
                    Certificate::LayoutOnly | Certificate::UpToSpelling
                ));
            }
            Err(FormatError::HasParseErrors(_)) => {} // a non-member, refused (§5.7)
            // A member morphe could not certify is a bug, never the input's fault
            // (§5.3). The whole corpus is granted (§13), so the shipped target
            // enforces the grant rather than tolerating its breach: a break here
            // fails the fuzz run with the tier's witness.
            Err(FormatError::CertificateBroke { reason, .. }) => {
                panic!("a member surfaced a certificate break: {reason:?}")
            }
        }
    }
});
