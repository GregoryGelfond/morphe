//! The morphe command-line formatter — the thin driver over the `morphe`
//! library. See docs/design/morphe.md §10. All of morphe's I/O lives here; the
//! library core is pure (§4.1). The binary (`src/main.rs`) is a shim over
//! [`run`]; the driver lives in the library so the suite can name the exit
//! contract ([`outcome::Outcome`]) rather than a raw process code (§10.4).
#![forbid(unsafe_code)]

pub mod outcome;

mod atomic;
mod color;
mod config;
mod diff;
mod report;
mod walk;

use std::fs;
use std::io::{self, IsTerminal, Read};
use std::path::{Path, PathBuf};

use clap::{Parser, ValueEnum};
#[cfg(feature = "embedded-python")]
use morphe::RuffLibBackend;
#[cfg(feature = "embedded-lua")]
use morphe::StyLuaLibBackend;
use morphe::themelios_syntax::base::source::{Source, SourceId};
use morphe::{
    DEFAULT_LINE_WIDTH, Dialect, EmbeddedFormatter, EmbeddedFormatters, FormatError, FormatOptions,
    LanguageKey, ReorderError, format,
};

use outcome::Outcome;

/// The source identity minted for each input (§10.4). A run formats one source
/// per `format` call and never surfaces the identity, so one named value stands
/// for every input, as `certify`'s reparse identity does.
const INPUT_SOURCE_ID: SourceId = SourceId::new(0);

/// The label a stdin input carries in messages and diff headers.
const STDIN_LABEL: &str = "<stdin>";

/// The `--dialect` choices (§10.1) — the CLI's own enum, so `morphe`'s
/// [`Dialect`] stays free of the argument parser. Each value name is pinned to
/// the spelling the user types.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum DialectArg {
    #[value(name = "clingo")]
    Clingo,
    #[value(name = "asp-core-2")]
    AspCore2,
}

impl From<DialectArg> for Dialect {
    fn from(dialect: DialectArg) -> Dialect {
        match dialect {
            DialectArg::Clingo => Dialect::Clingo,
            DialectArg::AspCore2 => Dialect::AspCore2,
        }
    }
}

/// The option crop (§10.1). A value option left unset falls back to the
/// discovered `morphe.toml` (§10.2), then to the built-in default; a boolean
/// flag ORs with the config. The options are grouped by the families §10.1
/// names, both so the help reads that way and so no single group carries enough
/// boolean flags to become a state better modelled another way. The embedded
/// and reorder wiring is added below.
#[derive(Parser, Debug)]
#[command(
    name = "morphe",
    version,
    about = "A formatter for Answer Set Programming."
)]
struct Cli {
    #[command(flatten)]
    mode: ModeArgs,

    #[command(flatten)]
    style: StyleArgs,

    #[command(flatten)]
    embedded: EmbeddedArgs,

    #[command(flatten)]
    reorder: ReorderArgs,

    #[command(flatten)]
    input: InputArgs,

    #[command(flatten)]
    config: ConfigArgs,

    #[command(flatten)]
    report: ReportArgs,

    /// Inputs to format in place, or `-` to format stdin to stdout.
    #[arg(value_name = "PATH")]
    paths: Vec<PathBuf>,
}

/// The modes (§10.1): format in place by default, or check/diff without writing.
#[derive(clap::Args, Debug)]
struct ModeArgs {
    /// Check whether inputs are already formatted; write nothing.
    #[arg(long)]
    check: bool,

    /// Print a diff of what would change instead of writing.
    #[arg(long)]
    diff: bool,
}

/// Style and layout (§10.1): the one width knob, the dialect, and spellings.
#[derive(clap::Args, Debug)]
struct StyleArgs {
    /// The maximum line width [default: 100].
    #[arg(long, value_name = "N")]
    line_width: Option<usize>,

    /// The dialect to parse and lay out under [default: clingo].
    #[arg(long, value_enum)]
    dialect: Option<DialectArg>,

    /// Preserve synonym spellings instead of normalizing them.
    #[arg(long)]
    preserve_spellings: bool,
}

/// Embedded-script formatting (§10.1, §9): reformat `#script` bodies with the
/// language's own tool (ruff for Python, `StyLua` for Lua), or hold them verbatim.
/// Off by default; the last of the pair on the command line wins.
#[derive(clap::Args, Debug)]
struct EmbeddedArgs {
    /// Reformat `#script` bodies with their language's formatter.
    #[arg(long = "format-embedded", overrides_with = "no_format_embedded")]
    format_embedded: bool,

    /// Hold `#script` bodies verbatim (the default).
    #[arg(long = "no-format-embedded", overrides_with = "format_embedded")]
    no_format_embedded: bool,
}

impl EmbeddedArgs {
    /// The explicit choice, if any (§10.2): `--format-embedded` is `Some(true)`,
    /// `--no-format-embedded` is `Some(false)`, neither is `None` — the config
    /// or the default then decides. The `overrides_with` pairing leaves at most
    /// one flag set, the later on the command line.
    fn choice(&self) -> Option<bool> {
        if self.format_embedded {
            Some(true)
        } else if self.no_format_embedded {
            Some(false)
        } else {
            None
        }
    }
}

/// The opt-in reorder (§10.1, §15): organize a program's top-level signature,
/// off by default. `--reorder-includes` is the one **unsafe** toggle (§15.4) —
/// the caller's assertion that their `#include`s carry no order-sensitive
/// construct, which morphe cannot prove; it implies `--reorder`.
#[derive(clap::Args, Debug)]
struct ReorderArgs {
    /// Reorder top-level statements into the canonical signature order.
    #[arg(long)]
    reorder: bool,

    /// Also hoist #include (unsafe; implies --reorder).
    #[arg(long)]
    reorder_includes: bool,
}

impl ReorderArgs {
    /// Whether the run reorders: `--reorder`, or `--reorder-includes` (which
    /// implies it). The config can also ask for it (§10.2).
    fn wanted(&self) -> bool {
        self.reorder || self.reorder_includes
    }
}

/// Input selection for a directory walk (§10.1): the exclude globs and whether
/// to honour the ignore files.
#[derive(clap::Args, Debug)]
struct InputArgs {
    /// Glob patterns to skip when walking a directory; repeatable.
    #[arg(long, value_name = "GLOB")]
    exclude: Vec<String>,

    /// Further glob patterns to skip, in addition to --exclude; repeatable.
    #[arg(long, value_name = "GLOB")]
    extend_exclude: Vec<String>,

    /// Walk files that .gitignore or .ignore would exclude.
    #[arg(long)]
    no_respect_gitignore: bool,

    /// The assumed path for stdin (config, diagnostics, diff headers).
    #[arg(long, value_name = "PATH")]
    stdin_filename: Option<PathBuf>,
}

/// Config discovery (§10.2): an explicit file, or none at all.
#[derive(clap::Args, Debug)]
struct ConfigArgs {
    /// Read configuration from this file instead of discovering a morphe.toml.
    #[arg(long = "config", value_name = "PATH")]
    path: Option<PathBuf>,

    /// Ignore any morphe.toml; use the built-in defaults and these flags alone.
    #[arg(long, conflicts_with = "path")]
    isolated: bool,
}

/// Reporting (§10.1): quieter, or per-file detail. `-V`/`--version` stays the
/// class convention (clap's own); `-v` is verbosity, never version.
#[derive(clap::Args, Debug)]
struct ReportArgs {
    /// Suppress the summary and changed-file list (errors still shown).
    #[arg(short = 'q', long)]
    quiet: bool,

    /// List every input's status, not only the changed.
    #[arg(short = 'v', long)]
    verbose: bool,

    /// When to colour terminal output: `auto` (a terminal, honouring
    /// `NO_COLOR`), `always`, or `never` (§10.1).
    #[arg(long, value_enum, default_value = "auto")]
    color: color::ColorChoice,
}

/// Run the formatter over the command line, returning the process [`Outcome`]
/// (§10.3). The binary hands the value to the runtime, which turns it into an
/// exit code through the one [`Termination`](std::process::Termination) impl
/// (§10.4). A run over several inputs reports the worst outcome (§10.3).
#[must_use]
pub fn run() -> Outcome {
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error) => return report_parse(&error),
    };
    let config_source = match resolve_config_source(&cli) {
        Ok(config_source) => config_source,
        Err(outcome) => return outcome,
    };

    let reads_stdin = cli.paths.iter().any(|path| is_stdin(path));
    let files: Vec<&Path> = cli
        .paths
        .iter()
        .map(PathBuf::as_path)
        .filter(|path| !is_stdin(path))
        .collect();
    if reads_stdin && !files.is_empty() {
        eprintln!("morphe: cannot mix '-' (stdin) with file paths");
        return Outcome::UserError;
    }
    if reads_stdin || (cli.paths.is_empty() && !io::stdin().is_terminal()) {
        return format_stdin(&cli, &config_source);
    }
    if files.is_empty() {
        eprintln!("morphe: no input; give a path, or pipe stdin");
        return Outcome::UserError;
    }
    let walk_options = walk::Options {
        excludes: cli
            .input
            .exclude
            .iter()
            .chain(&cli.input.extend_exclude)
            .cloned()
            .collect(),
        respect_gitignore: !cli.input.no_respect_gitignore,
    };
    let targets = match walk::collect(&files, &walk_options) {
        Ok(targets) => targets,
        Err(error) => {
            eprintln!("morphe: {error}");
            return Outcome::UserError;
        }
    };
    let mut worst = Outcome::Clean;
    let mut summary = Summary::new(cli.mode.check || cli.mode.diff, cli.report.verbose);
    for path in &targets {
        let report = format_file(&cli, &config_source, path);
        worst = worst.max(report.outcome);
        summary.record(path, report.changed);
    }
    if !cli.report.quiet {
        summary.report();
    }
    worst
}

/// One input's result for the run summary (§10.1): its process outcome, and
/// whether it changed — `None` when an error stopped it before that was known.
struct FileReport {
    outcome: Outcome,
    changed: Option<bool>,
}

impl FileReport {
    /// An I/O or usage error stopped this file before formatting could decide
    /// anything: a user error, left out of the summary tally.
    fn errored() -> FileReport {
        FileReport {
            outcome: Outcome::UserError,
            changed: None,
        }
    }
}

/// The running tally a file/directory run reports at the end (§10.1): the files
/// reformatted (or, under --check/--diff, that would be) and those already in
/// morphe-form. An errored file is reported where it occurs and left out of the
/// tally. The changed-file list always names the reformatted; under `--verbose`
/// it names the unchanged too.
struct Summary {
    would_change: bool,
    verbose: bool,
    reformatted: Vec<PathBuf>,
    unchanged: Vec<PathBuf>,
}

impl Summary {
    /// `would_change` when the run only reports (--check/--diff) and so counts
    /// what *would* be reformatted rather than what was; `verbose` lists the
    /// unchanged files as well as the changed.
    fn new(would_change: bool, verbose: bool) -> Summary {
        Summary {
            would_change,
            verbose,
            reformatted: Vec::new(),
            unchanged: Vec::new(),
        }
    }

    /// Record one file's change status; an errored file (`None`) is not tallied.
    fn record(&mut self, path: &Path, changed: Option<bool>) {
        match changed {
            Some(true) => self.reformatted.push(path.to_owned()),
            Some(false) => self.unchanged.push(path.to_owned()),
            None => {}
        }
    }

    /// Print the changed-file list and the summary line to stderr (§10.1), so
    /// stdout carries only formatted output or a diff. Nothing when the run
    /// tallied no file.
    fn report(&self) {
        if self.reformatted.is_empty() && self.unchanged.is_empty() {
            return;
        }
        let verb = if self.would_change {
            "would reformat"
        } else {
            "reformatted"
        };
        for path in &self.reformatted {
            eprintln!("{verb} {}", path.display());
        }
        if self.verbose {
            for path in &self.unchanged {
                eprintln!("unchanged {}", path.display());
            }
        }
        let changed = self.reformatted.len();
        if self.would_change {
            eprintln!("{changed} would be reformatted");
        } else {
            eprintln!("{changed} reformatted, {} unchanged", self.unchanged.len());
        }
    }
}

/// Where each file's configuration comes from (§10.2): discovered per file,
/// fixed by `--config`, or none under `--isolated`.
enum ConfigSource {
    /// `--isolated`: no config file — the built-in defaults and the flags alone.
    Isolated,
    /// `--config <path>`: one file governs every input.
    Fixed(config::Config),
    /// The default: the nearest `morphe.toml` walking up from each file.
    Discover,
}

/// Resolve where configuration comes from, reading the `--config` file once
/// (§10.2). A bad `--config` file is a usage error ([`Outcome::UserError`]).
fn resolve_config_source(cli: &Cli) -> Result<ConfigSource, Outcome> {
    if cli.config.isolated {
        return Ok(ConfigSource::Isolated);
    }
    if let Some(path) = &cli.config.path {
        return match config::load(path) {
            Ok(config) => Ok(ConfigSource::Fixed(config)),
            Err(error) => {
                eprintln!("morphe: {error}");
                Err(Outcome::UserError)
            }
        };
    }
    Ok(ConfigSource::Discover)
}

/// The config governing an input, per the source (§10.2). `assumed` is the
/// directory discovery walks up from — a file's own directory, or the assumed
/// directory of stdin; `None` (bare stdin, no assumed path) discovers nothing.
fn config_for(
    config_source: &ConfigSource,
    assumed: Option<&Path>,
) -> Result<config::Config, config::ConfigError> {
    match config_source {
        ConfigSource::Isolated => Ok(config::Config::default()),
        ConfigSource::Fixed(config) => Ok(config.clone()),
        ConfigSource::Discover => match assumed {
            Some(directory) => config::discover(directory),
            None => Ok(config::Config::default()),
        },
    }
}

/// Merge the command-line flags over a file's config into the [`FormatOptions`]
/// the formatter reads (§10.2): a set value option wins over the config, which
/// wins over the built-in default; a boolean flag ORs with the config's.
fn resolve_options(cli: &Cli, config: &config::Config) -> FormatOptions {
    let format_embedded = cli
        .embedded
        .choice()
        .or(config.format_embedded)
        .unwrap_or(false);
    FormatOptions {
        line_width: cli
            .style
            .line_width
            .or(config.line_width)
            .unwrap_or(DEFAULT_LINE_WIDTH),
        dialect: cli
            .style
            .dialect
            .map(Dialect::from)
            .or(config.dialect.map(Dialect::from))
            .unwrap_or(Dialect::Clingo),
        preserve_spellings: cli.style.preserve_spellings
            || config.preserve_spellings.unwrap_or(false),
        format_embedded,
        // The registry is assembled only when the run will consult it (§9.2);
        // off, every `#script` stays verbatim without constructing a backend.
        embedded: if format_embedded {
            default_formatters()
        } else {
            EmbeddedFormatters::default()
        },
        // The one unsafe toggle (§15.4): the flag, or the config's `[unsafe]`
        // section. Read only when reordering.
        reorder_includes: cli.reorder.reorder_includes
            || config.unsafe_toggles.reorder_includes.unwrap_or(false),
    }
}

/// Whether this run reorders (§10.1, §15): the `--reorder` flag, `--reorder-includes`
/// (which implies it), or the config's `reorder`/`[unsafe] reorder_includes` —
/// the unsafe toggle implies reordering wherever it is set.
fn wants_reorder(cli: &Cli, config: &config::Config) -> bool {
    cli.reorder.wanted()
        || config.reorder.unwrap_or(false)
        || config.unsafe_toggles.reorder_includes.unwrap_or(false)
}

/// The output of formatting or reordering a source: the text, whether it differs
/// from the input, and any warning notes (§9.4, §15).
struct Produced {
    text: String,
    changed: bool,
    notes: Vec<morphe::Diagnostic>,
}

/// Format, or when `reorder` reorder, the source (§10.1, §15). Reordering routes
/// to [`morphe::reorder`] instead of [`format`]; either refusal is rendered
/// through base's human view and mapped to its [`Outcome`] (§10.3, §11).
///
/// # Errors
///
/// The already-reported [`Outcome`] on a refusal — nothing is left to print.
fn produce(
    source: &Source,
    options: &FormatOptions,
    reorder: bool,
    label: &str,
    choice: color::ColorChoice,
) -> Result<Produced, Outcome> {
    if reorder {
        match morphe::reorder(source, options) {
            Ok(reordered) => Ok(Produced {
                text: reordered.text,
                changed: reordered.changed,
                notes: reordered.notes,
            }),
            Err(error) => Err(report_reorder_error(&error, label, source.text(), choice)),
        }
    } else {
        match format(source, options) {
            Ok(formatted) => Ok(Produced {
                text: formatted.text,
                changed: formatted.changed,
                notes: formatted.notes,
            }),
            Err(error) => Err(report_format_error(&error, label, source.text(), choice)),
        }
    }
}

/// The default embedded backends the CLI supplies (§9.1, §9.6): ruff for Python
/// and `StyLua` for Lua, each present only when its Cargo feature is compiled in.
/// The ASP-only build wires neither, so even with `--format-embedded` set an
/// unbacked `#script` degrades to verbatim with a note (§9.4).
fn default_formatters() -> EmbeddedFormatters {
    let backends: Vec<(LanguageKey, Box<dyn EmbeddedFormatter>)> = vec![
        #[cfg(feature = "embedded-python")]
        (
            LanguageKey::new("python"),
            Box::new(RuffLibBackend::default()) as Box<dyn EmbeddedFormatter>,
        ),
        #[cfg(feature = "embedded-lua")]
        (
            LanguageKey::new("lua"),
            Box::new(StyLuaLibBackend::default()) as Box<dyn EmbeddedFormatter>,
        ),
    ];
    backends.into_iter().collect()
}

/// The directory config discovery begins from for `path` — its parent, or the
/// current directory when `path` names a bare file.
fn directory_of(path: &Path) -> &Path {
    match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    }
}

/// Whether a path argument denotes stdin (the literal `-`, §10.1).
fn is_stdin(path: &Path) -> bool {
    path.as_os_str() == "-"
}

/// Format stdin to stdout, never in place (§10.1): print the formatted text, or
/// under `--check`/`--diff` write nothing and signal whether it would change.
fn format_stdin(cli: &Cli, config_source: &ConfigSource) -> Outcome {
    // The assumed path (`--stdin-filename`) names stdin in every message and
    // roots its config discovery; bare stdin falls back to `<stdin>` and
    // discovers no config (§10.1).
    let stdin_path = cli.input.stdin_filename.as_deref();
    let label =
        stdin_path.map_or_else(|| STDIN_LABEL.to_owned(), |path| path.display().to_string());
    let mut input = String::new();
    if let Err(error) = io::stdin().read_to_string(&mut input) {
        eprintln!("morphe: {label}: {error}");
        return Outcome::UserError;
    }
    let config = match config_for(config_source, stdin_path.map(directory_of)) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("morphe: {error}");
            return Outcome::UserError;
        }
    };
    let options = resolve_options(cli, &config);
    let source = match Source::new(INPUT_SOURCE_ID, input) {
        Ok(source) => source,
        Err(too_large) => {
            eprintln!("morphe: {label}: {too_large}");
            return Outcome::UserError;
        }
    };
    let reorder = wants_reorder(cli, &config);
    let produced = match produce(&source, &options, reorder, &label, cli.report.color) {
        Ok(produced) => produced,
        Err(outcome) => return outcome,
    };
    print_diagnostics(&label, source.text(), &produced.notes, cli.report.color);
    if cli.mode.check || cli.mode.diff {
        if cli.mode.diff {
            let colored = color::enabled(cli.report.color, io::stdout().is_terminal());
            print!(
                "{}",
                diff::unified(source.text(), &produced.text, &label, &label, colored)
            );
        }
        return changes(produced.changed);
    }
    print!("{}", produced.text);
    Outcome::Clean
}

/// Format one file: in place and atomically by default, or under
/// `--check`/`--diff` write nothing and signal whether it would change (§10.1).
/// Returns the file's outcome and whether it changed, for the run summary.
fn format_file(cli: &Cli, config_source: &ConfigSource, path: &Path) -> FileReport {
    let label = path.display().to_string();
    let input = match fs::read_to_string(path) {
        Ok(input) => input,
        Err(error) => {
            eprintln!("morphe: {label}: {error}");
            return FileReport::errored();
        }
    };
    let config = match config_for(config_source, Some(directory_of(path))) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("morphe: {error}");
            return FileReport::errored();
        }
    };
    let options = resolve_options(cli, &config);
    let source = match Source::new(INPUT_SOURCE_ID, input) {
        Ok(source) => source,
        Err(too_large) => {
            eprintln!("morphe: {label}: {too_large}");
            return FileReport::errored();
        }
    };
    let reorder = wants_reorder(cli, &config);
    let produced = match produce(&source, &options, reorder, &label, cli.report.color) {
        Ok(produced) => produced,
        Err(outcome) => {
            return FileReport {
                outcome,
                changed: None,
            };
        }
    };
    print_diagnostics(&label, source.text(), &produced.notes, cli.report.color);
    if cli.mode.check || cli.mode.diff {
        if cli.mode.diff {
            let colored = color::enabled(cli.report.color, io::stdout().is_terminal());
            print!(
                "{}",
                diff::unified(source.text(), &produced.text, &label, &label, colored)
            );
        }
        return FileReport {
            outcome: changes(produced.changed),
            changed: Some(produced.changed),
        };
    }
    if produced.changed
        && let Err(error) = atomic::replace(path, &produced.text)
    {
        eprintln!("morphe: {label}: {error}");
        return FileReport::errored();
    }
    FileReport {
        outcome: Outcome::Clean,
        changed: Some(produced.changed),
    }
}

/// The `--check`/`--diff` outcome: nothing was written, and the exit status
/// alone reports whether the input was already in morphe-form (§10.1, §10.3).
fn changes(changed: bool) -> Outcome {
    if changed {
        Outcome::ChangesNeeded
    } else {
        Outcome::Clean
    }
}

/// Render `diagnostics` through base's human view (§11.1) and print them to
/// stderr — the tier's parse-error `SyntaxError`s, or the warning-level embedded
/// notes (§9.4). Painted when stderr is a colour-bearing terminal, else the
/// canonical colourless text (§10.1). `text` is the input they point into.
/// Nothing when there are none.
fn print_diagnostics(
    label: &str,
    text: &str,
    diagnostics: &[morphe::Diagnostic],
    choice: color::ColorChoice,
) {
    if diagnostics.is_empty() {
        return;
    }
    let rendered = report::render(label, text, diagnostics);
    let painted = if color::enabled(choice, io::stderr().is_terminal()) {
        color::diagnostics(&rendered)
    } else {
        rendered
    };
    eprint!("{painted}");
}

/// Report a `format` refusal and classify it (§10.3). A parse error forwards
/// the tier's own `SyntaxError`s, rendered through base's human view (§11.1);
/// `text` is the input the diagnostics point into. A broken certificate is a
/// morphe bug, printed with a bug-report ask. The exit classification is
/// [`Outcome`]'s own (§10.3).
fn report_format_error(
    error: &FormatError,
    label: &str,
    text: &str,
    choice: color::ColorChoice,
) -> Outcome {
    match error {
        FormatError::HasParseErrors(errors) => {
            print_diagnostics(label, text, &report::parse_errors(errors), choice);
        }
        FormatError::CertificateBroke { .. } => {
            eprintln!(
                "morphe: {label}: internal error — morphe produced output it could not certify."
            );
            eprintln!(
                "This is a bug in morphe, not a problem with your file; please report it with the input that triggered it."
            );
        }
    }
    Outcome::from(error)
}

/// Report a `reorder` refusal and classify it (§10.3, §15.3). As with `format`,
/// a parse error forwards the tier's `SyntaxError`s through base's human view
/// (§11.1); a broken permutation certificate is a morphe bug, printed with a
/// bug-report ask. The exit classification is [`Outcome`]'s own (§10.3).
fn report_reorder_error(
    error: &ReorderError,
    label: &str,
    text: &str,
    choice: color::ColorChoice,
) -> Outcome {
    match error {
        ReorderError::HasParseErrors(errors) => {
            print_diagnostics(label, text, &report::parse_errors(errors), choice);
        }
        ReorderError::PermutationBroke { .. } => {
            eprintln!(
                "morphe: {label}: internal error — morphe produced a reorder it could not certify."
            );
            eprintln!(
                "This is a bug in morphe, not a problem with your file; please report it with the input that triggered it."
            );
        }
    }
    Outcome::from(error)
}

/// Route a `clap` parse result through the outcome contract (§10.3, §10.4):
/// `clap` prints help and version to stdout (a clean exit) and a usage error to
/// stderr (a user error).
fn report_parse(error: &clap::Error) -> Outcome {
    let _ = error.print();
    if error.use_stderr() {
        Outcome::UserError
    } else {
        Outcome::Clean
    }
}
