//! The Lua embedded backend (docs/design/morphe.md §9.6): a
//! [`StyLuaLibBackend`] that formats a `#script(lua)` body with `StyLua`, behind
//! the `embedded-lua` feature. This is the only module that names
//! `stylua_lib`/`full_moon`, so a pin move touches one file and never the core.
//!
//! The call is wrapped at this boundary (§9.5), converting a panic inside `StyLua`
//! into [`EmbeddedError::Panicked`] — 100% safe Rust (§12). `StyLua`'s
//! [`OutputVerification::Full`] reparse is
//! the delegated meaning-preservation guarantee (§9.3 step 2): a body it cannot
//! parse is [`EmbeddedError::Failed`]; an output whose reparse fails or whose
//! AST differs from the input's is [`EmbeddedError::Unverified`].

use std::panic::{AssertUnwindSafe, catch_unwind};

use stylua_lib::{Config, Error, OutputVerification, format_code};

use crate::embedded::{
    Availability, EmbeddedContext, EmbeddedError, EmbeddedFormatter, LanguageKey,
};

/// The `#script` language this backend claims (§9.6).
const LANGUAGE: &str = "lua";

/// The Lua embedded formatter, delegating to `StyLua` (§9.6). Construct it with
/// [`StyLuaLibBackend::new`] and register it under the language it
/// [claims](EmbeddedFormatter::language).
pub struct StyLuaLibBackend {
    language: LanguageKey,
}

impl StyLuaLibBackend {
    /// The Lua backend, claiming the `lua` script language (§9.6).
    #[must_use]
    pub fn new() -> StyLuaLibBackend {
        StyLuaLibBackend {
            language: LanguageKey::new(LANGUAGE),
        }
    }
}

impl Default for StyLuaLibBackend {
    fn default() -> StyLuaLibBackend {
        StyLuaLibBackend::new()
    }
}

impl EmbeddedFormatter for StyLuaLibBackend {
    fn language(&self) -> &LanguageKey {
        &self.language
    }

    fn availability(&self) -> Availability {
        // Compiled in and pure in-process — StyLua is always ready (§9.2).
        Availability::Available
    }

    fn format(&self, body: &str, cx: &EmbeddedContext) -> Result<String, EmbeddedError> {
        contain(|| format_lua(body, *cx))
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

/// Format `body` with `StyLua` under the `Full` output verification (§9.6, §9.3
/// step 2). A parse error of the input maps to [`EmbeddedError::Failed`]; a
/// verification failure — the reparse errored, or the output AST differs from
/// the input's — maps to [`EmbeddedError::Unverified`].
fn format_lua(body: &str, cx: EmbeddedContext) -> Result<String, EmbeddedError> {
    format_code(body, config(cx), None, OutputVerification::Full).map_err(|error| map_error(&error))
}

/// `StyLua`'s configuration with morphe's line width applied (§9.6). morphe
/// translates only the line width (§9.1); every other `StyLua` option keeps its
/// default, and the default `syntax` spans Lua 5.1–5.4 (the `lua54` feature).
fn config(cx: EmbeddedContext) -> Config {
    Config {
        column_width: cx.line_width,
        ..Config::default()
    }
}

/// Map a `StyLua` error to the seam's taxonomy (§9.1, §9.6): an input parse error
/// is [`EmbeddedError::Failed`]; either verification failure is
/// [`EmbeddedError::Unverified`].
fn map_error(error: &Error) -> EmbeddedError {
    match error {
        Error::ParseError(_) => EmbeddedError::Failed {
            message: error.to_string(),
        },
        Error::VerificationAstError(_) | Error::VerificationAstDifference => {
            EmbeddedError::Unverified
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_panic_inside_the_backend_is_contained_as_panicked() {
        // §9.5: the boundary converts a panic into a typed Panicked, never an
        // unwind into morphe's core. Proven with a real panic, since a script
        // body cannot deterministically drive StyLua into one.
        assert_eq!(
            contain(|| panic!("the backend panicked")),
            Err(EmbeddedError::Panicked)
        );
    }
}
