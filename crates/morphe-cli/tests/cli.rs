//! The CLI's end-to-end behaviour (docs/design/morphe.md §10.1, §10.3): each
//! test drives the built `morphe` binary over stdin or real files and checks
//! its bytes, its writes, and its process outcome. Exit statuses are named
//! through [`Outcome`] rather than restated as raw integers (§10.4).

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use morphe_cli::outcome::Outcome;
use tempfile::TempDir;

/// The built binary under test; Cargo sets `CARGO_BIN_EXE_<name>` for the
/// integration test of the crate that owns the binary.
const MORPHE: &str = env!("CARGO_BIN_EXE_morphe");

/// One run of the binary: its process exit code and the two captured streams.
struct Run {
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Run `morphe` with the given arguments and stdin, capturing both streams.
fn morphe(args: &[&str], stdin: &str) -> Run {
    let mut child = Command::new(MORPHE)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn the morphe binary");
    child
        .stdin
        .take()
        .expect("the child's stdin")
        .write_all(stdin.as_bytes())
        .expect("write to the child's stdin");
    let output = child
        .wait_with_output()
        .expect("wait for the morphe binary");
    Run {
        code: output.status.code(),
        stdout: String::from_utf8(output.stdout).expect("stdout is UTF-8"),
        stderr: String::from_utf8(output.stderr).expect("stderr is UTF-8"),
    }
}

/// The process exit code an [`Outcome`] resolves to — named, never a bare
/// integer (§10.4). Wrapped as `Some(..)` at the assertion, matching the
/// `Option` a process exit code arrives as.
fn code(outcome: Outcome) -> i32 {
    i32::from(outcome.exit_code())
}

/// Write a fixture file into `dir`, returning its path.
fn fixture(dir: &Path, name: &str, contents: &str) -> PathBuf {
    let path = dir.join(name);
    fs::write(&path, contents).expect("write the fixture file");
    path
}

/// The files present in a directory, by name — so a test can assert an atomic
/// write left no temp residue beside the target.
fn entries(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .expect("read the directory")
        .map(|entry| {
            entry
                .expect("a directory entry")
                .file_name()
                .into_string()
                .unwrap()
        })
        .collect();
    names.sort();
    names
}

/// The path of a golden file under `tests/golden`. Set `GOLDEN_BLESS` to
/// rewrite it from the run's actual output, as the tier's own golden tests do.
fn golden_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden")
        .join(name)
}

/// Assert `actual` matches the named golden, or rewrite it under `GOLDEN_BLESS`.
fn assert_golden(name: &str, actual: &str) {
    let path = golden_path(name);
    if std::env::var_os("GOLDEN_BLESS").is_some() {
        fs::write(&path, actual).expect("write the golden file");
        return;
    }
    let expected = fs::read_to_string(&path).expect("the golden file is shipped");
    assert_eq!(actual, expected, "output drifted from {name}");
}

/// `a :- b.` is already in morphe-form; `a:-b.` is the same member unformatted;
/// `p(` is a non-member (a parse error). All three verified against the library.
const FORMATTED: &str = "a :- b.\n";
const UNFORMATTED: &str = "a:-b.\n";
const NON_MEMBER: &str = "p(\n";

/// A rule already in morphe-form at the default width 100 (so `--check` is
/// clean there) that wraps at a narrow width — the witness that a configured
/// `line_width` reached the formatter. Verified against the library.
const FITS_AT_100: &str = "a :- b, c, d, e, f, g, h.\n";

#[test]
fn stdin_formats_to_stdout_leaving_a_formatted_member_unchanged() {
    let run = morphe(&[], FORMATTED);
    assert_eq!(
        run.stdout, FORMATTED,
        "a formatted member passes through byte-for-byte"
    );
    assert_eq!(run.code, Some(code(Outcome::Clean)));
}

#[test]
fn stdin_reformats_an_unformatted_member_to_stdout() {
    let run = morphe(&[], UNFORMATTED);
    assert_eq!(
        run.stdout, FORMATTED,
        "stdin is laid out and written to stdout"
    );
    assert_eq!(run.code, Some(code(Outcome::Clean)));
}

#[test]
fn check_on_a_would_change_member_signals_changes_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "in.lp", UNFORMATTED);
    let run = morphe(&["--check", path.to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::ChangesNeeded)));
    assert!(run.stdout.is_empty(), "--check writes no formatted output");
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        UNFORMATTED,
        "--check leaves the file untouched"
    );
}

#[test]
fn check_on_a_formatted_member_is_clean() {
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "in.lp", FORMATTED);
    let run = morphe(&["--check", path.to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert_eq!(fs::read_to_string(&path).unwrap(), FORMATTED);
}

#[test]
fn a_parse_error_file_is_a_user_error_and_touches_nothing() {
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "bad.lp", NON_MEMBER);
    let run = morphe(&[path.to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::UserError)));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        NON_MEMBER,
        "a non-member is left untouched"
    );
    assert!(
        !run.stderr.is_empty(),
        "the refusal is reported to the user"
    );
}

#[test]
fn an_in_place_format_replaces_the_file_atomically() {
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "in.lp", UNFORMATTED);
    let run = morphe(&[path.to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        FORMATTED,
        "the file now holds the formatted text"
    );
    assert_eq!(
        entries(dir.path()),
        vec!["in.lp"],
        "no temp file is left beside the target"
    );
}

#[test]
fn diff_prints_a_unified_diff_signals_changes_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "in.lp", UNFORMATTED);
    let run = morphe(&["--diff", path.to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::ChangesNeeded)));
    assert!(
        run.stdout.contains("@@"),
        "the output is a unified diff:\n{}",
        run.stdout
    );
    assert!(
        run.stdout.contains("+a :- b."),
        "the reformatted line is shown added"
    );
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        UNFORMATTED,
        "--diff leaves the file untouched"
    );
}

#[test]
fn check_and_diff_combine() {
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "in.lp", UNFORMATTED);
    let run = morphe(&["--check", "--diff", path.to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::ChangesNeeded)));
    assert!(
        run.stdout.contains("@@"),
        "the diff is still printed under --check"
    );
    assert_eq!(fs::read_to_string(&path).unwrap(), UNFORMATTED);
}

#[test]
fn color_forces_the_diff_paint_while_the_default_follows_the_stream() {
    // stdout here is a pipe, not a terminal, so `auto` (the default) stays
    // colourless; `--color` is what forces the choice either way (§10.1). The
    // ESC that opens every SGR sequence is the tell that colour was emitted.
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "in.lp", UNFORMATTED);
    let arg = path.to_str().unwrap();

    let always = morphe(&["--diff", "--color=always", arg], "");
    assert!(
        always.stdout.contains('\u{1b}'),
        "--color=always paints the diff even when stdout is not a terminal:\n{}",
        always.stdout
    );

    let never = morphe(&["--diff", "--color=never", arg], "");
    assert!(
        !never.stdout.contains('\u{1b}'),
        "--color=never leaves the diff plain"
    );

    let auto = morphe(&["--diff", arg], "");
    assert!(
        !auto.stdout.contains('\u{1b}'),
        "the default follows the stream: a piped stdout is no terminal, so plain"
    );
}

#[test]
fn a_directory_argument_formats_every_lp_member_in_place() {
    let dir = TempDir::new().unwrap();
    let member = fixture(dir.path(), "a.lp", UNFORMATTED);
    let other = fixture(dir.path(), "note.txt", UNFORMATTED);
    let run = morphe(&[dir.path().to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert_eq!(
        fs::read_to_string(&member).unwrap(),
        FORMATTED,
        "the .lp member is reformatted in place"
    );
    assert_eq!(
        fs::read_to_string(&other).unwrap(),
        UNFORMATTED,
        "a non-.lp file in the directory is left untouched"
    );
}

#[test]
fn a_directory_walk_respects_gitignore() {
    let dir = TempDir::new().unwrap();
    fixture(dir.path(), ".gitignore", "skip.lp\n");
    let skipped = fixture(dir.path(), "skip.lp", UNFORMATTED);
    let kept = fixture(dir.path(), "keep.lp", UNFORMATTED);
    let run = morphe(&[dir.path().to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert_eq!(
        fs::read_to_string(&kept).unwrap(),
        FORMATTED,
        "a non-ignored member is reformatted"
    );
    assert_eq!(
        fs::read_to_string(&skipped).unwrap(),
        UNFORMATTED,
        "a gitignored member is not walked"
    );
}

#[test]
fn exclude_globs_skip_matching_members() {
    let dir = TempDir::new().unwrap();
    let excluded = fixture(dir.path(), "gen.lp", UNFORMATTED);
    let extended = fixture(dir.path(), "vendor.lp", UNFORMATTED);
    let kept = fixture(dir.path(), "keep.lp", UNFORMATTED);
    let run = morphe(
        &[
            "--exclude",
            "gen.lp",
            "--extend-exclude",
            "vendor.lp",
            dir.path().to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert_eq!(
        fs::read_to_string(&kept).unwrap(),
        FORMATTED,
        "a non-excluded member is reformatted"
    );
    assert_eq!(
        fs::read_to_string(&excluded).unwrap(),
        UNFORMATTED,
        "--exclude skips its match"
    );
    assert_eq!(
        fs::read_to_string(&extended).unwrap(),
        UNFORMATTED,
        "--extend-exclude skips its match"
    );
}

#[test]
fn no_respect_gitignore_walks_ignored_members() {
    let dir = TempDir::new().unwrap();
    fixture(dir.path(), ".gitignore", "skip.lp\n");
    let skipped = fixture(dir.path(), "skip.lp", UNFORMATTED);
    let run = morphe(
        &["--no-respect-gitignore", dir.path().to_str().unwrap()],
        "",
    );
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert_eq!(
        fs::read_to_string(&skipped).unwrap(),
        FORMATTED,
        "--no-respect-gitignore formats a would-be-ignored member"
    );
}

#[test]
fn a_non_lp_file_argument_is_formatted_as_is() {
    // The `*.lp` match filters a directory walk; a file named directly is
    // taken as-is, whatever its extension (§10.1).
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "prog.asp", UNFORMATTED);
    let run = morphe(&[path.to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert_eq!(
        fs::read_to_string(&path).unwrap(),
        FORMATTED,
        "a file named directly is formatted whatever its extension"
    );
}

#[test]
fn a_nearest_morphe_toml_line_width_is_applied() {
    let dir = TempDir::new().unwrap();
    fixture(dir.path(), "morphe.toml", "line_width = 20\n");
    let path = fixture(dir.path(), "wide.lp", FITS_AT_100);
    let run = morphe(&["--check", path.to_str().unwrap()], "");
    assert_eq!(
        run.code,
        Some(code(Outcome::ChangesNeeded)),
        "the discovered config's narrow width makes the wide rule need reformatting; stderr: {}",
        run.stderr
    );
}

#[test]
fn config_flag_overrides_a_discovered_morphe_toml() {
    let dir = TempDir::new().unwrap();
    // A discovered config that would leave the wide rule alone (width 100)...
    fixture(dir.path(), "morphe.toml", "line_width = 100\n");
    let path = fixture(dir.path(), "wide.lp", FITS_AT_100);
    // ...is overridden by --config's narrow width, so the rule now wraps.
    let explicit = fixture(dir.path(), "narrow.toml", "line_width = 20\n");
    let run = morphe(
        &[
            "--check",
            "--config",
            explicit.to_str().unwrap(),
            path.to_str().unwrap(),
        ],
        "",
    );
    assert_eq!(
        run.code,
        Some(code(Outcome::ChangesNeeded)),
        "--config's width wins over the discovered morphe.toml; stderr: {}",
        run.stderr
    );
}

#[test]
fn isolated_ignores_a_discovered_morphe_toml() {
    let dir = TempDir::new().unwrap();
    // A config that would wrap the wide rule at width 20...
    fixture(dir.path(), "morphe.toml", "line_width = 20\n");
    let path = fixture(dir.path(), "wide.lp", FITS_AT_100);
    // ...is ignored under --isolated, so the default width 100 leaves it clean.
    let run = morphe(&["--isolated", "--check", path.to_str().unwrap()], "");
    assert_eq!(
        run.code,
        Some(code(Outcome::Clean)),
        "--isolated ignores the morphe.toml and formats at the default width; stderr: {}",
        run.stderr
    );
}

#[test]
fn an_unknown_config_key_is_a_user_error() {
    let dir = TempDir::new().unwrap();
    fixture(dir.path(), "morphe.toml", "line_widht = 20\n");
    let path = fixture(dir.path(), "in.lp", UNFORMATTED);
    let run = morphe(&["--check", path.to_str().unwrap()], "");
    assert_eq!(
        run.code,
        Some(code(Outcome::UserError)),
        "an unknown config key is refused, not silently dropped; stderr: {}",
        run.stderr
    );
    assert!(
        !run.stderr.is_empty(),
        "the config error is reported to the user"
    );
}

#[test]
fn stdin_filename_labels_the_diff_header() {
    let run = morphe(&["--diff", "--stdin-filename", "prog.lp"], UNFORMATTED);
    assert_eq!(run.code, Some(code(Outcome::ChangesNeeded)));
    assert!(
        run.stdout.contains("prog.lp"),
        "the diff header carries the assumed filename:\n{}",
        run.stdout
    );
}

#[test]
fn a_parse_error_renders_through_base_human_view() {
    // Fed through stdin with a fixed assumed filename, so the rendered
    // location is deterministic and the golden is stable (§11.1).
    let run = morphe(&["--stdin-filename", "bad.lp"], NON_MEMBER);
    assert_eq!(run.code, Some(code(Outcome::UserError)));
    assert!(
        run.stdout.is_empty(),
        "a refusal writes no formatted output"
    );
    assert_golden("parse-error-diagnostic.txt", &run.stderr);
}

#[test]
fn a_run_prints_a_summary_and_the_changed_files() {
    let dir = TempDir::new().unwrap();
    fixture(dir.path(), "wrong.lp", UNFORMATTED);
    fixture(dir.path(), "right.lp", FORMATTED);
    let run = morphe(&[dir.path().to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert!(
        run.stderr.contains("1 reformatted") && run.stderr.contains("1 unchanged"),
        "the summary reports the counts:\n{}",
        run.stderr
    );
    assert!(
        run.stderr.contains("wrong.lp"),
        "the changed-file list names the reformatted file:\n{}",
        run.stderr
    );
}

#[test]
fn quiet_suppresses_the_summary() {
    let dir = TempDir::new().unwrap();
    fixture(dir.path(), "wrong.lp", UNFORMATTED);
    let run = morphe(&["-q", dir.path().to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert!(
        !run.stderr.contains("reformatted"),
        "-q suppresses the summary; stderr: {}",
        run.stderr
    );
}

#[test]
fn verbose_names_unchanged_files_too() {
    let dir = TempDir::new().unwrap();
    fixture(dir.path(), "right.lp", FORMATTED);
    let run = morphe(&["-v", dir.path().to_str().unwrap()], "");
    assert_eq!(run.code, Some(code(Outcome::Clean)));
    assert!(
        run.stderr.contains("right.lp"),
        "-v names the unchanged file, not only the changed; stderr: {}",
        run.stderr
    );
}

#[test]
fn format_embedded_notes_an_unsupported_script_language() {
    // `fortran` has no backend in any build, so --format-embedded keeps its body
    // verbatim and records a note, rendered through base's view (§9.4, §11).
    let input = "#script(fortran)\ncode here\n#end.\n";
    let run = morphe(&["--format-embedded", "--stdin-filename", "e.lp"], input);
    assert_eq!(
        run.code,
        Some(code(Outcome::Clean)),
        "an embedded skip is warning-level, not an error; stderr: {}",
        run.stderr
    );
    assert!(
        run.stderr.contains("fortran"),
        "the unsupported script language is noted:\n{}",
        run.stderr
    );
}

#[cfg(feature = "embedded-python")]
#[test]
fn format_embedded_reformats_a_python_script() {
    // With the python backend compiled in, --format-embedded hands the body to
    // ruff, which spaces the assignment (§9.6). Verified against the backend.
    let input = "#script(python)\nx=1\n#end.\n";
    let run = morphe(&["--format-embedded", "--stdin-filename", "e.lp"], input);
    assert_eq!(
        run.code,
        Some(code(Outcome::Clean)),
        "stderr: {}",
        run.stderr
    );
    assert!(
        run.stdout.contains("x = 1"),
        "ruff reformats the python body:\n{}",
        run.stdout
    );
}

#[test]
fn reorder_moves_a_show_after_the_rules() {
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "p.lp", "#show a/1.\na :- b.\n");
    let run = morphe(&["--reorder", path.to_str().unwrap()], "");
    assert_eq!(
        run.code,
        Some(code(Outcome::Clean)),
        "stderr: {}",
        run.stderr
    );
    let out = fs::read_to_string(&path).unwrap();
    assert!(
        out.find("a :- b.").unwrap() < out.find("#show a/1.").unwrap(),
        "the rule now precedes the #show:\n{out}"
    );
}

#[test]
fn reorder_includes_hoists_an_include() {
    // --reorder-includes implies --reorder and hoists #include (the unsafe
    // toggle, §15.4).
    let dir = TempDir::new().unwrap();
    let path = fixture(dir.path(), "p.lp", "a :- b.\n#include \"lib.lp\".\n");
    let run = morphe(&["--reorder-includes", path.to_str().unwrap()], "");
    assert_eq!(
        run.code,
        Some(code(Outcome::Clean)),
        "stderr: {}",
        run.stderr
    );
    let out = fs::read_to_string(&path).unwrap();
    assert!(
        out.find("#include").unwrap() < out.find("a :- b.").unwrap(),
        "the include is hoisted above the rule:\n{out}"
    );
    assert!(
        run.stderr.contains("reorder_includes moved") && run.stderr.contains("#include"),
        "the hoist warns on stderr, never silent (§15.4):\n{}",
        run.stderr
    );
}

#[test]
fn config_unsafe_reorder_includes_hoists_an_include() {
    // The config's [unsafe] section supplies the reorder_includes toggle and,
    // being an unsafe tenant, implies the reorder (§10.2/§15.4).
    let dir = TempDir::new().unwrap();
    fixture(
        dir.path(),
        "morphe.toml",
        "[unsafe]\nreorder_includes = true\n",
    );
    let path = fixture(dir.path(), "p.lp", "a :- b.\n#include \"lib.lp\".\n");
    let run = morphe(&[path.to_str().unwrap()], "");
    assert_eq!(
        run.code,
        Some(code(Outcome::Clean)),
        "stderr: {}",
        run.stderr
    );
    let out = fs::read_to_string(&path).unwrap();
    assert!(
        out.find("#include").unwrap() < out.find("a :- b.").unwrap(),
        "the config's [unsafe] toggle hoists the include:\n{out}"
    );
}

#[test]
fn config_reorder_enables_the_reorder() {
    // The safe top-level `reorder` config key turns the reorder on with no flag.
    let dir = TempDir::new().unwrap();
    fixture(dir.path(), "morphe.toml", "reorder = true\n");
    let path = fixture(dir.path(), "p.lp", "#show a/1.\na :- b.\n");
    let run = morphe(&[path.to_str().unwrap()], "");
    assert_eq!(
        run.code,
        Some(code(Outcome::Clean)),
        "stderr: {}",
        run.stderr
    );
    let out = fs::read_to_string(&path).unwrap();
    assert!(
        out.find("a :- b.").unwrap() < out.find("#show a/1.").unwrap(),
        "the config reordered the program:\n{out}"
    );
}
