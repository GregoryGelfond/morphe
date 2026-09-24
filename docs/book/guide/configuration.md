# Configuration

morphe's style is fixed on purpose: there is one layout knob — the line width — and
nothing else about how code lays out is configurable. What a `morphe.toml` sets is
that one knob, the dialect, and which sound transforms run.

## `morphe.toml`, and how it's found

Drop a `morphe.toml` beside your code. morphe discovers it by walking up from each
file it formats, so one file at a project's root configures the whole tree:

```toml
line_width = 100
dialect = "clingo"          # or "asp-core-2"
preserve_spellings = false
format_embedded = false
reorder = false

# The one setting that can affect meaning lives on its own (see below).
[unsafe]
reorder_includes = false
```

Those are all the keys. An **unknown key is a usage error**, not a silently
ignored typo:

```console
$ morphe program.lp
morphe: ./morphe.toml: unknown field `line_wdith`, expected one of `line_width`, `dialect`, `preserve_spellings`, `format_embedded`, `reorder`, `unsafe`
```

A command-line flag overrides the file. `--config <path>` reads a specific file
instead of discovering one; `--isolated` ignores any `morphe.toml` entirely and
uses the built-in defaults with only the flags you pass.

## Safe and unsafe

The keys divide into two kinds. The ordinary ones select a **morphe-proved**
transform — `reorder`, for instance, moves whole statements and proves it only
permuted them (see [Reordering](reorder.md)). One key is different, and it lives in
its own `[unsafe]` table to say so: **`reorder_includes`** lets the reorder hoist an
`#include` past other statements. morphe cannot prove that safe — it never resolves
an include, so it cannot see what the included file contributes — and the transform
rests on *your* assertion that your includes carry nothing order-sensitive. When
`reorder_includes` actually moves an `#include`, morphe emits a warning that names
the move and states it rests on that unverifiable assertion. Everything in
`[unsafe]` is a capability you turn on for your own program: never a default, never
silent.

The split is orthogonal to style — these toggles choose *which sound transforms
run*, never *how code lays out* — so morphe stays as opinionated about appearance
as `black` or `gofmt`.
