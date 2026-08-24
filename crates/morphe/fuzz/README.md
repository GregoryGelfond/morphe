# morphe fuzzing (docs/design/morphe.md §13)

The `format` target runs `morphe::format` over arbitrary bytes under both
dialects and asserts the shipped laws: morphe never panics (§12), a member's
output re-parses idempotently (§5.4), and every granted certificate is
`LayoutOnly` or `UpToSpelling` (§13). A non-member is refused (§5.7); a
`CertificateBroke` is a hard failure, so the shipped target enforces the grant
rather than tolerating its breach (§13).

Out of band, on stable (no nightly needed with `-s none`):

    cargo fuzz build --fuzz-dir crates/morphe/fuzz -s none format
    cargo fuzz run   --fuzz-dir crates/morphe/fuzz -s none format -- -max_total_time=60

The **seed corpus is the vendored `crates/morphe/tests/corpus/`** (real and
adversarial programs — better starting inputs than random blobs); pass it to
seed a run:

    cargo fuzz run --fuzz-dir crates/morphe/fuzz -s none format crates/morphe/tests/corpus

The working corpus (`fuzz/corpus/`), crash artifacts, and coverage are
git-ignored; only the crate and its target are tracked.
