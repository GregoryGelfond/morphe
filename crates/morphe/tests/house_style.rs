//! The house-style golden harness (docs/design/morphe.md §7, §13): each golden
//! is an input `<name>.in.lp` and its expected output `<name>.out.lp` under a
//! category directory below `tests/house_style/`. The harness formats each
//! input (clingo, default options) and compares; a mismatch prints the expected
//! and actual output for review, and every golden is read and accepted at the
//! build close (§13).

use std::path::{Path, PathBuf};

use morphe::{FormatOptions, format};
use morphe_test_support::source;

fn goldens_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/house_style")
}

fn inputs(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut paths: Vec<PathBuf> = entries
        .map(|entry| entry.expect("a dir entry").path())
        .collect();
    paths.sort();
    for path in paths {
        if path.is_dir() {
            inputs(&path, out);
        } else if path.to_string_lossy().ends_with(".in.lp") {
            out.push(path);
        }
    }
}

#[test]
fn every_golden_formats_to_its_expected_output() {
    let mut input_paths = Vec::new();
    inputs(&goldens_dir(), &mut input_paths);
    assert!(!input_paths.is_empty(), "no house-style goldens found");

    let mut failures = Vec::new();
    for input_path in &input_paths {
        let expected_path: PathBuf = input_path
            .to_string_lossy()
            .replace(".in.lp", ".out.lp")
            .into();
        let input = std::fs::read_to_string(input_path).expect("read a golden input");
        let expected = std::fs::read_to_string(&expected_path)
            .unwrap_or_else(|_| panic!("missing expected output {}", expected_path.display()));
        let formatted = format(&source(&input), &FormatOptions::default())
            .unwrap_or_else(|err| panic!("{}: {err:?}", input_path.display()));
        if formatted.text != expected {
            failures.push(format!(
                "--- {} ---\n== expected ==\n{expected}\n== actual ==\n{}",
                input_path.display(),
                formatted.text
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "golden mismatches:\n\n{}",
        failures.join("\n\n")
    );
}
