//! The embedded-script seam laws (docs/design/morphe.md §9.3, §9.4), driven
//! through the public `format` surface with a mock backend whose outcome each
//! test selects. The real ruff/StyLua backends are not exercised here; these
//! laws hold for *any* `EmbeddedFormatter`: a claimed body is reformatted and
//! the ASP certificate still stands (§9.3 step 1); every typed failure degrades
//! to a verbatim body and a warning note without refusing the format (§9.4); a
//! splice that would disturb the ASP structure is caught and reverted (§9.3
//! step 3); and a body left verbatim on failure converges with the
//! format_embedded-off default, idempotence holding (§9.4).

use morphe::themelios_syntax::base::diagnostic::Severity;
use morphe::{
    Availability, Certificate, EmbeddedContext, EmbeddedError, EmbeddedFormatter,
    EmbeddedFormatters, FormatOptions, LanguageKey, format,
};
use morphe_test_support::source;

/// A backend whose outcome each test selects (§9.1): reformat to a fixed text,
/// hand the body back unchanged, or refuse with a chosen `EmbeddedError`. The
/// seam consults it only through `format`, so `availability` is `Available`
/// throughout — §9.4 drives every degradation through the `format` result.
struct MockBackend {
    language: LanguageKey,
    behavior: Behavior,
}

/// The outcome a `MockBackend` produces for any body it is handed.
enum Behavior {
    /// Return this text as the formatted body.
    Reformat(String),
    /// Return the body unchanged — a backend already at its fixed point.
    Passthrough,
    /// Refuse with this typed reason (§9.1).
    Refuse(EmbeddedError),
}

impl MockBackend {
    fn new(language: &str, behavior: Behavior) -> MockBackend {
        MockBackend {
            language: LanguageKey::new(language),
            behavior,
        }
    }
}

impl EmbeddedFormatter for MockBackend {
    fn language(&self) -> &LanguageKey {
        &self.language
    }

    fn availability(&self) -> Availability {
        Availability::Available
    }

    fn format(&self, body: &str, _cx: &EmbeddedContext) -> Result<String, EmbeddedError> {
        match &self.behavior {
            Behavior::Reformat(text) => Ok(text.clone()),
            Behavior::Passthrough => Ok(body.to_owned()),
            Behavior::Refuse(error) => Err(error.clone()),
        }
    }
}

/// A registry holding `mock` under the language it claims.
fn registry(mock: MockBackend) -> EmbeddedFormatters {
    let language = mock.language().clone();
    let mut registry = EmbeddedFormatters::default();
    registry.insert(language, Box::new(mock));
    registry
}

/// Options that opt into embedded formatting with `registry` installed (§9.2).
fn formatting_embedded(registry: EmbeddedFormatters) -> FormatOptions {
    FormatOptions {
        embedded: registry,
        format_embedded: true,
        ..FormatOptions::default()
    }
}

/// A program with one Python script between two facts.
const ONE_SCRIPT: &str = "a.\n#script (python)\nx=1\n#end.\nb.\n";

#[test]
fn a_claimed_body_is_reformatted_and_the_asp_certificate_stands() {
    // §9.3 step 1: the body is reformatted, embedded_changed is set, and the ASP
    // skeleton's certificate holds regardless of what the backend produced.
    let options = formatting_embedded(registry(MockBackend::new(
        "python",
        Behavior::Reformat("x = 1\n".to_owned()),
    )));
    let formatted = format(&source(ONE_SCRIPT), &options).expect("a member formats");
    assert!(formatted.embedded_changed);
    assert!(
        formatted.text.contains("x = 1"),
        "the reformatted body is spliced"
    );
    assert!(!formatted.text.contains("x=1"), "the verbatim body is gone");
    assert!(matches!(
        formatted.certificate,
        Certificate::LayoutOnly | Certificate::UpToSpelling
    ));
    assert!(
        formatted.notes.is_empty(),
        "a clean reformat records no note"
    );
}

#[test]
fn every_backend_refusal_keeps_the_body_verbatim_with_a_note() {
    // §9.4: every EmbeddedError collapses to one outcome — that body stays
    // verbatim, a warning note is recorded, and the format is not refused.
    for error in [
        EmbeddedError::Unavailable {
            reason: "the python backend is disabled".to_owned(),
        },
        EmbeddedError::Failed {
            message: "invalid syntax".to_owned(),
        },
        EmbeddedError::Unverified,
        EmbeddedError::Panicked,
    ] {
        let options = formatting_embedded(registry(MockBackend::new(
            "python",
            Behavior::Refuse(error.clone()),
        )));
        let formatted = format(&source(ONE_SCRIPT), &options).expect("a member still formats");
        assert!(
            !formatted.embedded_changed,
            "{error:?}: nothing was spliced"
        );
        assert!(
            formatted.text.contains("x=1"),
            "{error:?}: the body stayed verbatim"
        );
        assert_eq!(formatted.notes.len(), 1, "{error:?}: one note");
        assert_eq!(
            formatted.notes[0].severity(),
            Severity::Warning,
            "{error:?}: warning-level, never a refusal"
        );
    }
}

#[test]
fn an_unclaimed_language_keeps_the_body_verbatim_with_a_note() {
    // §9.4: with embedded formatting on but no backend for the language, the
    // registry refuses Unsupported — still a verbatim body and a note.
    let options = FormatOptions {
        format_embedded: true,
        ..FormatOptions::default()
    };
    let formatted = format(&source(ONE_SCRIPT), &options).expect("a member still formats");
    assert!(!formatted.embedded_changed);
    assert!(formatted.text.contains("x=1"), "the body stayed verbatim");
    assert_eq!(formatted.notes.len(), 1);
    assert_eq!(formatted.notes[0].severity(), Severity::Warning);
}

#[test]
fn one_failing_script_stops_neither_the_others_nor_the_asp() {
    // §9.4: within a file, one failed #script never stops the other scripts or
    // the ASP skeleton from formatting.
    let input = "a.\n#script (python)\nx=1\n#end.\n#script (lua)\ny=2\n#end.\nb.\n";
    let mut registry = EmbeddedFormatters::default();
    registry.insert(
        LanguageKey::new("python"),
        Box::new(MockBackend::new(
            "python",
            Behavior::Reformat("x = 1\n".to_owned()),
        )),
    );
    registry.insert(
        LanguageKey::new("lua"),
        Box::new(MockBackend::new(
            "lua",
            Behavior::Refuse(EmbeddedError::Failed {
                message: "bad chunk".to_owned(),
            }),
        )),
    );
    let formatted = format(&source(input), &formatting_embedded(registry)).expect("a member");
    assert!(
        formatted.embedded_changed,
        "the python body was reformatted"
    );
    assert!(formatted.text.contains("x = 1"), "python spliced");
    assert!(formatted.text.contains("y=2"), "lua stayed verbatim");
    assert_eq!(formatted.notes.len(), 1, "one note, for the lua failure");
    assert!(
        formatted.text.contains("a.") && formatted.text.contains("b."),
        "the ASP skeleton formatted"
    );
}

#[test]
fn a_structure_disturbing_splice_reverts_to_verbatim_with_a_note() {
    // §9.3 step 3: a backend whose output smuggles a script terminator would
    // re-lex the region early; equal_except_script_bodies catches it and that
    // body reverts to verbatim rather than corrupting the ASP program.
    let options = formatting_embedded(registry(MockBackend::new(
        "python",
        Behavior::Reformat("x = 1\n#end.\ninjected".to_owned()),
    )));
    let formatted = format(&source(ONE_SCRIPT), &options).expect("a member still formats");
    assert!(
        !formatted.embedded_changed,
        "the disturbing splice was rejected"
    );
    assert!(
        formatted.text.contains("x=1"),
        "the body reverted to verbatim"
    );
    assert!(
        !formatted.text.contains("injected"),
        "the backend output was not spliced"
    );
    assert_eq!(formatted.notes.len(), 1);
    assert_eq!(formatted.notes[0].severity(), Severity::Warning);
    assert!(matches!(
        formatted.certificate,
        Certificate::LayoutOnly | Certificate::UpToSpelling
    ));
}

#[test]
fn a_disturbing_splice_reverts_while_a_clean_sibling_still_applies() {
    // §9.3 step 3, §9.4: when the composed text fails the whole-text bind, morphe
    // attributes the disturbance to the offending body and keeps only it verbatim
    // — the clean sibling is still reformatted.
    let input = "a.\n#script (python)\nx=1\n#end.\n#script (lua)\ny=2\n#end.\nb.\n";
    let mut registry = EmbeddedFormatters::default();
    registry.insert(
        LanguageKey::new("python"),
        Box::new(MockBackend::new(
            "python",
            Behavior::Reformat("x = 1\n".to_owned()),
        )),
    );
    registry.insert(
        LanguageKey::new("lua"),
        Box::new(MockBackend::new(
            "lua",
            Behavior::Reformat("y = 2\n#end.\ninjected".to_owned()),
        )),
    );
    let formatted = format(&source(input), &formatting_embedded(registry)).expect("a member");
    assert!(
        formatted.embedded_changed,
        "the python body still reformatted"
    );
    assert!(formatted.text.contains("x = 1"), "python spliced");
    assert!(formatted.text.contains("y=2"), "lua reverted to verbatim");
    assert!(
        !formatted.text.contains("injected"),
        "the disturbing output was not spliced"
    );
    assert_eq!(
        formatted.notes.len(),
        1,
        "one note, for the lua disturbance"
    );
    assert!(matches!(
        formatted.certificate,
        Certificate::LayoutOnly | Certificate::UpToSpelling
    ));
}

#[test]
fn a_failed_body_converges_with_the_format_embedded_off_default() {
    // §9.4: a body left verbatim on failure is byte-identical to what
    // format_embedded = false produces.
    let safe_default = format(&source(ONE_SCRIPT), &FormatOptions::default())
        .expect("a member")
        .text;
    let options = formatting_embedded(registry(MockBackend::new(
        "python",
        Behavior::Refuse(EmbeddedError::Failed {
            message: "no".to_owned(),
        }),
    )));
    let degraded = format(&source(ONE_SCRIPT), &options)
        .expect("a member")
        .text;
    assert_eq!(safe_default, degraded);
}

#[test]
fn a_backend_that_returns_the_body_unchanged_is_a_no_op() {
    // A backend already at its fixed point returns the body verbatim: nothing is
    // spliced and no note is recorded, converging with the safe default (§9.4).
    let safe_default = format(&source(ONE_SCRIPT), &FormatOptions::default())
        .expect("a member")
        .text;
    let options = formatting_embedded(registry(MockBackend::new("python", Behavior::Passthrough)));
    let formatted = format(&source(ONE_SCRIPT), &options).expect("a member");
    assert!(
        !formatted.embedded_changed,
        "an unchanged body is not a change"
    );
    assert!(formatted.notes.is_empty());
    assert_eq!(formatted.text, safe_default);
}

#[test]
fn embedded_formatting_is_idempotent() {
    // §9.4: formatting the output again is stable — the backend is at its fixed
    // point on an already-formatted body, so the second pass changes nothing.
    let options = formatting_embedded(registry(MockBackend::new(
        "python",
        Behavior::Reformat("x = 1\n".to_owned()),
    )));
    let once = format(&source(ONE_SCRIPT), &options)
        .expect("a member")
        .text;
    let twice = format(&source(&once), &options).expect("a member").text;
    assert_eq!(once, twice);
}

// ── The concrete backends, behind their features (§9.6, §13) ──────────────────
// The embedded round-trip law for each real backend, driven through the public
// `format` surface: a representative body formats and the ASP skeleton's
// certificate still stands (§9.3 step 1); re-formatting is idempotent (§9.3 step
// 2); a body the backend rejects degrades to verbatim + a warning note (§9.4).
// The whole-text equal-except-`SCRIPT_BODY` bind (§9.3 step 3) is witnessed by
// the reformat landing (`embedded_changed`) with the ASP skeleton intact and its
// certificate held — a backend output that disturbed one ASP token could not.
// Panic containment (§9.5) is proven in the backend modules themselves, where a
// real panic can be raised and caught; a script body cannot conjure one from the
// linked formatter deterministically.

#[cfg(any(feature = "embedded-python", feature = "embedded-lua"))]
fn registry_with(language: &str, backend: Box<dyn EmbeddedFormatter>) -> EmbeddedFormatters {
    let mut registry = EmbeddedFormatters::default();
    registry.insert(LanguageKey::new(language), backend);
    registry
}

#[cfg(feature = "embedded-python")]
mod ruff {
    use super::{
        Certificate, ONE_SCRIPT, Severity, format, formatting_embedded, registry_with, source,
    };
    use morphe::RuffLibBackend;

    fn python(backend_body_source: &str) -> String {
        let options = formatting_embedded(registry_with("python", Box::new(RuffLibBackend::new())));
        format(&source(backend_body_source), &options)
            .expect("a member still formats")
            .text
    }

    #[test]
    fn formats_a_python_body_and_the_asp_certificate_stands() {
        let options = formatting_embedded(registry_with("python", Box::new(RuffLibBackend::new())));
        let formatted = format(&source(ONE_SCRIPT), &options).expect("a member formats");
        assert!(formatted.embedded_changed, "ruff reformatted the body");
        assert!(
            formatted.text.contains("x = 1"),
            "the reformatted body is spliced"
        );
        assert!(!formatted.text.contains("x=1"), "the verbatim body is gone");
        assert!(
            formatted.text.contains("a.") && formatted.text.contains("b."),
            "the ASP skeleton is intact — the reformat was confined to the script interior"
        );
        assert!(matches!(
            formatted.certificate,
            Certificate::LayoutOnly | Certificate::UpToSpelling
        ));
        assert!(
            formatted.notes.is_empty(),
            "a clean reformat records no note"
        );
    }

    #[test]
    fn embedded_formatting_is_idempotent() {
        let once = python(ONE_SCRIPT);
        let twice = python(&once);
        assert_eq!(
            once, twice,
            "ruff is at its fixed point on the formatted body"
        );
    }

    #[test]
    fn an_invalid_python_body_stays_verbatim_with_a_note() {
        // §9.4: ruff rejects the body (its own syntax error) → Failed → verbatim + note.
        let options = formatting_embedded(registry_with("python", Box::new(RuffLibBackend::new())));
        let formatted = format(
            &source("a.\n#script (python)\ndef (\n#end.\nb.\n"),
            &options,
        )
        .expect("a member still formats");
        assert!(!formatted.embedded_changed, "nothing was spliced");
        assert!(formatted.text.contains("def ("), "the body stayed verbatim");
        assert_eq!(formatted.notes.len(), 1, "one note for the rejected body");
        assert_eq!(
            formatted.notes[0].severity(),
            Severity::Warning,
            "warning-level, never a refusal"
        );
    }
}

#[cfg(feature = "embedded-lua")]
mod stylua {
    use super::{Certificate, Severity, format, formatting_embedded, registry_with, source};
    use morphe::StyLuaLibBackend;

    const ONE_LUA_SCRIPT: &str = "a.\n#script (lua)\nlocal x=1\n#end.\nb.\n";

    fn lua(backend_body_source: &str) -> String {
        let options = formatting_embedded(registry_with("lua", Box::new(StyLuaLibBackend::new())));
        format(&source(backend_body_source), &options)
            .expect("a member still formats")
            .text
    }

    #[test]
    fn formats_a_lua_body_and_the_asp_certificate_stands() {
        let options = formatting_embedded(registry_with("lua", Box::new(StyLuaLibBackend::new())));
        let formatted = format(&source(ONE_LUA_SCRIPT), &options).expect("a member formats");
        assert!(formatted.embedded_changed, "StyLua reformatted the body");
        assert!(
            formatted.text.contains("local x = 1"),
            "the reformatted body is spliced"
        );
        assert!(
            !formatted.text.contains("local x=1"),
            "the verbatim body is gone"
        );
        assert!(
            formatted.text.contains("a.") && formatted.text.contains("b."),
            "the ASP skeleton is intact — the reformat was confined to the script interior"
        );
        assert!(matches!(
            formatted.certificate,
            Certificate::LayoutOnly | Certificate::UpToSpelling
        ));
        assert!(
            formatted.notes.is_empty(),
            "a clean reformat records no note"
        );
    }

    #[test]
    fn embedded_formatting_is_idempotent() {
        let once = lua(ONE_LUA_SCRIPT);
        let twice = lua(&once);
        assert_eq!(
            once, twice,
            "StyLua is at its fixed point on the formatted body"
        );
    }

    #[test]
    fn an_invalid_lua_body_stays_verbatim_with_a_note() {
        // §9.4: StyLua rejects the body (its own parse error) → Failed → verbatim + note.
        let options = formatting_embedded(registry_with("lua", Box::new(StyLuaLibBackend::new())));
        let formatted = format(
            &source("a.\n#script (lua)\nlocal x = = 1\n#end.\nb.\n"),
            &options,
        )
        .expect("a member still formats");
        assert!(!formatted.embedded_changed, "nothing was spliced");
        assert!(
            formatted.text.contains("local x = = 1"),
            "the body stayed verbatim"
        );
        assert_eq!(formatted.notes.len(), 1, "one note for the rejected body");
        assert_eq!(
            formatted.notes[0].severity(),
            Severity::Warning,
            "warning-level, never a refusal"
        );
    }
}
