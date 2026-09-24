# Working on morphe

Read [`CONTRIBUTING.md`](CONTRIBUTING.md) before making changes. It is the authoritative contributor guide; this file is a concise entry point for coding agents. Apply the same standards to implementation, tests, and documentation.

## Find the relevant boundary

Repository knowledge is in the manual and the design of record, not a directory tour:

- The formatter, end to end: the book's guided tour ([`docs/book/guide/tour.md`](docs/book/guide/tour.md)) and the safety spine ([`docs/book/guide/safety.md`](docs/book/guide/safety.md)).
- The house style, comment placement, the reorder, and embedded scripts: the book's Part I, and the design of record [`docs/design/morphe.md`](docs/design/morphe.md) (§7, §8, §9, §15).
- The library surface — `format`, `format_parsed`, `reorder`, `FormatOptions`, the certificate: the book's Part II and `docs/design/morphe.md` §4.2.
- The safety argument — the always-on certificate, idempotence, depth safety: `docs/design/morphe.md` §5.
- The input surfaces and what is defended: [`docs/security/threat-model.md`](docs/security/threat-model.md).

## Preserve the safety spine

- **morphe never returns output it did not certify.** `format` re-parses its own result and proves it `LayoutOnly` or `UpToSpelling` against the input before returning it; a broken certificate is a morphe **bug**, surfaced loudly, never a silent write.
- **morphe formats only** — it never evaluates, grounds, or solves, and it never runs an embedded script. The reorder is a *separate* operation with its own permutation certificate; `format` never changes statement order.
- **`#include` is a reorder barrier** by default; hoisting it is the one caller-asserted, `[unsafe]` transform, and it is never silent.
- **Emitted and generated text is built as syntax values and rendered**, never string-templated; the whitespace floor is the tier's fusion oracle, and morphe keeps no spacing table of its own.
- The themelios dependency is arm's-length: **never modify it**; close a gap in themelios and adopt it by a deliberate, reviewed pin bump.

## Make the correctness argument readable

- Library first, the CLI thin: a capability lives in `morphe` and the `morphe` command is a view over it.
- Every foreign input returns a typed diagnostic or refusal, never a panic or a bare string. One concept, one name (the book's [Vocabulary](docs/book/vocabulary.md) is the registry).
- Resolve every rustfmt, pedantic Clippy, and rustdoc diagnostic in the code. The workspace denies `unsafe_code`, `missing_docs`, `unused`, `dead_code`, and pedantic Clippy; never suppress one with `allow` or `expect`, except a narrow, reasoned `expect` for a foreign-API false positive.
- The build profiles pin `panic = "unwind"` — the embedded backends' `catch_unwind` containment depends on it; do not change it.
- One test name states one proposition. Every book example is produced by running the binary.

## Validate the affected contract

Run the gate through the single entry point before pushing. Hosted CI is paused (the Actions quota; it resumes at the October refresh), so the local gate is the gate:

| mode | scope |
|---|---|
| `scripts/check.sh portable` | fmt, clippy (`-D warnings`, including the embedded backends), test, doc (`-D warnings`) |
| `scripts/check.sh coverage` | line coverage, floor 90 |
| `scripts/check.sh differential` | the out-of-band clingo differential (via pixi) |
| `scripts/check.sh book` | build the mdBook manual |
| `scripts/check.sh full` | all of the above |

Report the checks you actually ran. Never weaken a gate to accommodate a change.

## Keep public documentation current

Update the manual and its worked examples with the implementation they describe, and re-run each example through the binary so the book cannot drift from the tool. Keep development diaries, session notes, and machine-local paths out of the tree — documentation must be intelligible without them.
