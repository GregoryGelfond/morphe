//! The input walker (docs/design/morphe.md §10.1): expand a directory argument
//! to its `*.lp` members, gitignore-aware and honouring the exclude globs, and
//! take a file argument as-is whatever its extension. This is the only module
//! that names any `ignore` type, so a version move or a swap of the walker
//! touches this file alone (§12).

use std::fmt;
use std::path::{Path, PathBuf};

use ignore::WalkBuilder;
use ignore::overrides::{Override, OverrideBuilder};

/// The default extension a directory walk formats (§10.1): `*.lp`.
const LP_EXTENSION: &str = "lp";

/// How a directory walk selects files (§10.1).
pub(crate) struct Options {
    /// Glob patterns whose matches are skipped, from `--exclude` and
    /// `--extend-exclude`; empty walks everything the ignore rules admit.
    pub(crate) excludes: Vec<String>,
    /// Whether to honour `.gitignore` / `.ignore` rules; `--no-respect-gitignore`
    /// turns this off and walks the would-be-ignored files (§10.1).
    pub(crate) respect_gitignore: bool,
}

/// Why a walk could not start (§10.1, §10.3): a usage error the driver reports
/// and folds to [`crate::outcome::Outcome::UserError`].
pub(crate) enum WalkError {
    /// An exclude glob the walker could not compile — a malformed
    /// `--exclude` / `--extend-exclude` pattern.
    BadExclude {
        /// The message, naming the pattern.
        message: String,
    },
}

impl fmt::Display for WalkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WalkError::BadExclude { message } => f.write_str(message),
        }
    }
}

/// The files to format for the given path arguments (§10.1). Each directory
/// expands to its `*.lp` members, walked gitignore-aware and with the exclude
/// globs applied; each file argument is taken as-is, whatever its extension,
/// since naming a file is asking for it directly. The order follows the
/// arguments, then each directory's own walk.
///
/// # Errors
///
/// [`WalkError::BadExclude`] when an exclude glob does not compile (§10.1). A
/// per-entry I/O error during the walk is reported and the walk continues, so
/// one unreadable directory never silently drops the rest of the tree.
pub(crate) fn collect(roots: &[&Path], options: &Options) -> Result<Vec<PathBuf>, WalkError> {
    let mut files = Vec::new();
    for root in roots {
        if root.is_dir() {
            let mut builder = WalkBuilder::new(root);
            builder
                // Honour `.gitignore` even outside a git repository — an ASP
                // tree is often not itself a repo, and a formatter must not
                // rewrite a file its author has told git (and so the tools) to
                // leave alone (§10.1). `require_git(false)` reads the ignore
                // rules regardless; `--no-respect-gitignore` drops them all yet
                // still skips hidden files, so `.git` is never descended.
                .require_git(false)
                .standard_filters(options.respect_gitignore)
                .hidden(true)
                .overrides(exclude_overrides(root, &options.excludes)?);
            for result in builder.build() {
                match result {
                    Ok(entry) => {
                        if entry.file_type().is_some_and(|kind| kind.is_file())
                            && has_lp_extension(entry.path())
                        {
                            files.push(entry.path().to_owned());
                        }
                    }
                    Err(error) => eprintln!("morphe: {error}"),
                }
            }
        } else {
            files.push((*root).to_owned());
        }
    }
    Ok(files)
}

/// The exclude globs as an `ignore` override rooted at the walked directory:
/// each pattern is negated (`!glob`), so it removes its matches while
/// everything else stays walked — an all-negated override whitelists nothing,
/// so a non-matching file is not swept up (`ignore`'s override semantics).
fn exclude_overrides(root: &Path, excludes: &[String]) -> Result<Override, WalkError> {
    let mut builder = OverrideBuilder::new(root);
    for glob in excludes {
        builder
            .add(&format!("!{glob}"))
            .map_err(|error| WalkError::BadExclude {
                message: format!("invalid exclude glob `{glob}`: {error}"),
            })?;
    }
    builder.build().map_err(|error| WalkError::BadExclude {
        message: format!("could not build the exclude set: {error}"),
    })
}

/// Whether `path` carries the default `*.lp` extension (§10.1).
fn has_lp_extension(path: &Path) -> bool {
    path.extension()
        .is_some_and(|extension| extension == LP_EXTENSION)
}
