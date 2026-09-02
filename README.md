# morphe

[![Gate CI status](https://img.shields.io/github/actions/workflow/status/GregoryGelfond/morphe/gate.yml?branch=main&style=flat-square&label=gate)](https://github.com/GregoryGelfond/morphe/actions/workflows/gate.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)

μορφή, *form* — a formatter for Answer Set Programming.

morphe formats the clingo/clingcon dialect of ASP (and its ASP-Core-2 variant)
the way `rustfmt`, `black`, and `ruff` format their languages: one fixed house
style, a single knob (the line width), and a hard safety guarantee — **morphe
never writes output it cannot certify against its input.**

Before writing a byte, morphe re-parses its own candidate output and proves it
stands in the right relation to the input. By default that relation is
token-stream equivalence: the same statements and the same comments, in the
same order, differing only in whitespace (and, optionally, synonym spellings).
The opt-in `--reorder` proves a *permutation* instead — the same statements,
each still equivalent to its original, reorganized within regions that
order-sensitive statements and `#include` pin in place. Either way the
guarantee is *checked*, not asserted: if the proof fails, morphe refuses
rather than risk silently changing what a program computes.

It is built on
[`themelios-syntax`](https://github.com/GregoryGelfond/themelios), which owns the
lexer, the lossless syntax tree, the typed AST, comment attachment, and the
token-stream-equivalence certificate morphe's guarantee rests on.

## At a glance

morphe reflows to one house style and leaves comments and statement order
exactly where they are. Given this input:

```
#show reach/1.
reach(Y):-  reach(X),edge(X,Y).
#defined edge/2.
reach(X) :- start(X).
#const   max=3.
```

`morphe` normalizes layout only — the spacing, the `:-`, the commas — and holds
every statement in place:

```
#show reach/1.
reach(Y) :- reach(X), edge(X, Y).
#defined edge/2.
reach(X) :- start(X).
#const max = 3.
```

The opt-in `morphe --reorder` instead proves a permutation, moving whole
statements (with their comments) into canonical signature order — `#defined`,
`#const`, rules, `#show`:

```
#defined edge/2.
#const max = 3.
reach(Y) :- reach(X), edge(X, Y).
reach(X) :- start(X).
#show reach/1.
```

## Installation

morphe is not yet published to crates.io — it depends on `themelios-syntax` by a
local path — so build it from source. Clone the two repositories **as siblings**
(morphe resolves the tier at `../themelios`):

```sh
git clone https://github.com/GregoryGelfond/themelios.git
git clone https://github.com/GregoryGelfond/morphe.git
cd morphe
cargo build --release          # the `morphe` binary at target/release/morphe
```

Or install it onto your `PATH`:

```sh
cargo install --path crates/morphe-cli
```

Reformatting embedded `#script` bodies is opt-in at build time — Python through
[ruff], Lua through [StyLua] — behind Cargo features, off by default so the lean
build compiles neither:

```sh
cargo build --release --features embedded-python,embedded-lua
```

morphe builds on Rust 1.97 or newer (edition 2024).

## Usage

Format files and directories **in place** (recursive, `*.lp` by default; writes
are atomic, so an interrupted run never truncates a source):

```sh
morphe program.lp
morphe src/                    # every *.lp under src/, honouring .gitignore
```

Check or preview without writing — for CI and editors:

```sh
morphe --check program.lp      # exit 1 if it would change; writes nothing
morphe --diff program.lp       # print a unified diff; writes nothing
```

Format standard input to standard output (never in place):

```sh
cat program.lp | morphe -
```

Reorganize a program's top-level statements into a canonical signature order — an
**opt-in**, certified transform, off by default (it holds order-sensitive
statements and `#include` in place as barriers, and organizes within the regions
they divide):

```sh
morphe --reorder program.lp
```

Common options (`morphe --help` lists them all):

| Option | Effect |
| --- | --- |
| `--check` | Exit non-zero if the input would change; write nothing. |
| `--diff` | Print a unified diff; write nothing. |
| `--line-width <n>` | The one style knob (default 100). |
| `--dialect <clingo\|asp-core-2>` | The ASP dialect (default clingo). |
| `--reorder` | Apply the opt-in signature reorder. |
| `--format-embedded` | Reformat `#script` bodies with their language's tool. |
| `--exclude <glob>` | Skip matching files during a directory walk. |
| `--color <auto\|always\|never>` | Terminal colour (default auto; honours `NO_COLOR`). |
| `-q`/`--quiet`, `-v`/`--verbose` | Less, or more, per-file reporting. |

## Configuration

A `morphe.toml`, discovered by walking up from each formatted file (or named with
`--config`, or ignored with `--isolated`), sets the defaults a command-line flag
then overrides. Unknown keys are a usage error, not a silent drop:

```toml
line_width = 100
dialect = "clingo"             # or "asp-core-2"
preserve_spellings = false
format_embedded = false
reorder = false

# The one meaning-affecting toggle lives apart, in its own table: hoisting an
# `#include` past other statements rests on your assertion that the include is
# pure — morphe cannot verify that. See docs/security/threat-model.md.
[unsafe]
reorder_includes = false
```

## Exit codes

morphe follows the formatter convention, so `--check`/`--diff` compose with CI:

| Code | Meaning |
| --- | --- |
| `0` | Clean — already formatted, or formatted in place. |
| `1` | Changes needed (under `--check`/`--diff`). |
| `2` | Usage error — bad arguments, or a malformed `morphe.toml`. |
| `70` | Internal error — a bug in morphe (it never writes on this path). |

## What morphe does, and does not

morphe changes **layout** always, **synonym spelling** always (unless
`--preserve-spellings`), and top-level **order** only under the opt-in, certified
`--reorder`. It never otherwise rewrites a program: no styled or semantic
transformation, no evaluation, no grounding, no solving — those belong to other
tools. Comments are carried to the same anchors the parser gives them, never
transposed; documentation and `#script` bodies stay verbatim unless you opt into
embedded formatting.

The safety certificate covers the **ASP program**. A reformatted `#script`
interior instead carries its own tool's guarantee (ruff, StyLua), contained
behind a hardened boundary — see
[`docs/security/threat-model.md`](docs/security/threat-model.md).

## Project layout

- **`morphe`** — the library: the formatter as a pure, I/O-free API.
- **`morphe-cli`** — the thin binary (`morphe`): files, directories, stdin,
  `--check`, `--diff`, config, and the embedded backends.
- **`docs/design/morphe.md`** — the design of record.

## Development

```sh
cargo test --workspace                          # the suite is the executable spec
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```

The same checks run in CI (`.github/workflows/gate.yml`); the clingo differential
and the criterion benches run out of band (they need clingo and the `pixi`
environment).

## Status

**Built.** morphe is a working library and the `morphe` binary: the fixed house
style, the always-on safety certificate, comment placement that preserves
attachment, the embedded `#script` seam, and the opt-in signature reorder. The
design of record is `docs/design/morphe.md`; the security posture is
`docs/security/threat-model.md`. It is held to the estate's engineering standard:
a standing gate of formatting, lints, and property tests, plus a fuzz target and
a differential against clingo itself.

## License

MIT © 2026 Gregory Gelfond.

[ruff]: https://github.com/astral-sh/ruff
[StyLua]: https://github.com/JohnnyMorganz/StyLua
