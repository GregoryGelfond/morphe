//! morphe (μορφή, *form*): a formatter for Answer Set Programming.
//!
//! morphe reads the clingo/clingcon dialect of ASP (and its ASP-Core-2
//! variant) through [`themelios_syntax`], lays it out in a fixed house
//! style with one knob — the line width — and refuses to write any output
//! it cannot certify *layout-only* or *up-to-spelling* equivalent to its
//! input (`themelios_syntax::equiv`). Documentation is kept verbatim above its
//! statement, trivia comments are carried to the anchors the tier gives them,
//! and `#script` bodies are kept verbatim unless an embedded formatter is asked
//! for and available.
//!
//! This crate is the library; `morphe-cli` is the thin binary over it.
//! Its core is pure and does no I/O; I/O enters only through the embedded
//! formatter backends a caller injects.
//!
//! Design of record: `docs/design/morphe.md`. morphe is the stage-2
//! first-consumer checkpoint of themelios-syntax (syntax.md §15): what it
//! reveals about that surface folds back into the tier.
#![forbid(unsafe_code)]

pub mod options;
mod doc;
mod lower;
mod emit;
mod comments;
mod certify;
mod reorder;
pub mod embedded;
mod spelling;

pub use options::{
    BijectionBreach, CertificateBreak, DEFAULT_LINE_WIDTH, FormatError, FormatOptions, Formatted,
    ReorderError, Reordered,
};
// The embedded seam is public API — a caller injects the backends (§9.1).
pub use embedded::{
    Availability, EmbeddedContext, EmbeddedError, EmbeddedFormatter, EmbeddedFormatters,
    LanguageKey,
};
// The concrete backends, each behind its feature (§9.6); the CLI assembles them
// into the registry it supplies (§9.1).
#[cfg(feature = "embedded-python")]
pub use embedded::ruff::RuffLibBackend;
#[cfg(feature = "embedded-lua")]
pub use embedded::stylua::StyLuaLibBackend;
// The API vocabulary, reachable through morphe alone (§1, §4.2). base's
// `Diagnostic` sits under its own `diagnostic` module: base is re-exported
// whole (syntax.md §1), so its submodules are preserved, not flattened to
// base's root.
pub use themelios_syntax::{
    ast::Program,
    base::diagnostic::Diagnostic,
    base::source::Source,
    diagnostic::SyntaxError,
    dialect::Dialect,
    equiv::{Certificate, Mismatch},
    parse::Parse,
};

/// The syntax tier morphe is built on, re-exported so a consumer reaches the
/// whole formatting vocabulary — the typed AST, comment attachment, the
/// fusion oracle, and the equivalence certificate — through morphe alone
/// (§1, §4.2).
pub use themelios_syntax;

/// Format a source program. Parses under `options.dialect`, lays it out, and
/// returns the formatted text with the certificate it earned — or a typed
/// refusal. Pure; no I/O beyond any embedded backend in `options` (§4.2, §5.1).
///
/// # Errors
///
/// [`FormatError::HasParseErrors`] when `source` is not a member (§5.7).
/// [`FormatError::CertificateBroke`] is never returned for a member — it would
/// be a morphe bug (§5.3), surfaced loudly (§10.3).
pub fn format(source: &Source, options: &FormatOptions) -> Result<Formatted, FormatError> {
    let parsed = themelios_syntax::parse::parse(source, options.dialect);
    format_pipeline(&parsed, options, source.text())
}

/// Format a program already parsed — for a consumer holding a `Parse<Program>`
/// (the language server, the program tier), so morphe never forces a re-parse.
/// The dialect is read from `parse.dialect()`; this path ignores
/// `options.dialect`. **Precondition:** the `Parse` was built by the file door
/// `parse` (at `NestingLimit::DEFAULT`), the one door morphe uses (§4.2, §5.6).
///
/// # Errors
///
/// As [`format()`]: [`FormatError::HasParseErrors`] for a non-member (§5.7);
/// [`FormatError::CertificateBroke`] only on a morphe bug (§5.3).
pub fn format_parsed(
    parse: &Parse<Program>,
    options: &FormatOptions,
) -> Result<Formatted, FormatError> {
    let input_text = parse.syntax().text().to_string();
    format_pipeline(parse, options, &input_text)
}

/// The fixed pipeline (§5.1), shared by both entries: refuse a non-member
/// (§5.7); lower to a document and render it; run the certificate gate, which
/// cannot return `Ok` text it did not certify (§5.3); then, only when the caller
/// opts in, the embedded pass (step 5, §9.3). The embedded pass never refuses
/// the format or touches the certificate — the ASP skeleton is certified with
/// every body verbatim, and each spliced body is bound by the
/// equal-except-script-bodies check (§9.4); a failure degrades to a verbatim
/// body and a warning note.
fn format_pipeline(
    parse: &Parse<Program>,
    options: &FormatOptions,
    input_text: &str,
) -> Result<Formatted, FormatError> {
    if parse.has_errors() {
        return Err(FormatError::HasParseErrors(parse.diagnostics().to_vec()));
    }
    let document = lower::lower(parse, options);
    let skeleton = doc::render(&document, options.line_width);
    let target = certify::target(options.preserve_spellings);
    let certificate = certify::certify(parse, &skeleton, target).map_err(|reason| {
        FormatError::CertificateBroke {
            certificate: target,
            reason,
        }
    })?;
    let (text, embedded_changed, notes) = if options.format_embedded {
        let pass = embedded::format_scripts(parse, &skeleton, options);
        (pass.text, pass.changed, pass.notes)
    } else {
        (skeleton, false, Vec::new())
    };
    let changed = text.as_str() != input_text;
    Ok(Formatted {
        text,
        certificate,
        embedded_changed,
        changed,
        notes,
    })
}

/// Reorder a program's top-level units into the canonical signature order — a
/// distinct operation from [`format()`], which never changes order (§15). Parses
/// under `options.dialect`, formats each unit, moves whole units (carrying their
/// comments) within the regions the barriers divide (§15.1, §15.4), holds every
/// barrier in place, and proves the permutation certificate before returning
/// (§15.3). A program that is all barriers or already in order moves nothing —
/// `reordered` is false and the text is merely formatted (§15.3). `options`
/// carries the reorder toggles (`reorder_includes`, §10.2). Pure.
///
/// # Errors
///
/// [`ReorderError::HasParseErrors`] when `source` is not a member (§5.7).
/// [`ReorderError::PermutationBroke`] is never returned for a member — it would
/// be a morphe bug (§15.3), surfaced loudly (§10.3).
pub fn reorder(source: &Source, options: &FormatOptions) -> Result<Reordered, ReorderError> {
    let parsed = themelios_syntax::parse::parse(source, options.dialect);
    if parsed.has_errors() {
        return Err(ReorderError::HasParseErrors(parsed.diagnostics().to_vec()));
    }
    let program = parsed.tree();
    let plan = comments::plan(&parsed.syntax());
    let items = reorder::units_and_barriers(&program, &plan, options.reorder_includes);
    let order = if options.reorder_includes {
        reorder::ORDER_WITH_INCLUDES
    } else {
        reorder::ORDER
    };
    let before: Vec<u32> = items.iter().map(reorder::Item::source_offset).collect();
    // The `#include` units reorder_includes made movable (empty otherwise): if one
    // lands at a new position below, an include was hoisted and is surfaced (§15.4).
    let include_ranges = reorder::include_unit_ranges(&items);
    let reordered_items = reorder::bucket_emit(items, order);
    let after: Vec<u32> = reordered_items
        .iter()
        .map(reorder::Item::source_offset)
        .collect();

    if before == after {
        // Nothing moved: a program that is all barriers or already in canonical
        // order is merely formatted, its order and certificate unchanged (§15.3).
        // The parse in hand is reused — it is the file door's, at `DEFAULT` (§5.6).
        let formatted = format_parsed(&parsed, options).map_err(reorder_error)?;
        return Ok(Reordered {
            text: formatted.text,
            certificate: formatted.certificate,
            reordered: false,
            changed: formatted.changed,
            notes: formatted.notes,
        });
    }

    let assembled = reorder::assemble(&reordered_items, source.text(), &plan, options)
        .map_err(reorder_error)?;
    certify::permutation_certificate(&parsed, &assembled.text, options).map_err(|bijection| {
        ReorderError::PermutationBroke {
            unit: None,
            bijection: Some(bijection),
        }
    })?;
    let changed = assembled.text != source.text();
    let mut notes = assembled.notes;
    // An include reorder_includes actually hoisted (its position moved) is the one
    // change that can alter answer sets, on an assertion morphe cannot verify
    // (§15.4) — surfaced here, never silent (the config-discovery posture, §3.4).
    let hoisted: Vec<_> = include_ranges
        .into_iter()
        .filter(|range| {
            let offset = u32::from(range.start());
            before.iter().position(|o| *o == offset) != after.iter().position(|o| *o == offset)
        })
        .collect();
    if let Some(first) = hoisted.first() {
        notes.push(reorder::include_hoist_note(&parsed, *first, hoisted.len()));
    }
    Ok(Reordered {
        text: assembled.text,
        certificate: assembled.certificate,
        reordered: true,
        changed,
        notes,
    })
}

/// Carry a per-unit [`FormatError`] out as the reorder's refusal (§15.3): a
/// per-unit certificate break — a morphe bug — is the reorder's `PermutationBroke`
/// (§10.3), the token-mismatch witness kept; a non-member unit forwards its
/// diagnostics, though a member's one-statement slice is itself a member, so that
/// arm is not reached.
fn reorder_error(error: FormatError) -> ReorderError {
    match error {
        FormatError::HasParseErrors(errors) => ReorderError::HasParseErrors(errors),
        FormatError::CertificateBroke { reason, .. } => {
            let unit = match reason {
                CertificateBreak::TokenMismatch(mismatch) => Some(mismatch),
                CertificateBreak::NotAMember(_) => None,
            };
            ReorderError::PermutationBroke {
                unit,
                bijection: None,
            }
        }
    }
}
