//! The `morphe.toml` configuration (docs/design/morphe.md §10.2): the nearest
//! file discovered by walking up from each formatted file, its keys the
//! style/behavior options, unknown keys refused rather than silently dropped.
//! This is the only module that names any `toml` or `serde` type, so a version
//! move or a swap of the parser touches this file alone (§12).

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use morphe::Dialect;
use serde::Deserialize;

/// The file a config is read from, and the name discovery walks up looking for
/// (§10.2). ASP has no `pyproject.toml`-style shared table, so morphe keeps its
/// own file.
const CONFIG_FILE_NAME: &str = "morphe.toml";

/// A `morphe.toml`'s parsed contents (§10.2): the style/behavior options, every
/// one optional so an omitted key leaves the command-line flag or the built-in
/// default in force. Unknown keys are refused, not dropped (`deny_unknown_fields`,
/// §10.2). The default value carries no overrides — what discovery yields when
/// no file is found, and what `--isolated` supplies.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Config {
    /// The maximum line width (§10.1).
    pub(crate) line_width: Option<usize>,
    /// The dialect to parse and lay out under (§10.1).
    pub(crate) dialect: Option<DialectName>,
    /// Whether to preserve the author's synonym spellings (§10.1).
    pub(crate) preserve_spellings: Option<bool>,
    /// Whether to reformat `#script` bodies with their language's tool (§10.1, §9).
    pub(crate) format_embedded: Option<bool>,
    /// Whether to apply the conservative signature reorder (§10.1, §15).
    pub(crate) reorder: Option<bool>,
    /// The unsafe toggles (§10.2, §15.4), under `[unsafe]` so opting into a
    /// transform morphe cannot prove is visible in the file.
    #[serde(rename = "unsafe", default)]
    pub(crate) unsafe_toggles: UnsafeToggles,
}

/// The **unsafe** config section (§10.2, §15.4): transforms whose soundness
/// rests on the caller's assertion, not a morphe proof — housed apart from the
/// safe style/behavior keys so choosing one is legible in the file. Its first
/// tenant is `reorder_includes`.
#[derive(Debug, Default, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UnsafeToggles {
    /// Treat a top-level `#include` as a reorderable section hoisted to the top,
    /// rather than a barrier — the caller's assertion that their includes carry
    /// no order-sensitive construct (§15.4). Read only when reordering.
    pub(crate) reorder_includes: Option<bool>,
}

/// A dialect as named in the config, spelled exactly as `--dialect` takes it
/// (§10.1): `clingo` or `asp-core-2`.
#[derive(Debug, Clone, Copy, Deserialize)]
pub(crate) enum DialectName {
    /// The clingo/clingcon dialect.
    #[serde(rename = "clingo")]
    Clingo,
    /// The ASP-Core-2 variant.
    #[serde(rename = "asp-core-2")]
    AspCore2,
}

impl From<DialectName> for Dialect {
    fn from(name: DialectName) -> Dialect {
        match name {
            DialectName::Clingo => Dialect::Clingo,
            DialectName::AspCore2 => Dialect::AspCore2,
        }
    }
}

/// Why a config could not be used (§10.2, §10.3) — a usage error the driver
/// reports and folds to [`crate::outcome::Outcome::UserError`]. Each names the
/// file, so the message locates the problem the way a parse-error diagnostic
/// locates a source (§11).
pub(crate) enum ConfigError {
    /// The config file could not be read.
    Read {
        /// The file.
        path: PathBuf,
        /// The underlying I/O message.
        message: String,
    },
    /// The config's TOML was malformed or carried an unknown key (§10.2).
    Parse {
        /// The file.
        path: PathBuf,
        /// The parser's own message, locating the offending key or token.
        message: String,
    },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Both conditions read the same to the user — the file, then what went
        // wrong — but stay separate variants: one is an I/O failure, the other a
        // malformed or unknown-key TOML (§10.2), distinct at their call sites.
        match self {
            ConfigError::Read { path, message } | ConfigError::Parse { path, message } => {
                write!(f, "{}: {message}", path.display())
            }
        }
    }
}

/// The config governing a file, discovered as the nearest `morphe.toml` walking
/// up from `start` — the file's own directory — to the filesystem root (§10.2).
/// The default (no overrides) when no file is found.
///
/// # Errors
///
/// [`ConfigError`] when a discovered file cannot be read or parsed (§10.2).
pub(crate) fn discover(start: &Path) -> Result<Config, ConfigError> {
    let mut directory = Some(start);
    while let Some(current) = directory {
        let candidate = current.join(CONFIG_FILE_NAME);
        if candidate.is_file() {
            return parse_file(&candidate);
        }
        directory = current.parent();
    }
    Ok(Config::default())
}

/// The config at an explicit path (`--config`, §10.1) — no discovery, just this
/// file.
///
/// # Errors
///
/// [`ConfigError`] when the file cannot be read or parsed (§10.2).
pub(crate) fn load(path: &Path) -> Result<Config, ConfigError> {
    parse_file(path)
}

/// Read and parse one `morphe.toml`, refusing an unknown key (§10.2).
fn parse_file(path: &Path) -> Result<Config, ConfigError> {
    let text = fs::read_to_string(path).map_err(|error| ConfigError::Read {
        path: path.to_owned(),
        message: error.to_string(),
    })?;
    toml::from_str(&text).map_err(|error| ConfigError::Parse {
        path: path.to_owned(),
        message: error.message().to_owned(),
    })
}
