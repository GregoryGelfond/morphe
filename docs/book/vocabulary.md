# Vocabulary

One concept, one name. This page is the registry the rest of the book and the
source hold to.

| Concept | morphe term |
|---|---|
| The relation morphe proves its output stands in to the input — nothing but whitespace changed | **`LayoutOnly`** |
| The same, save that interchangeable operators were normalized to a canonical spelling | **`UpToSpelling`** |
| The proof, re-derived by re-parsing morphe's own output before it is returned, that one of those relations holds | **certificate** |
| The authority (from themelios-syntax) that fixes the minimum whitespace two tokens must carry between them | **oracle** — the fusion oracle, a *floor* the house style may exceed but never undercut |
| An input morphe can parse, and so format | **member** — a **non-member** has parse errors and is refused, unformatted |
| A top-level statement the reorder moves as a whole, carrying its comments with it | **unit** |
| A statement the reorder never moves, and never reorders across | **barrier** (a fence) — `#include` by default |
| The proof that a reorder was a pure permutation of the units — nothing added, dropped, or split | **permutation certificate** |
| Where a comment attaches, and in which position | **anchor** and **slot** — `Leading`, `Trailing`, or `Dangling` |
| A boundary that admits external input | **door** |
| The fixed set of layout decisions morphe imposes | **the house style** — whose one configurable knob is the **line width** |
| The contained boundary through which an embedded formatter (ruff, StyLua) is called | **`EmbeddedFormatter`** — the seam |
| What a failed, unavailable, or disturbing backend converges to: the body left verbatim, a note recorded | **degradation** |
