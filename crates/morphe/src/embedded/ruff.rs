//! The Python embedded backend (docs/design/morphe.md §9.6): a
//! [`RuffLibBackend`] that formats a `#script(python)` body with ruff, behind
//! the `embedded-python` feature. This is the only module that names any
//! `ruff_*` type, so a pin move (§9.6) touches one file and never the core.
//!
//! The call is wrapped at this boundary (§9.5): a linked backend shares morphe's
//! process, so a panic inside ruff is converted to [`EmbeddedError::Panicked`]
//! rather than unwinding into the core — 100% safe Rust (§12). ruff has no
//! built-in output verification, so morphe supplies its own reparse/idempotence
//! check on the body (§9.3 step 2): ruff's output must re-format — re-parsing it
//! in the process — to itself, or the result is [`EmbeddedError::Unverified`].

use std::panic::{AssertUnwindSafe, catch_unwind};

use ruff_formatter::LineWidth;
use ruff_python_formatter::{PyFormatOptions, format_module_source};

use crate::embedded::{
    Availability, EmbeddedContext, EmbeddedError, EmbeddedFormatter, LanguageKey,
};

/// The `#script` language this backend claims (§9.6).
const LANGUAGE: &str = "python";

/// The Python embedded formatter, delegating to ruff (§9.6). Construct it with
/// [`RuffLibBackend::new`] and register it under the language it
/// [claims](EmbeddedFormatter::language).
pub struct RuffLibBackend {
    language: LanguageKey,
}

impl RuffLibBackend {
    /// The Python backend, claiming the `python` script language (§9.6).
    #[must_use]
    pub fn new() -> RuffLibBackend {
        RuffLibBackend {
            language: LanguageKey::new(LANGUAGE),
        }
    }
}

impl Default for RuffLibBackend {
    fn default() -> RuffLibBackend {
        RuffLibBackend::new()
    }
}

impl EmbeddedFormatter for RuffLibBackend {
    fn language(&self) -> &LanguageKey {
        &self.language
    }

    fn availability(&self) -> Availability {
        // Compiled in and pure in-process — ruff is always ready (§9.2).
        Availability::Available
    }

    fn format(&self, body: &str, cx: &EmbeddedContext) -> Result<String, EmbeddedError> {
        contain(|| format_python(body, *cx))
    }
}

/// Run `call`, containing a panic inside the linked backend as
/// [`EmbeddedError::Panicked`] (§9.5) instead of letting it unwind into morphe's
/// core. Asserting unwind safety is sound: on a panic every value the closure
/// touched is discarded and a typed error is returned, so no broken invariant
/// crosses the boundary. 100% safe Rust (§12).
fn contain(call: impl FnOnce() -> Result<String, EmbeddedError>) -> Result<String, EmbeddedError> {
    match catch_unwind(AssertUnwindSafe(call)) {
        Ok(result) => result,
        Err(_) => Err(EmbeddedError::Panicked),
    }
}

/// Format `body` with ruff and hold it to morphe's own reparse/idempotence check
/// (§9.3 step 2, §9.6). ruff's own parse/format/print errors map to
/// [`EmbeddedError::Failed`]; an output that does not re-format to itself —
/// re-parsed in the process — is [`EmbeddedError::Unverified`].
fn format_python(body: &str, cx: EmbeddedContext) -> Result<String, EmbeddedError> {
    let formatted = format_module_source(body, options(cx))
        .map_err(|error| EmbeddedError::Failed {
            message: error.to_string(),
        })?
        .into_code();
    match format_module_source(&formatted, options(cx)) {
        Ok(again) if again.as_code() == formatted => Ok(formatted),
        _ => Err(EmbeddedError::Unverified),
    }
}

/// ruff's options with morphe's line width applied (§9.6). morphe translates
/// only the line width (§9.1); every other ruff option keeps ruff's default. A
/// width ruff cannot represent (zero, or beyond `u16`) keeps ruff's default
/// rather than failing — the seam degrades, never panics (§9.4).
fn options(cx: EmbeddedContext) -> PyFormatOptions {
    let options = PyFormatOptions::default();
    match u16::try_from(cx.line_width)
        .ok()
        .and_then(|width| LineWidth::try_from(width).ok())
    {
        Some(line_width) => options.with_line_width(line_width),
        None => options,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_inside_the_backend_is_contained_as_panicked() {
        // §9.5: the boundary converts a panic into a typed Panicked, never an
        // unwind into morphe's core. Proven with a real panic, since a script
        // body cannot deterministically drive ruff into one.
        assert_eq!(
            contain(|| panic!("the backend panicked")),
            Err(EmbeddedError::Panicked)
        );
    }
}
