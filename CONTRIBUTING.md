# Contributing to morphe

Start with the worked examples in the book's [morphe by example](docs/book/by-example/format.md) and its [guided tour](docs/book/guide/tour.md), then the design of record, [`docs/design/morphe.md`](docs/design/morphe.md). The manual in [`docs/book/`](docs/book/) is the reader's path; this file is the standard the work is held to.

morphe is an opinionated formatter for Answer Set Programming — the clingo/clingcon dialect and the ASP-Core-2 standard — and only that: it lays a program out in one fixed house style and proves it changed only the layout. It never evaluates, grounds, or solves. It is a library first; the `morphe` command is a thin shell over `morphe`. The MIT license names Gregory Gelfond.

## Design from the formatter's questions

The house style is **fixed** — `line_width` is the one layout knob, and indent and the rest are settled (a deliberate stance, not a gap to fill). Public operations speak in the domain's terms — programs, statements, the certificate, the house style, units and barriers — not the incidental shapes of the crates beneath them.

The certificate **trusts nothing**: `format` re-parses its own output and proves the interleaved token-and-comment sequence is the input's, differing only in whitespace (and, off `preserve_spellings`, canonical operator spellings). It is checked, not asserted, and always on — never a mode. A guarantee morphe cannot check it does not make: the one transform resting on the caller's word rather than a morphe proof, `reorder_includes`, is walled off in an `[unsafe]` config table and is never silent.

The themelios dependency is arm's-length: morphe consumes it, never modifies it, and pins it by git revision so the build is reproducible. A surfaced gap is closed in themelios and adopted by a deliberate pin bump, never worked around by editing it. When themelios publishes to crates.io, the pin becomes a semver dependency — a one-line change.

## Make represented knowledge inspectable

A distinction in a type must have a producer and a consumer; a variant nothing constructs, or nothing reads, is a false claim about the design. Every foreign input crosses a `Result` boundary returning a typed diagnostic or refusal — never a panic, never a bare string, never a half-written file. One concept has one name: the two relations morphe proves are `LayoutOnly` and `UpToSpelling`; a statement the reorder moves is a *unit*; one it holds is a *barrier*; a boundary that admits external input is a *door*. The book's [Vocabulary](docs/book/vocabulary.md) is that registry — two words for one thing is the same defect as one word for two.

Documentation is held to the same standard as code: it must be intelligible without access to private working records, and it can never lie. Keep development notes, session logs, and machine-local paths out of the tree. Every book example is produced by running the `morphe` binary, so the manual cannot drift from the tool.

## Carry the guarantee through

The safety spine is the whole of morphe, and it is layered:

- **The ASP program** is certified `LayoutOnly`/`UpToSpelling` on every `format`, the certificate re-derived from morphe's own output (`docs/design/morphe.md` §5).
- **The reorder** earns a separate **permutation certificate** — a total bijection of units with every barrier held — and is a distinct operation, so `format`'s contract stays pure (§15).
- **The embedded surface** is contained, not trusted: the ASP skeleton is certified with every `#script` body verbatim; each interior is delegated to its backend's own guarantee; and an equal-except-script-bodies check binds the two. Any backend failure — a caught panic included — degrades to *verbatim + a note*, never worse (§9).

All emitted and generated text is built as syntax values and rendered; the whitespace floor is the tier's fusion oracle, and a change that would assemble output by string concatenation is wrong however plausible it looks.

## Verification and review

Every change is held to the gate, green, before it lands. The single entry point is `scripts/check.sh`:

| mode | scope |
|---|---|
| `portable` | `cargo fmt --all --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`, and again with `--features embedded-python,embedded-lua`; `cargo test --workspace --locked`, and the embedded features; `cargo doc` with `RUSTDOCFLAGS=-D warnings` |
| `coverage` | line coverage, floor **90** (`cargo llvm-cov --workspace --exclude morphe-fuzz --locked --fail-under-lines 90`) |
| `differential` | the out-of-band clingo differential (`pixi run differential`, clingo 5.8.2) |
| `book` | build the mdBook manual (mdBook 0.5.4) |
| `full` | all of the above |

The coverage floor follows themelios's method — measure line coverage, round down to a multiple of five, minus five — raised only deliberately and never lowered. The README's coverage badge reports region coverage, a higher figure than the line coverage the floor gates; both come from the same `cargo llvm-cov` run. Beyond it, adversariality is carried by the property laws (the always-on certificate, idempotence, comment-fact preservation, the permutation certificate), the fuzz target, and the clingo differential; coverage counts lines run, not behaviour pinned. The workspace denies `unsafe_code`, `missing_docs`, `unused`, `dead_code`, and pedantic Clippy; never suppress one with `allow` or `expect`, save a narrow `expect` where a discharged invariant makes a foreign-API lint a false positive, with the reason beside it. The build profiles pin `panic = "unwind"`, which the embedded `catch_unwind` containment requires. One test name states one proposition — fifty characters is a review cue, not a limit.

**Proposing a change.** Branch off `main`; `main` is protected and lands only by pull request, rebase-merged (`gh pr merge --rebase`) to keep history linear. Run `scripts/check.sh full` locally before you push — hosted CI is paused (the Actions quota is exhausted; it resumes at the October refresh, and `.github/workflows/gate.yml` is retained as documented intent, `workflow_dispatch`-only), so the local gate is the gate. Commit subjects are plain and imperative ("Add the ASP-Core-2 query layout", not "feat:"). Keep per-person tooling out of the tree.

## Repository presentation

The GitHub description, topics, badges, and release metadata state what is true and no more. Badges are static and honest: license, Rust version, a coverage figure measured locally by `scripts/check.sh coverage`, and a link to the book. There is **no CI badge** while the workflow is paused — a CI badge must identify an active workflow. A release tag corresponds to what was released.
