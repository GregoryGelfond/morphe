# morphe

[![Gate CI status](https://img.shields.io/github/actions/workflow/status/GregoryGelfond/morphe/gate.yml?branch=main&style=flat-square&label=gate)](https://github.com/GregoryGelfond/morphe/actions/workflows/gate.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue?style=flat-square)](LICENSE)
![Rust 1.97+](https://img.shields.io/badge/rust-1.97%2B-orange?style=flat-square)

μορφή, *form* — an opinionated formatter for Answer Set Programming.

morphe gives your clingo and ASP-Core-2 programs one clean, consistent look — the
way `rustfmt`, `black`, and `ruff` do for their languages. The house style is
fixed, so the only thing to set is the line width. And morphe never changes what
your program means: before it saves a file, it reads its own output back and
checks that it says exactly what you wrote. If it can't be sure, it leaves the
file untouched.

**Highlights**

- **Safe by design.** morphe checks its own output before writing. If the result
  isn't your program — same rules, same facts, same comments — it refuses rather
  than guess.
- **One style, one setting.** A single fixed house style; the only knob is the
  line width (default 100).
- **clingo and ASP-Core-2.** Formats the clingo/clingcon dialect and the
  ASP-Core-2 standard.
- **Your comments, kept.** Every comment stays exactly as you wrote it, next to
  the code it belongs to.
- **Reorders only if you ask.** An opt-in pass tidies the order of your
  statements — and proves it only moved them.
- **Made for editors and CI.** Format on save, or check formatting in CI with
  `--check` and `--diff`. Writes are atomic, so an interrupted run never leaves a
  half-written file.
- **Embedded scripts too.** Optionally formats the Python and Lua inside
  `#script` blocks, using [ruff] and [StyLua].

## A first example

Give morphe a program with run-together statements and loose spacing:

```clingo
%reachability
edge(1,2). edge(2,3).edge(3,4).
reach(X,Y):-edge(X,Y).
reach(X,Z):-reach(X,Y),edge(Y,Z).  %transitive
#show reach/2.
```

```console
$ morphe reach.lp
```

```clingo
%reachability
edge(1, 2).
edge(2, 3).
edge(3, 4).
reach(X, Y) :- edge(X, Y).
reach(X, Z) :- reach(X, Y), edge(Y, Z). %transitive
#show reach/2.
```

One statement per line, the neck `:-` and the commas spaced, and your comments
left exactly where they were — morphe doesn't even add a space after `%`. The
same program, easier to read, and morphe checked that before it wrote the file.

## Choice rules and aggregates

Choice rules, aggregates, and conditions — the constructs real encodings are
built from — get a consistent shape. However you type this assignment rule,
morphe gives it one form:

```clingo
1{assigned(A,T):agent(A),compat(A,T)}1:-task(T).
#minimize{C,A,T:assigned(A,T),cost(A,T,C)}.
```

```console
$ morphe assign.lp
```

```clingo
1 { assigned(A,T) : agent(A), compat(A,T) } 1 :- task(T).
#minimize{ C, A, T : assigned(A,T), cost(A,T,C) }.
```

Bounds and braces are spaced, a condition's `:` is spaced, and an aggregate
keyword like `#minimize` hugs its brace.

## Constraints, too

morphe formats clingcon's constraint atoms as well, with the same fixed style.
Here is an N-queens model built from them — one queen per row, and none sharing a
column or a diagonal:

```clingo
#const n=8.
row(1..n).
% q(R): the column of the queen in row R.
&dom{1..n}=q(R):-row(R).
&distinct{q(R):row(R)}.
&distinct{q(R)-R:row(R)}.
&distinct{q(R)+R:row(R)}.
```

```console
$ morphe queens.lp
```

```clingo
#const n = 8.
row(1..n).
% q(R): the column of the queen in row R.
&dom{ 1 .. n } = q(R) :- row(R).
&distinct{ q(R) : row(R) }.
&distinct{ q(R) - R : row(R) }.
&distinct{ q(R) + R : row(R) }.
```

## Wrapping

A statement that fits stays on one line:

```console
$ morphe game.lp
```

```clingo
win(X) ; lose(X) :- player(X), move(X, Y), not win(Y), reachable(X).
```

When a rule is wider than the line width, morphe breaks its body one element per
line, so the structure stays legible instead of running off the screen:

```console
$ morphe --line-width 40 game.lp
```

```clingo
win(X) ; lose(X) :-
    player(X),
    move(X, Y),
    not win(Y),
    reachable(X).
```

## Reordering, if you ask for it

By default morphe leaves your statements exactly where they are. Add `--reorder`
and it also tidies their order, grouping them into a canonical shape —
`#defined`, then `#const`, then rules, then `#show` — and proving, as always,
that it only moved them:

```clingo
#show reach/1.
reach(Y) :- reach(X), edge(X, Y).
#defined edge/2.
reach(X) :- start(X).
#const max = 3.
```

```console
$ morphe --reorder program.lp
```

```clingo
#defined edge/2.
#const max = 3.
reach(Y) :- reach(X), edge(X, Y).
reach(X) :- start(X).
#show reach/1.
```

Statements whose order can matter, and every `#include`, stay put and act as
fences that reordering won't cross.

## Installation

morphe isn't on crates.io yet — it tracks `themelios-syntax` by a local path — so
build it from source. Clone the two repositories **side by side** (morphe looks
for the tier at `../themelios`):

```sh
git clone https://github.com/GregoryGelfond/themelios.git
git clone https://github.com/GregoryGelfond/morphe.git
cd morphe
cargo build --release          # the morphe binary at target/release/morphe
```

Or put it on your `PATH`:

```sh
cargo install --path crates/morphe-cli
```

To reformat the Python and Lua inside `#script` blocks, turn on the embedded
backends at build time (they're off by default, so the lean build compiles
neither):

```sh
cargo build --release --features embedded-python,embedded-lua
```

morphe builds on Rust 1.97 or newer.

## Usage

Format files and directories **in place** — recursive, `*.lp` by default,
honoring `.gitignore`. Writes are atomic, so an interrupted run never truncates a
source:

```sh
morphe program.lp
morphe src/
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

The options in full (`morphe --help` lists them all):

| Option | Effect |
| --- | --- |
| `--check` | Exit non-zero if the input would change; write nothing. |
| `--diff` | Print a unified diff; write nothing. |
| `--line-width <n>` | The one style knob (default 100). |
| `--dialect <clingo\|asp-core-2>` | The ASP dialect (default clingo). |
| `--reorder` | Apply the opt-in reorder. |
| `--format-embedded` | Reformat `#script` bodies with their language's tool. |
| `--exclude <glob>` | Skip matching files while walking a directory. |
| `--color <auto\|always\|never>` | Terminal color (default auto; honors `NO_COLOR`). |
| `-q`/`--quiet`, `-v`/`--verbose` | Less, or more, per-file reporting. |

## Configuration

Drop a `morphe.toml` beside your code and morphe finds it by walking up from each
file (or point at one with `--config`, or ignore them all with `--isolated`). It
sets the defaults; a command-line flag then overrides them. An unknown key is an
error, not a silent typo:

```toml
line_width = 100
dialect = "clingo"             # or "asp-core-2"
preserve_spellings = false
format_embedded = false
reorder = false

# The one setting that can affect meaning lives on its own: hoisting an
# #include past other statements rests on your word that the include is
# pure — morphe can't check that for you. See docs/security/threat-model.md.
[unsafe]
reorder_includes = false
```

## Exit codes

morphe follows the formatter convention, so `--check` and `--diff` slot straight
into CI:

| Code | Meaning |
| --- | --- |
| `0` | Clean — already formatted, or formatted in place. |
| `1` | Changes needed (under `--check`/`--diff`). |
| `2` | Usage error — bad arguments, or a malformed `morphe.toml`. |
| `70` | Internal error — a bug in morphe (it never writes on this path). |

## How the safety check works

morphe's promise is checked, not asserted. Each time it formats a program it
re-reads its own output and confirms two things: the statements and comments are
the ones you wrote, and — unless you asked for `--reorder` — they're still in the
same order. Only the spacing may differ (and, unless you keep them with
`--preserve-spellings`, the spelling of interchangeable operators). If that check
ever fails, morphe treats it as a bug in itself: it reports an internal error and
writes nothing. Your original file is never at risk.

## What morphe changes, and what it doesn't

morphe changes **layout** always, and **operator spelling** unless you pass
`--preserve-spellings`. It changes **statement order** only under the opt-in
`--reorder`. It never does anything else: no rewriting your encoding, no
evaluation, no grounding, no solving — those belong to other tools. Comments ride
with the code they annotate, never moved; documentation and `#script` bodies stay
verbatim unless you opt into embedded formatting.

The safety check covers your **ASP program**. A reformatted `#script` interior
instead carries its own tool's guarantee ([ruff], [StyLua]), kept behind a
hardened boundary — see
[`docs/security/threat-model.md`](docs/security/threat-model.md).

## Using morphe as a library

morphe is a library first; the `morphe` binary is a thin shell over it.
`morphe::format` takes a parsed source and returns the formatted text along with
the proof it checked, or a typed refusal — the core is pure and does no I/O.
Since the crate isn't published yet, the API is documented in the design of
record, [`docs/design/morphe.md`](docs/design/morphe.md).

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

The same checks run in CI (`.github/workflows/gate.yml`). The clingo differential
and the criterion benches run out of band — they need clingo and the `pixi`
environment.

## Status

**Built and released.** morphe is a working library and the `morphe` binary: the
fixed house style, the always-on safety check, comment placement that preserves
attachment, the embedded `#script` seam, and the opt-in reorder. The design of
record is [`docs/design/morphe.md`](docs/design/morphe.md); the security posture
is [`docs/security/threat-model.md`](docs/security/threat-model.md). It is held to
a high engineering bar: a standing gate of formatting, lints, and property tests,
plus a fuzz target and a differential against clingo itself.

## License

MIT © 2026 Gregory Gelfond. See [LICENSE](LICENSE).

[ruff]: https://github.com/astral-sh/ruff
[StyLua]: https://github.com/JohnnyMorganz/StyLua
