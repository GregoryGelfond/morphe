//! The options vocabulary (docs/design/morphe.md §4.2, §4.3): the one
//! configuration `format`, `format_parsed`, and `reorder` read, and the
//! results and typed refusals they return. The embedded seam these options
//! carry lives in [`crate::embedded`] (§9.1).

use themelios_syntax::base::diagnostic::Diagnostic;
use themelios_syntax::diagnostic::SyntaxError;
use themelios_syntax::dialect::Dialect;
use themelios_syntax::equiv::{Certificate, Mismatch};

use crate::embedded::EmbeddedFormatters;
#[cfg(test)]
use crate::embedded::{EmbeddedContext, EmbeddedError};

/// The formatted result. `certificate` is the claim morphe proved of the
/// output's **ASP program (skeleton)** against the input (§5.2, §5.3),
/// bodies held verbatim for that proof. When `embedded_changed` is true a
/// `#script` interior was reformatted (§9.3): that interior is *outside* the
/// certificate, under its backend's delegated guarantee plus the
/// equal-except-script-bodies check, so the whole-text relation to the input
/// is "certificate ∧ equal-except-`SCRIPT_BODY`", not the certificate alone.
/// `changed` is whether the text differs from the input; `notes` carries the
/// per-script embedded outcomes and other warnings (§9.4, §11).
#[derive(Clone, Debug)]
pub struct Formatted {
    /// The formatted output text.
    pub text: String,
    /// The certificate proved of the ASP skeleton (§5.2).
    pub certificate: Certificate,
    /// Whether the embedded pass reformatted at least one `#script` body
    /// (§9.3) — the signal that the whole-text guarantee is the composed one,
    /// not the certificate alone.
    pub embedded_changed: bool,
    /// Whether `text` differs from the input source.
    pub changed: bool,
    /// Per-script embedded outcomes and other warnings that do not refuse the
    /// format (§9.4, §11).
    pub notes: Vec<Diagnostic>,
}

/// Why morphe declined to produce certified output.
#[derive(Clone, Debug)]
pub enum FormatError {
    /// The input is not a member — it has parse errors. morphe does not
    /// format a non-member (§5.7); the tier's own diagnostics are returned
    /// for the caller to render, unchanged.
    HasParseErrors(Vec<SyntaxError>),
    /// morphe produced output it could not certify against the input. This is a
    /// morphe **bug**, never the input's fault; `reason` carries the witness
    /// matched to the class of defect. It must never occur; that it is a typed
    /// value rather than a panic is the totality discipline (§12), and the CLI
    /// maps it to `Outcome::InternalError` with a bug-report ask (§10.3).
    CertificateBroke {
        /// The certificate morphe was proving.
        certificate: Certificate,
        /// Which class of break, with its witness (§5.3).
        reason: CertificateBreak,
    },
}

/// Why the certificate gate refused morphe's own candidate output — always a
/// morphe **bug** (§5.3), split by class so the loud `Outcome::InternalError`
/// report (§10.3) carries the witness that actually locates the defect.
#[derive(Clone, Debug)]
pub enum CertificateBreak {
    /// The candidate re-parsed as a member but its non-whitespace token sequence
    /// diverged from the input's — morphe permuted, dropped, or altered a token
    /// or comment. The tier's `Mismatch` locates the first divergence.
    TokenMismatch(Mismatch),
    /// The candidate did not re-parse as a member — morphe emitted text that is
    /// not valid ASP (e.g. two tokens run together). The tier's own diagnostics
    /// locate the malformed spot, where a token witness would only mislead.
    NotAMember(Vec<SyntaxError>),
}

/// The reorder result (§15). `text` is formatted, and reordered when any
/// reorderable unit moved (the per-unit `certificate` and the **permutation
/// certificate** (§15.3) both proved); `reordered` is false when the program
/// is all barriers or already in order — `text` is then merely formatted. The
/// reorder never skips a member: order-sensitivity is handled by barriers
/// (§15.4), not a whole-program precondition.
#[derive(Clone, Debug)]
pub struct Reordered {
    /// The formatted, and when `reordered` permuted, output text.
    pub text: String,
    /// The **uniform claim class** every unit earned (§5.3), not a per-unit
    /// value: `preserve_spellings` fixes one target for all, so this is
    /// `UpToSpelling` if any unit canonicalized, else `LayoutOnly` (weakest
    /// wins). The permutation certificate (§15.3) carries no value of its own
    /// — its success is the `Ok` return.
    pub certificate: Certificate,
    /// Whether the permutation was applied and proved (§15.3) — false when the
    /// program is barrier-only or already in order (nothing moved); then
    /// `text` is merely formatted. The reorder never skips a member (§15.4).
    pub reordered: bool,
    /// Whether `text` differs from the input source.
    pub changed: bool,
    /// Per-unit embedded outcomes and other warnings that do not refuse the
    /// reorder (§9.4, §11).
    pub notes: Vec<Diagnostic>,
}

/// Why morphe declined to produce a reorder result.
#[derive(Clone, Debug)]
pub enum ReorderError {
    /// A non-member — parse errors; refused as `format` does (§5.7).
    HasParseErrors(Vec<SyntaxError>),
    /// morphe produced a reorder it could not prove a pure permutation of the
    /// input — a morphe **bug** (the reorder analogue of `CertificateBroke`),
    /// mapped to `Outcome::InternalError` (§10.3, §15.3). The witness
    /// distinguishes the two failure shapes §15.3 checks: a **per-unit**
    /// layout break (a token `Mismatch`, from that unit's own `format`), or a
    /// **bijection** breach.
    PermutationBroke {
        /// The per-unit layout-break witness, from that unit's own `format`
        /// (§15.3).
        unit: Option<Mismatch>,
        /// The bijection-breach witness (§15.3).
        bijection: Option<BijectionBreach>,
    },
}

/// A breach of the permutation's structural law (§15.3) — always a morphe bug,
/// never the input's fault. The check is a both-ways multiset count over the
/// units of each barrier-divided region, so it sees a unit's whole subsequence
/// present or absent: a unit that was *split* across two output units surfaces as
/// one dropped and two added, a *duplicated* one as an extra added. Those finer
/// distinctions are not drawn because the count cannot draw them; the variants
/// are exactly what it produces (§15.3).
#[derive(Clone, Debug)]
pub enum BijectionBreach {
    /// An output unit with no matching input unit.
    UnitAdded {
        /// The offending top-level index.
        at: usize,
    },
    /// An input unit with no matching output unit.
    UnitDropped {
        /// The offending top-level index.
        at: usize,
    },
    /// A barrier that did not hold its position or its neighbours (§15.3;
    /// syntax.md §9.2's four-facts hazard).
    BarrierMoved {
        /// The offending top-level index.
        at: usize,
    },
    /// The reassembled output did not re-parse as a member — morphe emitted text
    /// that is not valid ASP. The tier's own diagnostics locate the malformed
    /// spot, where a unit index would only mislead (the `NotAMember` of the
    /// layout certificate's break, §5.3, one operation up).
    NotAMember(Vec<SyntaxError>),
}

/// The default line width — kallos's and rustfmt's own default (§7.1). The
/// sole configurable layout value.
pub const DEFAULT_LINE_WIDTH: usize = 100;

/// The whole of morphe's configuration: one style knob (the line width), the
/// dialect and spelling policy, the embedded backends, and the reorder's
/// safe/unsafe toggles (§10.2). Style stays opinionated — `line_width` is the
/// *only* layout knob; indent and the rest are fixed (§16). The other fields
/// select the dialect, and which sound transforms run.
#[derive(Clone)]
pub struct FormatOptions {
    /// The one style knob — `DEFAULT_LINE_WIDTH` by default (§7.1). Indent is
    /// fixed (`INDENT`, §6); configurable indent is a §16 seam.
    pub line_width: usize,
    /// Which dialect to parse and lay out under (syntax.md §3). Used by
    /// `format`; `format_parsed` reads the dialect from the `Parse` and
    /// ignores this field.
    pub dialect: Dialect,
    /// Whether to preserve the author's synonym spellings. Off (the default)
    /// normalizes to canonical spellings (§7.4) and earns `UpToSpelling`; on
    /// preserves them and earns the stricter `LayoutOnly` (§5.3).
    pub preserve_spellings: bool,
    /// The embedded-script formatters, injected. Empty (the default) or
    /// backend-less means every `#script` body stays verbatim (§9). The
    /// library hardcodes no backend; the CLI supplies the defaults.
    pub embedded: EmbeddedFormatters,
    /// Whether embedded formatting is applied at all (the run-time opt-in).
    /// Off by default — nobody's script is reformatted unasked (§9.2).
    pub format_embedded: bool,
    /// The reorder's one **unsafe** toggle (§10.2, §15.4): treat a top-level
    /// `#include` as a reorderable, hoistable `Include` section rather than a
    /// barrier — the *caller's assertion* that their includes carry no
    /// order-sensitive construct. Off by default (includes are barriers,
    /// sound). Read only by `reorder`.
    pub reorder_includes: bool,
}

impl Default for FormatOptions {
    /// Every literal resolves through a named constant or an explicit enum
    /// value (§10.4): `DEFAULT_LINE_WIDTH` for the width, `Dialect::Clingo`
    /// for the dialect, an empty registry for `embedded`, and the safe/off
    /// flag each field's own documentation states.
    fn default() -> FormatOptions {
        FormatOptions {
            line_width: DEFAULT_LINE_WIDTH,
            dialect: Dialect::Clingo,
            preserve_spellings: false,
            embedded: EmbeddedFormatters::default(),
            format_embedded: false,
            reorder_includes: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_defaults_are_named_constants_no_bare_numbers() {
        let options = FormatOptions::default();
        assert_eq!(options.line_width, DEFAULT_LINE_WIDTH);
        assert_eq!(DEFAULT_LINE_WIDTH, 100); // §7.1 kallos's and rustfmt's default
        assert!(!options.preserve_spellings); // §5.3 default canonicalizes → UpToSpelling
        assert!(!options.format_embedded); // §9.2 nobody's script reformatted unasked
        assert!(!options.reorder_includes); // §15.4 includes are barriers by default
    }

    #[test]
    fn an_empty_registry_supports_no_language() {
        let cx = EmbeddedContext {
            line_width: DEFAULT_LINE_WIDTH,
        };
        let refusal = EmbeddedFormatters::default().format("python", "x=1", &cx);
        assert!(matches!(refusal, Err(EmbeddedError::Unsupported)));
    }
}
