//! The process outcome contract (docs/design/morphe.md §10.3, §10.4): the
//! typed status the CLI exits with, and the single conversion from it to a raw
//! process code. Every other part of the CLI speaks in [`Outcome`]; only the
//! [`Termination`] impl below turns one into an integer the operating system
//! reads (§10.4).

use std::process::{ExitCode, Termination};

use morphe::{FormatError, ReorderError};

/// morphe's process outcome (§10.3). The variants are ordered least- to
/// most-severe, so a run over several inputs folds its per-input outcomes to
/// the worst by [`Ord`] (`max`): a clean run reports [`Outcome::Clean`], one
/// pending reformat lifts it to [`Outcome::ChangesNeeded`], the user's error to
/// [`Outcome::UserError`], and a morphe bug to [`Outcome::InternalError`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Outcome {
    /// Formatted, or `--check`/`--diff` found nothing to change.
    Clean,
    /// `--check`/`--diff`: an input is not already in morphe-form.
    ChangesNeeded,
    /// A parse refusal (§5.7), or a usage or I/O error — the user's file or
    /// invocation, not morphe's fault.
    UserError,
    /// The never-should-happen morphe bug — a broken certificate (§5.3) or a
    /// broken permutation certificate (§15.3) — surfaced loudly with a
    /// bug-report ask (§10.3).
    InternalError,
}

/// The process exit codes (§10.4): morphe's only raw exit integers, named here
/// and consumed solely by [`Outcome::exit_code`]. The 0/1/2 trio is the
/// formatter-world convention CI and editors read — success, a pending
/// reformat, the user's error — matching black and ruff; an internal error
/// takes `EX_SOFTWARE` from `sysexits.h` (an internal software fault), distinct
/// so a caller can tell morphe's bug from a problem with their file.
const CLEAN: u8 = 0;
const CHANGES_NEEDED: u8 = 1;
const USER_ERROR: u8 = 2;
const INTERNAL_ERROR: u8 = 70;

impl Outcome {
    /// This outcome's process exit code — the single mapping from an
    /// [`Outcome`] to a raw code (§10.3, §10.4), consumed by the [`Termination`]
    /// conversion below and by the suite (so a test names the code through this
    /// method rather than restating a bare integer). Total; O(1).
    #[must_use]
    pub fn exit_code(self) -> u8 {
        match self {
            Outcome::Clean => CLEAN,
            Outcome::ChangesNeeded => CHANGES_NEEDED,
            Outcome::UserError => USER_ERROR,
            Outcome::InternalError => INTERNAL_ERROR,
        }
    }
}

impl Termination for Outcome {
    /// The one place a raw process code is produced (§10.4).
    fn report(self) -> ExitCode {
        ExitCode::from(self.exit_code())
    }
}

impl From<&FormatError> for Outcome {
    /// A parse-error refusal is the user's file, not morphe's fault, so it is a
    /// user error (§5.7, §10.3); a broken certificate is a morphe bug and earns
    /// the loud internal-error status (§5.3, §10.3).
    fn from(error: &FormatError) -> Outcome {
        match error {
            FormatError::HasParseErrors(_) => Outcome::UserError,
            FormatError::CertificateBroke { .. } => Outcome::InternalError,
        }
    }
}

impl From<&ReorderError> for Outcome {
    /// The reorder's refusals map as `format`'s do (§15.3, §10.3): a non-member
    /// is the user's file (a user error); a broken permutation certificate is a
    /// morphe bug (an internal error).
    fn from(error: &ReorderError) -> Outcome {
        match error {
            ReorderError::HasParseErrors(_) => Outcome::UserError,
            ReorderError::PermutationBroke { .. } => Outcome::InternalError,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use morphe::{Certificate, CertificateBreak};

    #[test]
    fn a_parse_refusal_is_a_user_error() {
        let refusal = FormatError::HasParseErrors(Vec::new());
        assert_eq!(Outcome::from(&refusal), Outcome::UserError);
    }

    #[test]
    fn a_broken_certificate_is_an_internal_error() {
        let bug = FormatError::CertificateBroke {
            certificate: Certificate::LayoutOnly,
            reason: CertificateBreak::NotAMember(Vec::new()),
        };
        assert_eq!(Outcome::from(&bug), Outcome::InternalError);
    }

    #[test]
    fn the_reorder_refusals_map_as_formats_do() {
        let refusal = ReorderError::HasParseErrors(Vec::new());
        assert_eq!(Outcome::from(&refusal), Outcome::UserError);

        let bug = ReorderError::PermutationBroke {
            unit: None,
            bijection: None,
        };
        assert_eq!(Outcome::from(&bug), Outcome::InternalError);
    }

    #[test]
    fn the_four_outcomes_carry_four_distinct_exit_codes() {
        let codes = [
            Outcome::Clean,
            Outcome::ChangesNeeded,
            Outcome::UserError,
            Outcome::InternalError,
        ]
        .map(Outcome::exit_code);
        let distinct: std::collections::BTreeSet<u8> = codes.iter().copied().collect();
        assert_eq!(
            distinct.len(),
            codes.len(),
            "each outcome needs its own code"
        );
    }

    #[test]
    fn severity_orders_clean_below_changes_below_user_below_internal() {
        assert!(Outcome::Clean < Outcome::ChangesNeeded);
        assert!(Outcome::ChangesNeeded < Outcome::UserError);
        assert!(Outcome::UserError < Outcome::InternalError);
    }
}
