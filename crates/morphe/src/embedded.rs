//! The embedded-script seam (docs/design/morphe.md §9.1): the
//! [`EmbeddedFormatter`] trait, the registry that resolves a `#script`'s
//! declared language to a backend, and the typed outcomes a backend can
//! produce. The trait and registry are always present (§9.2); no concrete
//! backend lives here — Cargo features gate those, and the run-time opt-in
//! ([`crate::options::FormatOptions`]'s `format_embedded`) gates whether the
//! registry is consulted at all.

use std::borrow::Borrow;
use std::collections::HashMap;
use std::ops::Range;
use std::sync::Arc;

use themelios_syntax::ast::{AstToken, Program, ScriptStatement, Statement};
use themelios_syntax::base::diagnostic::{Diagnostic, DiagnosticId, Label, Severity};
use themelios_syntax::parse::Parse;
use themelios_syntax::tree::{AstNode, TextRange};

use crate::certify;
use crate::options::FormatOptions;

// The concrete backends (§9.6), each behind its Cargo feature and each the only
// module naming its library's crates (§9.2). The trait, registry, and pass above
// are always present; these are compiled only when their feature is on.
#[cfg(feature = "embedded-python")]
pub mod ruff;
#[cfg(feature = "embedded-lua")]
pub mod stylua;

/// One embedded language's formatter (§9.1): total, bounded, producing
/// formatted text or a typed reason it could not.
pub trait EmbeddedFormatter {
    /// The language this backend claims, matched against a `#script`
    /// statement's declared name (§9.1).
    fn language(&self) -> &LanguageKey;

    /// Whether the backend can format right now — [`Availability::Available`],
    /// or [`Availability::Unavailable`] with the reason (§9.1, §9.2).
    fn availability(&self) -> Availability;

    /// Format `body` under `cx`.
    ///
    /// # Errors
    ///
    /// A typed [`EmbeddedError`] for every way formatting can fail — never a
    /// panic across this boundary (§9.1, §9.5).
    fn format(&self, body: &str, cx: &EmbeddedContext) -> Result<String, EmbeddedError>;
}

/// The backends a caller assembled from configuration, keyed by language
/// (§9.1). Empty by default, so with nothing registered every `#script` body
/// stays verbatim (§9.2); the library hardcodes no backend, the CLI supplies
/// the defaults.
#[derive(Clone, Default)]
pub struct EmbeddedFormatters {
    // Keyed by language. Held behind `Arc`, not the `Box` a caller hands to
    // `insert`: `FormatOptions` embeds this registry and derives `Clone`
    // (§4.2), which a map of `Box<dyn EmbeddedFormatter>` could not, and an
    // `Arc` clone is a pointer copy whatever the trait object holds.
    backends: HashMap<LanguageKey, Arc<dyn EmbeddedFormatter>>,
}

impl EmbeddedFormatters {
    /// Register `formatter` under `language`, replacing any backend already
    /// registered for it — the construction API a caller uses to assemble the
    /// registry from configuration (§9.1). Amortized O(1).
    pub fn insert(&mut self, language: LanguageKey, formatter: Box<dyn EmbeddedFormatter>) {
        self.backends.insert(language, Arc::from(formatter));
    }

    /// Format `body`, declared as `language`, under `cx`.
    ///
    /// # Errors
    ///
    /// [`EmbeddedError::Unsupported`] when no backend claims `language`;
    /// otherwise whatever the claiming backend returns (§9.1). Total; an O(1)
    /// lookup, then the backend's own cost.
    pub fn format(
        &self,
        language: &str,
        body: &str,
        cx: &EmbeddedContext,
    ) -> Result<String, EmbeddedError> {
        match self.backends.get(language) {
            Some(backend) => backend.format(body, cx),
            None => Err(EmbeddedError::Unsupported),
        }
    }
}

impl FromIterator<(LanguageKey, Box<dyn EmbeddedFormatter>)> for EmbeddedFormatters {
    /// Assemble a registry from `(language, backend)` pairs; a later pair for
    /// a language already seen replaces the earlier, as `insert` does.
    fn from_iter<I>(pairs: I) -> EmbeddedFormatters
    where
        I: IntoIterator<Item = (LanguageKey, Box<dyn EmbeddedFormatter>)>,
    {
        let mut formatters = EmbeddedFormatters::default();
        for (language, formatter) in pairs {
            formatters.insert(language, formatter);
        }
        formatters
    }
}

/// What a backend needs beyond the source text (§9.1): morphe's `line_width`,
/// passed through so the whole file shares one width. It carries no base
/// indentation — a `#script` is a top-level statement, so its body sits at
/// column 0 — and unifies no backend's own option surface; the seam unifies
/// the verb (format this string), not the knobs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EmbeddedContext {
    /// morphe's configured line width, shared across the whole file (§9.1).
    pub line_width: usize,
}

/// A typed reason a backend could not format a body — the "typed errors for
/// the unknowns" posture (§12). Every variant collapses to one handling: the
/// body stays verbatim and a note is recorded (§9.4).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum EmbeddedError {
    /// No backend claims this language (§9.1).
    Unsupported,
    /// A backend claims the language but is disabled or absent right now.
    Unavailable {
        /// Why the backend is unavailable.
        reason: String,
    },
    /// The backend rejected the body — its own syntax error, not morphe's.
    Failed {
        /// The backend's own message.
        message: String,
    },
    /// The backend's own round-trip / meaning-preservation check failed (§9.3).
    Unverified,
    /// A panic inside the backend, caught at the boundary (§9.5).
    Panicked,
}

/// Whether a backend can format right now (§9.1, §9.2): compiled in via its
/// Cargo feature and, at run time, able to do its job.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Availability {
    /// The backend is ready to format.
    Available,
    /// The backend cannot format right now.
    Unavailable {
        /// Why the backend is unavailable.
        reason: String,
    },
}

/// A `#script` language name (e.g. `python`, `lua`) — the registry's key and
/// a backend's own claimed identity (§9.1).
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct LanguageKey(String);

impl LanguageKey {
    /// The key for `name`, exactly as a `#script` statement declares it.
    #[must_use]
    pub fn new(name: impl Into<String>) -> LanguageKey {
        LanguageKey(name.into())
    }
}

impl Borrow<str> for LanguageKey {
    fn borrow(&self) -> &str {
        &self.0
    }
}

/// morphe's diagnostic namespace (§11).
const NAMESPACE: &str = "morphe";

/// A `#script` body the backend declined or could not format; it was kept
/// verbatim (§9.4). The specific reason is the note's message.
const NOT_FORMATTED: DiagnosticId = DiagnosticId::new(NAMESPACE, "embedded-not-formatted");

/// A reformat whose output would disturb the ASP program structure; the body
/// was kept verbatim to hold the certified program intact (§9.3 step 3).
const DISTURBS_STRUCTURE: DiagnosticId = DiagnosticId::new(NAMESPACE, "embedded-structure");

/// The outcome of the embedded pass (§9.3, §9.4): the text with every clean
/// reformat spliced in, whether any body was replaced, and the per-script
/// degradation notes for the bodies kept verbatim.
pub(crate) struct EmbeddedPass {
    /// The text after splicing — the skeleton text where nothing applied.
    pub(crate) text: String,
    /// Whether any `#script` body was replaced (§9.3).
    pub(crate) changed: bool,
    /// The warning notes for bodies kept verbatim (§9.4).
    pub(crate) notes: Vec<Diagnostic>,
}

/// A clean reformat awaiting the structural bind (§9.3 step 3): where it splices
/// into the skeleton text, the formatted body, and the note to record should the
/// splice prove to disturb the ASP structure.
struct Candidate {
    /// The body's byte span in the skeleton text.
    span: Range<usize>,
    /// The backend's formatted body, to splice over `span`.
    formatted: String,
    /// The warning to record if this splice is kept verbatim instead (§9.4).
    rejection: Diagnostic,
}

/// Format the `#script` bodies of the certified skeleton behind the contained
/// seam (§5.1 step 5): splice each clean reformat in, keep every failure verbatim
/// with a note, and bind the composed text against the certified skeleton (§9.3
/// step 3, §9.4). `input` gives each body's location in the caller's own source
/// for its note; `skeleton_text` — the skeleton certified with every body verbatim
/// (§5.1 step 4) — gives the spans to splice and, re-parsed, the structure the
/// bind holds the composed text to. The common path splices every clean reformat
/// and binds once; a splice that disturbs the ASP structure sends the pass down a
/// per-body attribution path that keeps the offending bodies verbatim. Total:
/// every outcome collapses to splice-or-verbatim, the returned text always
/// satisfies the bind (or is the skeleton itself), and nothing here refuses the
/// format or flips the exit-relevant outcome (§9.4).
pub(crate) fn format_scripts(
    input: &Parse<Program>,
    skeleton_text: &str,
    options: &FormatOptions,
) -> EmbeddedPass {
    let input_scripts = script_statements(input);
    if input_scripts.is_empty() {
        return EmbeddedPass {
            text: skeleton_text.to_owned(),
            changed: false,
            notes: Vec::new(),
        };
    }
    // The certificate (§5.1 step 4) grants the skeleton the input's exact token
    // sequence, so its `#script` statements stand one-for-one with the input's;
    // the re-parse gives their bodies' spans in the skeleton's own coordinates.
    let skeleton = certify::reparse(skeleton_text, input.dialect());
    let skeleton_scripts = script_statements(&skeleton);
    let context = EmbeddedContext {
        line_width: options.line_width,
    };

    let mut candidates: Vec<Candidate> = Vec::new();
    let mut notes: Vec<Diagnostic> = Vec::new();
    for (source_script, skeleton_script) in input_scripts.iter().zip(&skeleton_scripts) {
        let (Some(language), Some(body)) = (skeleton_script.language(), skeleton_script.body())
        else {
            continue; // no language to dispatch on, or an empty region: nothing to format
        };
        let language = language.text();
        let verbatim = body.text();
        match options.embedded.format(language, verbatim, &context) {
            Ok(formatted) if formatted == verbatim => {
                // The backend is at its fixed point on this body: a no-op, no note.
            }
            Ok(formatted) => candidates.push(Candidate {
                span: byte_range(body.syntax().text_range()),
                formatted,
                rejection: structure_note(input, source_script, language),
            }),
            Err(error) => notes.push(refusal_note(input, source_script, language, &error)),
        }
    }
    if candidates.is_empty() {
        return EmbeddedPass {
            text: skeleton_text.to_owned(),
            changed: false,
            notes,
        };
    }

    // §9.3 step 3: splice every candidate, re-parse the whole text, and bind its
    // non-whitespace token sequence to the certified skeleton's. On the common
    // path the bind holds and the composed text is the output.
    let composed = apply(skeleton_text, &candidates);
    if certify::equal_except_script_bodies(&skeleton, &composed).is_ok() {
        return EmbeddedPass {
            text: composed,
            changed: true,
            notes,
        };
    }

    // A splice disturbed the ASP structure. Attribute it — a body is at fault iff
    // splicing it alone disturbs the skeleton — and keep the culprits verbatim
    // with a note (§9.4). A `#script` body is bounded by ASP skeleton tokens, so
    // on the shipped dialects the survivors compose and the re-bind below holds.
    let mut survivors: Vec<Candidate> = Vec::new();
    for candidate in candidates {
        let alone = spliced(skeleton_text, candidate.span.clone(), &candidate.formatted);
        if certify::equal_except_script_bodies(&skeleton, &alone).is_ok() {
            survivors.push(candidate);
        } else {
            notes.push(candidate.rejection);
        }
    }
    let text = apply(skeleton_text, &survivors);
    if certify::equal_except_script_bodies(&skeleton, &text).is_ok() {
        return EmbeddedPass {
            text,
            changed: !survivors.is_empty(),
            notes,
        };
    }

    // The survivors each passed in isolation yet do not compose — reachable only
    // were a future nested-script dialect (§9.1) to let bodies interact. Keep every
    // body verbatim: the safe default the failure path converges with (§9.4).
    for survivor in survivors {
        notes.push(survivor.rejection);
    }
    EmbeddedPass {
        text: skeleton_text.to_owned(),
        changed: false,
        notes,
    }
}

/// The `#script` statements of `parse`, in program order (§9.1).
fn script_statements(parse: &Parse<Program>) -> Vec<ScriptStatement> {
    parse
        .tree()
        .statements()
        .filter_map(|statement| match statement {
            Statement::Script(script) => Some(script),
            _ => None,
        })
        .collect()
}

/// `range` as byte offsets into the source text (the tier's `TextSize` is a byte
/// offset).
fn byte_range(range: TextRange) -> Range<usize> {
    u32::from(range.start()) as usize..u32::from(range.end()) as usize
}

/// `base` with `span` replaced by `replacement`.
fn spliced(base: &str, span: Range<usize>, replacement: &str) -> String {
    let mut text = base.to_owned();
    text.replace_range(span, replacement);
    text
}

/// `base` with every candidate's formatted body spliced over its span. The spans
/// are disjoint `#script` body spans in `base`; applying them from last to first
/// keeps each offset valid.
fn apply(base: &str, candidates: &[Candidate]) -> String {
    let mut splices: Vec<(&Range<usize>, &str)> = candidates
        .iter()
        .map(|candidate| (&candidate.span, candidate.formatted.as_str()))
        .collect();
    splices.sort_by_key(|(span, _)| std::cmp::Reverse(span.start));
    let mut text = base.to_owned();
    for (span, formatted) in splices {
        text.replace_range(span.clone(), formatted);
    }
    text
}

/// The warning note for a body the backend refused (§9.4): the specific reason,
/// located at the script in the caller's own source.
fn refusal_note(
    input: &Parse<Program>,
    script: &ScriptStatement,
    language: &str,
    error: &EmbeddedError,
) -> Diagnostic {
    let reason = match error {
        EmbeddedError::Unsupported => {
            format!("no embedded formatter is registered for the `{language}` script language")
        }
        EmbeddedError::Unavailable { reason } => {
            format!("the `{language}` embedded formatter is unavailable ({reason})")
        }
        EmbeddedError::Failed { message } => {
            format!("the `{language}` embedded formatter rejected the script body ({message})")
        }
        EmbeddedError::Unverified => {
            format!("the `{language}` embedded formatter could not verify its own output")
        }
        EmbeddedError::Panicked => format!("the `{language}` embedded formatter panicked"),
    };
    note(input, script, NOT_FORMATTED, reason)
}

/// The warning note for a reformat that would disturb the ASP structure (§9.3
/// step 3): kept verbatim to hold the certified program intact.
fn structure_note(input: &Parse<Program>, script: &ScriptStatement, language: &str) -> Diagnostic {
    note(
        input,
        script,
        DISTURBS_STRUCTURE,
        format!(
            "the `{language}` embedded formatter's output would alter the ASP program structure"
        ),
    )
}

/// A warning note that `reason` — the script body was kept verbatim (§9.4) —
/// located at `script` in the caller's own source (§11).
fn note(
    input: &Parse<Program>,
    script: &ScriptStatement,
    id: DiagnosticId,
    reason: String,
) -> Diagnostic {
    let mut message = reason;
    message.push_str("; the script body was kept verbatim");
    let location = input.location(script.syntax().text_range());
    Diagnostic::new(
        id,
        Severity::Warning,
        message,
        Label {
            location,
            message: None,
        },
    )
    .expect("the embedded note message is never empty")
}
