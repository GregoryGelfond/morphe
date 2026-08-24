//! The clingo differential (docs/design/morphe.md §13), out of band and
//! feature-gated: run through `pixi run differential`, which supplies the pinned
//! authority (clingo v5.8.2, docs/grammar.md §3). It is a cross-check whose
//! signal is *agreement*, a meaning-preservation witness beyond the token
//! certificates — never the shipped guarantee.
//!
//! **The format leg.** Over every corpus member, the authority's own canonical
//! rendering of the input and of `format`'s output must be identical: layout and
//! synonym spelling cannot change what clingo parses, so the text clingo prints
//! from its AST is invariant under a layout-only/up-to-spelling reformat. This
//! is the strongest external witness that `format` preserved meaning. Comments
//! are the tier's trivia — carried by morphe under §8 and checked by the
//! comment-fact property — and are dropped from this comparison.
//!
//! **The reorder leg.** Over every groundable member, `reorder`'s output grounds
//! to the same ground-rule *set* as the input (`clingo --text`, canonicalized).
//! This is a **proxy** for answer-set preservation under a pure permutation, and
//! only a proxy: structural ground-set equality is never presented as answer-set
//! equivalence (spec §7.1). It is a **backstop** for the permutation certificate
//! (§15.3) and for same-bucket stability (§15.4) — not the shipped guarantee.
//!
//! **What it cannot do.** It cannot backstop `reorder_includes`: grounding a
//! program with `#include` needs the included files resolved, which a
//! self-contained corpus does not carry, so an include-bearing member is skipped
//! from the reorder leg (§15.4). And it proves agreement only on the corpus and
//! only at the pin — nothing beyond them. Every unrecorded disagreement fails;
//! a recorded one carries the reading that settles it (`AUTHORITY-DISAGREEMENTS`).
#![cfg(feature = "differential")]

use std::collections::BTreeSet;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::{env, fs};

use serde_json::Value;

use morphe::{Dialect, format, reorder};
use morphe_test_support::{cases, options, source};

/// The pinned authority (docs/design/morphe.md §13; docs/grammar.md §3).
const AUTHORITY_VERSION: &str = "5.8.2";

fn manifest_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn corpus_dir() -> PathBuf {
    manifest_dir().join("tests/corpus")
}

fn authority_helper() -> PathBuf {
    manifest_dir().join("tests/differential/authority.py")
}

/// A corpus input's path in register/report form: relative to the corpus root.
/// Both sides are canonicalized because the shared corpus helper reaches the
/// inputs through a sibling crate's `..`.
fn relative(path: &Path) -> String {
    let base = corpus_dir()
        .canonicalize()
        .expect("the corpus dir resolves");
    path.canonicalize()
        .expect("a corpus input resolves")
        .strip_prefix(&base)
        .expect("a corpus input is under the corpus")
        .display()
        .to_string()
}

/// A register file: `#` header lines and blank lines dropped, each remaining
/// line's first field a corpus-relative path. An absent register is empty, not a
/// failure (docs/design/morphe.md §13).
fn register(name: &str) -> BTreeSet<String> {
    fs::read_to_string(corpus_dir().join(name))
        .unwrap_or_default()
        .lines()
        .filter(|line| {
            let line = line.trim_start();
            !line.is_empty() && !line.starts_with('#')
        })
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .collect()
}

// ---- the format leg: the authority's own parse and printing -----------------

/// The authority's reading of one program, from `authority.py`.
struct Reading {
    accepted: bool,
    include_failed: bool,
    /// Each statement's AST type and the authority's canonical printing of it.
    statements: Vec<(String, String)>,
}

/// Spawn the helper on `program` from `cwd` and wait for its output. A closed
/// stdin means the helper exited before reading it — clingo not importable, say
/// — so the write is not asserted: the failure is read from the exit status and
/// stderr, not from a `BrokenPipe` panic here.
fn run_helper(program: &str, cwd: &Path) -> Output {
    let mut child = Command::new("python")
        .arg(authority_helper())
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("python runs: run this harness through `pixi run differential`");
    let _ = child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(program.as_bytes());
    child.wait_with_output().expect("the authority answers")
}

/// The authority's reading, with the pin asserted on every call — so a version
/// drift is a loud failure, not a silent change of what the differential proves
/// (executable-claims, docs/design/morphe.md §13; spec §10.4).
fn authority(program: &str, cwd: &Path) -> Reading {
    let output = run_helper(program, cwd);
    assert!(
        output.status.success(),
        "the authority helper failed (is clingo's Python module present? run through pixi):\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("the helper emits JSON");
    assert_eq!(
        value["version"].as_str(),
        Some(AUTHORITY_VERSION),
        "docs/design/morphe.md §13: the authority is pinned at v{AUTHORITY_VERSION}"
    );
    Reading {
        accepted: value["accepted"].as_bool().unwrap_or(false),
        include_failed: value["include_failed"].as_bool().unwrap_or(false),
        statements: value["statements"]
            .as_array()
            .map(|statements| {
                statements
                    .iter()
                    .map(|s| {
                        (
                            s["type"].as_str().unwrap_or_default().to_owned(),
                            s["text"].as_str().unwrap_or_default().to_owned(),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default(),
    }
}

/// The authority's canonical rendering of a parse, comments dropped — the
/// layout- and spelling-invariant form the format leg compares.
fn canonical_render(reading: &Reading) -> Vec<String> {
    reading
        .statements
        .iter()
        .filter(|(kind, _)| kind != "Comment")
        .map(|(_, text)| text.clone())
        .collect()
}

// ---- the reorder leg: the ground-rule set under a permutation ---------------

/// Ground `program` from `cwd` with the pinned authority (`clingo --text`) as a
/// set of ground lines — or `None` when the authority cannot ground it: an
/// unresolvable `#include` (the §15.4 exclusion — a self-contained corpus does
/// not carry the included files) or an ungroundable program (an unsafe variable,
/// say). A missing include exits zero but logs an error, so failure is read from
/// a non-zero exit *or* an `error:` on stderr, not the exit alone.
fn ground(program: &str, cwd: &Path) -> Option<BTreeSet<String>> {
    let mut child = Command::new("clingo")
        .args(["--text", "-"])
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("clingo runs: run this harness through `pixi run differential`");
    let _ = child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(program.as_bytes());
    let output = child.wait_with_output().expect("the authority answers");
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !output.status.success() || stderr.contains("error:") || stderr.contains("ERROR:") {
        return None;
    }
    Some(ground_lines(&String::from_utf8_lossy(&output.stdout)))
}

/// The ground-rule set of a `clingo --text` output: non-blank lines, trimmed, as
/// a set — the permutation must preserve the set, not the order (§15.4).
fn ground_lines(text: &str) -> BTreeSet<String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(str::to_owned)
        .collect()
}

/// The clingo binary's version string (`clingo --version`), for the reorder
/// leg's grounder — the format leg's module version is asserted on every
/// `authority` call.
fn clingo_binary_version() -> String {
    let output = Command::new("clingo")
        .arg("--version")
        .output()
        .expect("clingo runs: run this harness through `pixi run differential`");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or_default()
        .to_owned()
}

// ---- the differential ------------------------------------------------------

#[test]
fn the_pinned_authority_is_the_version_of_record() {
    // Executable-claims (docs/design/morphe.md §13; spec §10.4): both faces of
    // the authority — the clingo binary (the reorder leg's grounder) and its
    // Python module (the format leg's parser/printer) — are the pinned v5.8.2.
    assert!(
        clingo_binary_version().contains(AUTHORITY_VERSION),
        "the clingo binary is not v{AUTHORITY_VERSION}: {}",
        clingo_binary_version()
    );
    assert!(
        authority("a.\n", &corpus_dir()).accepted,
        "the authority reads a trivial member (its module version is asserted within)"
    );
}

#[test]
fn the_format_leg_preserves_the_authoritys_canonical_rendering() {
    let skip = register("DIFFERENTIAL-SKIP");
    let known = register("AUTHORITY-DISAGREEMENTS");
    let mut disagreements = Vec::new();
    let mut compared = 0usize;
    for case in cases().into_iter().filter(|case| case.is_member) {
        let name = relative(&case.path);
        if skip.contains(&name) {
            continue;
        }
        let cwd = case.path.parent().expect("a corpus input has a directory");
        let formatted = format(&source(&case.text), &options(case.dialect))
            .unwrap_or_else(|err| panic!("{name}: a member must format: {err:?}"));
        let input = authority(&case.text, cwd);
        // The authority cannot resolve the includes a self-contained corpus does
        // not carry (§15.4); such a member is skipped from both legs.
        if input.include_failed {
            continue;
        }
        if !input.accepted {
            // The tier admits it; clingo does not — a membership divergence to
            // record with its reading, not a format defect.
            if !known.contains(&name) {
                disagreements.push(format!(
                    "{name}: membership — morphe admits, the authority rejects"
                ));
            }
            continue;
        }
        let output = authority(&formatted.text, cwd);
        compared += 1;
        let before = canonical_render(&input);
        let after = canonical_render(&output);
        if before != after && !known.contains(&name) {
            disagreements.push(format!(
                "{name}: format changed the authority's parse\n  input : {before:?}\n  output: {after:?}"
            ));
        }
    }
    println!("format leg: compared {compared} members against the authority");
    assert!(compared > 0, "no member to compare in the format leg");
    assert!(
        disagreements.is_empty(),
        "unrecorded format-leg disagreements (each a defect in morphe or a divergence for docs/grammar.md §11):\n{}",
        disagreements.join("\n")
    );
}

#[test]
fn the_reorder_leg_preserves_the_ground_rule_set() {
    let skip = register("DIFFERENTIAL-SKIP");
    let known = register("AUTHORITY-DISAGREEMENTS");
    let mut disagreements = Vec::new();
    let mut compared = 0usize;
    let mut moved = 0usize;
    for case in cases().into_iter().filter(|case| case.is_member) {
        let name = relative(&case.path);
        if skip.contains(&name) {
            continue;
        }
        let cwd = case.path.parent().expect("a corpus input has a directory");
        let reordered = reorder(&source(&case.text), &options(case.dialect))
            .unwrap_or_else(|err| panic!("{name}: a member must reorder: {err:?}"));
        match (ground(&case.text, cwd), ground(&reordered.text, cwd)) {
            // Not groundable as a self-contained program — the §15.4 exclusion
            // (an unresolvable #include, so reorder_includes is never backstopped
            // here) or an ungroundable program. Skipped from the reorder leg.
            (None, None) => {}
            (Some(before), Some(after)) => {
                compared += 1;
                if reordered.reordered {
                    moved += 1;
                }
                if before != after && !known.contains(&name) {
                    disagreements.push(format!(
                        "{name}: reorder changed the ground-rule set\n  only in input : {:?}\n  only in output: {:?}",
                        before.difference(&after).collect::<Vec<_>>(),
                        after.difference(&before).collect::<Vec<_>>()
                    ));
                }
            }
            // Grounding one side but not the other means reorder changed whether
            // the program grounds — a defect, always raised.
            (Some(_), None) | (None, Some(_)) => disagreements.push(format!(
                "{name}: reorder changed whether the authority can ground the program"
            )),
        }
    }
    println!("reorder leg: compared {compared} groundable members ({moved} actually reordered)");
    assert!(
        compared > 0,
        "no groundable member to compare in the reorder leg"
    );
    assert!(
        disagreements.is_empty(),
        "unrecorded reorder-leg disagreements (§13's ground-set proxy; each a defect in morphe or a divergence to record):\n{}",
        disagreements.join("\n")
    );
}

#[test]
fn the_reorder_leg_holds_under_an_actual_permutation() {
    // The corpus's members are already in canonical order, so its reorder leg
    // moves nothing; these seeds — a signature directive out of order, which
    // morphe hoists to its bucket — exercise the permutation the ground-set
    // proxy backstops (§13, §15.4). Each grounds to the same set after the move.
    let cwd = corpus_dir();
    for program in [
        "#show p/1.\np(1).\np(2).\n",   // #show hoisted to the end
        "a.\nb :- a.\n#defined a/0.\n", // #defined hoisted to the front
        "p(1..n).\n#const n = 3.\n",    // #const hoisted before the body
    ] {
        let reordered = reorder(&source(program), &options(Dialect::Clingo))
            .unwrap_or_else(|err| panic!("{program:?}: a member must reorder: {err:?}"));
        assert!(
            reordered.reordered,
            "{program:?}: the seed must actually reorder to have teeth"
        );
        let before = ground(program, &cwd).expect("the input grounds");
        let after = ground(&reordered.text, &cwd).expect("the reordered program grounds");
        assert_eq!(
            before,
            after,
            "{program:?}: reorder changed the ground-rule set\n  only in input : {:?}\n  only in output: {:?}",
            before.difference(&after).collect::<Vec<_>>(),
            after.difference(&before).collect::<Vec<_>>()
        );
    }
}

// ---- the harness's own logic, held without the authority -------------------

#[test]
fn register_reads_paths_and_drops_the_header() {
    let skip = register("DIFFERENTIAL-SKIP");
    assert!(
        !skip.iter().any(|entry| entry.starts_with('#')),
        "the header is dropped"
    );
    assert!(
        register("NO-SUCH-REGISTER").is_empty(),
        "an absent register is empty"
    );
}

#[test]
fn canonical_render_drops_comments() {
    let reading = Reading {
        accepted: true,
        include_failed: false,
        statements: vec![
            ("Program".to_owned(), "#program base.".to_owned()),
            ("Comment".to_owned(), "% a comment".to_owned()),
            ("Rule".to_owned(), "a.".to_owned()),
        ],
    };
    assert_eq!(
        canonical_render(&reading),
        vec!["#program base.".to_owned(), "a.".to_owned()]
    );
}

#[test]
fn ground_lines_is_a_set_of_non_blank_lines() {
    let set = ground_lines("a.\n\n  b.  \na.\n");
    assert_eq!(set.len(), 2, "blank lines dropped, duplicates folded");
    assert!(set.contains("a.") && set.contains("b."));
}
