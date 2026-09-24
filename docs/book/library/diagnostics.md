# Diagnostics and exit codes

morphe returns typed refusals, never a panic or a bare string.

## `FormatError`

```rust
pub enum FormatError {
    HasParseErrors(Vec<SyntaxError>),
    CertificateBroke { certificate: Certificate, reason: CertificateBreak },
}
```

The two mean very different things. `HasParseErrors` is *your* input: a non-member,
which morphe refuses whole (see [safety](../guide/safety.md)), handing back the
parser's own diagnostics for you to render. `CertificateBroke` is *morphe's* bug —
it means morphe produced output it could not certify, which must never happen; the
`reason` locates it:

```rust
pub enum CertificateBreak {
    TokenMismatch(Mismatch),      // the output re-parsed, but a token diverged
    NotAMember(Vec<SyntaxError>), // the output did not re-parse as a member
}
```

`reorder` mirrors this with `ReorderError::{HasParseErrors, PermutationBroke}`; a
`PermutationBroke` (a per-unit mismatch, or a bijection breach recorded as a
`BijectionBreach`) is the reorder's equivalent of `CertificateBroke` — always a
morphe bug. Warnings that do *not* refuse — an embedded-script skip, the
`reorder_includes` note — ride in the `notes` field of the result, never as an
error.

## Exit codes

The `morphe` command maps outcomes to four exit codes, so a script can branch on
them:

| Code | Meaning |
|---|---|
| `0` | Clean — already formatted, or formatted in place |
| `1` | Changes needed (under `--check` / `--diff`) |
| `2` | Usage error — bad arguments or a malformed `morphe.toml`; also a parse refusal |
| `70` | Internal error — a morphe bug (a broken certificate); morphe never writes on this path |

A parse refusal (`HasParseErrors`) exits `2`; a broken certificate or permutation
(`CertificateBroke`, `PermutationBroke`) exits `70`. Across a directory run the
worst outcome wins, so a single internal error surfaces as `70`.
