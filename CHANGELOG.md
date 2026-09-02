# Changelog

All notable changes to morphe are recorded here.

The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and
morphe adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html): the
house style and the command-line surface are the public contract, and a change
that breaks either bumps the major.

## [Unreleased]

## [1.0.1] - 2026-09-02

No user-facing changes: the library and the CLI behave exactly as in 1.0.0.

### Internal

- Test names now hold to a single proposition each — three that welded two
  propositions are split into separate tests, and two that appended a rationale
  carry it in a comment instead. Test names and structure only.

## [1.0.0] - 2026-08-25

The first release. The version is a promise: the house style and the
command-line surface are stable, and anything that breaks either bumps the major.

### Added

- **A certificate refused on violation.** Before writing a byte, morphe re-parses
  its own candidate and proves it stands in the right relation to the input — by
  default token-stream equivalence: the same statements and comments, in the same
  order, differing only in whitespace (and, unless `--preserve-spellings`, synonym
  spellings). If the proof fails it writes nothing and exits `70`.
- **`--reorder`, proving a permutation.** Reorganizes top-level statements into a
  canonical signature order (`#defined`, `#const`, rules, `#show`), holding
  order-sensitive statements and `#include` in place as barriers, and proves the
  result a pure permutation of the input. The one meaning-affecting toggle,
  `[unsafe] reorder_includes`, hoists an `#include` on your assertion that it is
  pure — morphe cannot verify that, and records a note rather than moving silently.
- **One fixed style, a single knob.** The only dial is `--line-width` (default
  100); `--dialect` selects `clingo` or `asp-core-2`. Comments are carried to the
  anchors the parser gives them, never transposed.
- **Opt-in embedded-script formatting.** `#script` bodies stay verbatim unless you
  enable it; Python (via ruff) and Lua (via StyLua) reformatting is behind
  build-time features, off by default, each behind a hardened boundary carrying
  its own tool's guarantee rather than morphe's certificate.
- **A command line for CI and editors.** Formats files, directories, and stdin;
  in-place writes are atomic, so an interrupted run never truncates a source.
  `--check` and `--diff` write nothing and exit `1` on a change. A directory
  honours `.gitignore` and `--exclude`; configuration is a `morphe.toml`
  discovered by walking up, where an unknown key is a usage error, not a silent
  drop.

morphe formats; it does not rewrite — no semantic transformation, evaluation,
grounding, or solving. The certificate covers the ASP program; the embedded seam
carries its own posture in `docs/security/threat-model.md`. Not yet on crates.io
(it depends on `themelios-syntax` by path): build from source with the two
repositories cloned as siblings. Rust 1.97+ (edition 2024).

[Unreleased]: https://github.com/GregoryGelfond/morphe/compare/v1.0.1...HEAD
[1.0.1]: https://github.com/GregoryGelfond/morphe/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/GregoryGelfond/morphe/releases/tag/v1.0.0
