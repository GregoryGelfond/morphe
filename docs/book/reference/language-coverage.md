# Language coverage

morphe does not parse ASP itself. It is built on **themelios-syntax**, and what
morphe admits is exactly what that parser admits — so "what morphe covers" is a
question about the tier's language surface, not a second grammar morphe maintains.

## The two dialects

morphe formats two dialects, selected by `--dialect` (default `clingo`):

- **`clingo`** — the clingo/clingcon language: rules and integrity constraints,
  choice rules and aggregates (`#count`, `#sum`, `#min`, `#max`), conditions,
  optimization (`#minimize` / `#maximize` and weak constraints), the directive
  families (`#show`, `#const`, `#defined`, `#external`, `#program`, `#include`,
  `#project`, `#heuristic`, `#edge`, `#theory`), clingcon **theory atoms** (`&dom`,
  `&sum`, `&distinct`, …), and embedded `#script(python|lua)` blocks.
- **`asp-core-2`** — the ASP-Core-2 standard, including its query form (an atom
  followed by `?`, which must be the program's final statement).

Both dialects share the `.lp` extension, so nothing disambiguates by filename; the
default holds unless you set `--dialect`.

## What morphe admits, and what it refuses

morphe formats a **member** — a program the tier parses without error. Everything
the typed AST expresses, morphe lays out: rules, aggregates, theory atoms and
`#theory` definitions, comments and `%!` documentation, and `#script` regions (kept
verbatim unless you opt into [embedded formatting](../guide/embedded.md)).

A file with parse errors is a **non-member**, and morphe refuses it whole rather
than formatting around the broken regions (see [How morphe proves it didn't change
your program](../guide/safety.md)). A program nested more deeply than the tier's
default limit is likewise refused as a parse error, rather than risking the stack.

Because admission is the tier's, morphe tracks the language by advancing its pinned
`themelios-syntax` revision — never by growing a parser of its own.
