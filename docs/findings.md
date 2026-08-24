# morphe — findings ledger

The checkpoint's record (docs/design/morphe.md §14): one entry per friction
point met while building morphe against `themelios-syntax`'s formatter-facing
surface (syntax.md §15), each tagged by that surface's stable/free split
(§3.2).

- **stable** — a finding against a held-stable item: the kind roster's names,
  the tree laws and `role`, attachment's three slots and four facts, the two
  certificates' definitions, the oracle's exactness, the diagnostic
  identities, the entry points, or the token-source door. This is the
  checkpoint firing at the tier's design; it is carried to `themelios-syntax`
  and resolved there — syntax.md reopens to eat it — never worked around
  silently here, and never patched into the tier speculatively mid-build
  (§14.2).
- **free** — a finding against a free-to-move item: an accessor name or shape
  in `ast`, a whitespace-fact helper name, a message text, an `ERROR`-node
  shape under recovery, or the convenience of the two forms of attachment and
  the oracle. This is ergonomics: morphe absorbs it, or the tier tweaks the
  convenience layer without a ruling.

Each entry records the surface item, what morphe wanted, what it got, and the
cost of the gap (§14.1).

## Entries

### 1. The token-level oracle reads the input's own text

- **Tag:** free
- **Surface item:** `fusion::separator(left, right, dialect)` (syntax.md §10).
- **Wanted:** a token-level oracle morphe could hand the two *emitted* texts,
  for the house style's `emit` step (§5.5).
- **Got:** `separator` reads `left.text()` internally, so it answers only for
  the input's own spelling. A canonicalizing formatter emits canonical
  spellings, not the input's (§7.4), so `emit` cannot use `separator` and
  instead calls the text-level form, `separator_between(canonical(l),
  canonical(r), LexContext { dialect, mode: lex_mode_of(l) })`, on the emitted
  texts, reconstructing the lex mode from the input token (§5.5).
- **Cost:** one extra call (`lex_mode_of`) to reconstruct what `separator`'s
  convenience form would have supplied directly; absorbed in `emit`, no
  correctness gap — `separator_between` is exact on any texts (syntax.md §10).

### 2. No trailing-separator accessor beyond `Tuple`

- **Tag:** free
- **Surface item:** `ast::Tuple::trailing_comma_token`,
  `ast::Pool::{tuples, parenthesized}` (syntax.md §8.2).
- **Wanted:** a trailing-separator accessor on every construct the magic
  trailing comma (§7.5) might read — tuples, pools, aggregate-element and
  annotation lists — so the lever reads each the same way.
- **Got:** `Tuple::trailing_comma_token` is exposed; `Pool` exposes only
  `tuples()`/`parenthesized()`, with no trailing-`;` accessor, and neither the
  aggregate-element lists nor `Annotation` expose a trailing-separator
  accessor at all.
- **Cost:** none in v1. Every trailing separator — a tuple's comma, a pool's
  `;` — is preserved as an authored token and laid out by width alone (flat
  when it fits, broken when it does not, never force-exploded), so no lever
  reads a trailing-separator accessor and the absent pool, aggregate-element,
  and annotation accessors cost nothing. **Confirmed at first use:**
  `Tuple::trailing_comma_token` is present; the pool `;`, aggregate-element,
  and annotation trailing-separator accessors are absent, as recorded. The
  §7.5 wording still specifies a force-explode on a redundant trailing
  separator; v1's width-only reading and that wording are reconciled for the
  design later in the build.

### 3. Token text is cursor-scoped, not tree-lifetime

- **Tag:** free
- **Surface item:** `tree::SyntaxToken::text() -> &str` (rowan) and the typed
  token wrappers' `value()` accessors (syntax.md §8).
- **What morphe wanted:** to borrow each token's spelling at the tree's
  lifetime, so the `Doc<'a>` IR borrows its text from the source tree and "the
  walk copies no text" (§6).
- **What it got:** rowan's `SyntaxToken::text()` is tied to the token cursor,
  not the tree; there is no tree-lifetime token-text accessor, and
  `format_parsed` holds only a `Parse` whose `syntax().text()` is a rope, not a
  contiguous `&str`. So the lowering owns each token spelling (`Cow::Owned`) —
  which the canonicalizing default owns on respell anyway.
- **Cost:** the `Doc<'a>` zero-copy intent (§6) is not realized for token
  spellings; the walk copies token text (O(text), negligible for v1). No
  correctness gap. A tree-lifetime token-text accessor (or a source-slice-by-
  span reader) would restore the borrow path; until then §6's "the walk copies
  no text" overstates what the surface allows — raised for the design later in
  the build.

---

Not a finding: a `Parse::nesting_limit()` accessor. morphe uses the tier's
baked-in `parse` door throughout (§5.6), never the finer-grained limit door,
so it never reads a `Parse`'s nesting limit back out. That accessor is a
concern only of the deferred `CEILING`-aware `format_parsed` seam (§16), not a
v1 gap.

## Close

At the build's close, every entry above is reviewed against syntax.md §15's
split (§3.2):

- All three are **free** items — the token-level oracle's convenience form
  (#1), the trailing-separator accessors `ast` does not expose beyond `Tuple`
  (#2), and rowan's cursor-scoped token text (#3) — and each is absorbed:
  `emit` reconstructs the lex mode and calls `separator_between` on the emitted
  texts (#1); the width-only trailing separator reads no accessor, so the
  absent ones cost nothing (#2); the lowering owns each spelling and `Doc` owns
  its text, §6 reconciled later in the build (#3).
- There is **no stable-item finding**. Nothing routes to themelios-syntax
  through a ruling at this close (§14.2): the checkpoint fired only at the
  tier's *free* convenience layer, which morphe absorbed — never at a
  held-stable name, tree law, certificate, oracle, or door.

The ledger is current: every friction met against the surface is recorded
above and routed by the split, none worked around silently (§2). The comment
placement corrected at this close was morphe's own lowering, not the surface —
no finding against the tier. It did move morphe off the design's unconditional
slot-preservation invariant, though: a neck-to-body dangling comment now rides
with the body, re-attaching as `Leading`. That reconciliation is folded into
the design of record at this close (morphe.md §8.2, §5.4, §13), where
idempotence for the edge is rested on convergence to a fixed point — a design
catch-up, still not a themelios finding.
