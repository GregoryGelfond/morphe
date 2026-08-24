# morphe — design

morphe (μορφή, *form*) is a formatter for Answer Set Programming: the
`rustfmt`/`black`/`ruff`-class tool for the clingo/clingcon dialect and
its ASP-Core-2 variant. It reads a program through `themelios-syntax`,
lays it out in one fixed house style with a single knob, and **refuses to
write any output it cannot certify**: a *format* is layout-only or
up-to-spelling equivalent to the input (§5.2), a reformatted `#script`
interior carries its own delegated guarantee (§9.3), and an opt-in *reorder*
its own permutation certificate (§15). This document is its design of
record.

morphe is also a *checkpoint*. themelios spec §11 and syntax.md §15 make
stage 2 exit through a first real consumer, built outside the syntax
repository, whose friction reveals the tier's ergonomics — "witnesses
prove capability; only a consumer reveals ergonomics." morphe is that
consumer, and it is the one consumer checkpoint that gates themelios v1
completion. So this design is written to two ends at once: to be a
best-of-breed formatter, and to *exercise and stress* the formatter-facing
surface and route what it finds back into the tier (§14).

Cross-references of the form "syntax.md §N", "spec §N", "grammar §N" are
to the themelios repository (`~/Projects/themelios/docs/`). References of
the form "§N" alone are to this document.

---

## 1. What morphe is

- A **library** (`morphe`) — the formatter as a pure, I/O-free API — and a
  **thin binary** (`morphe-cli`, the `morphe` command) over it. The
  library is the product; the CLI, an editor's format-on-save, and the
  eventual lint face are all consumers of it (spec §1.1, the ecosystem
  thesis).
- A **black-class** formatter: opinionated, one fixed house style, a
  single knob (the line width). Not `rustfmt`-class configurability —
  "any two morphe users produce the same layout." Configurability may
  accrete later toward the north star; v1 does not carry it (§16).
- **Safety-certified.** morphe walks the typed AST, emits tokens whose
  every adjacency clears the fusion oracle's veto, carries comments to
  their anchors, keeps documentation verbatim and — by default — `#script`
  bodies too (an opt-in pass reformats them with ruff for Python or StyLua
  for Lua, §9), optionally normalizes synonym spellings, and **refuses to
  write until `equivalent(before, after, certificate)` grants the claim for
  the ASP program** (§5; a reformatted `#script` interior carries its own
  delegated guarantee, §9.3).
- The successor to **kallos** (the predecessor ASP formatter, spec §5.1),
  rebuilt so that each of kallos's three recorded defects is not merely
  avoided but *structurally impossible*, because the mechanisms that
  caused them are now owned by themelios-syntax rather than by the
  formatter (§3.3). Informed greenfield: kallos is evidence, not
  inheritance — its house style and its Wadler/Lindig document engine are
  re-derived here, not ported (spec §5.1's "read for the problem, then
  build new").

**The north star, and v1's place in it.** The eventual morphe is the full
`rustfmt` + `ruff`/`black` analogue for ASP: a formatter, a set of
transformations, *and* a linter. v1 is the **formatter plus one opt-in
transformation** — the conservative signature reorder (§15), the ASP
analogue of `rustfmt`'s import ordering — with the lint face present as a
*designed seam* and no shipped rule engine (§11.3). This is deliberate: the
formatter already exercises the whole formatter-facing surface, so it is
the maximal tier-stress per unit of morphe; the reorder adds the one
capability that stresses attachment under statement *moves*; and deferring
the lint engine lets the syntax tier's own diagnostic/`Hint` work be
*driven by demonstrated need* rather than front-loaded speculatively
(§14.3).

## 2. What this design is for

The scope is settled; this document records it as a plan-ready design.
It fixes:

- the surface morphe consumes and the rule by which its findings route
  back to the tier (§3);
- the crate and API architecture (§4);
- the safety spine — the pipeline, the certificate as an internal
  invariant, idempotence, the oracle floor, and depth safety (§5);
- the document model morphe lowers into (§6) and the house style it
  encodes (§7);
- comment placement, morphe's single hardest problem (§8);
- embedded `#script` formatting behind a pluggable, contained seam (§9);
- the CLI and its outcome contract (§10);
- diagnostics and the deferred lint seam (§11);
- the trust and dependency posture (§12);
- the assurance instruments (§13) and the checkpoint's findings apparatus
  (§14);
- the opt-in transformations — the conservative signature reorder (§15);
- the reserved seams and non-goals (§16).

It does **not** re-derive the inherited surface: the lexer, the tree, the
typed AST, attachment, the oracle, and the certificate are themelios's,
consumed as-is. Where morphe wishes one of them were different, that is a
checkpoint finding (§14), not a redesign here.

**This design has failed** — as syntax.md §2 and grammar.md §2 state their
own failure conditions — when any of the following holds: morphe writes
output whose ASP program it did not certify (§5.2); it certifies a changed
significant token, or a comment moved across a token, as layout-only or
up-to-spelling; it applies a reorder it did not prove by the permutation
certificate (§15.3), or moves a barrier or reorders across one (§15.4); `format(format(x)) != format(x)` for a member `x`; a member is
refused as `CertificateBroke` (§4.2); it re-derives a spacing, attachment,
or equivalence rule the tier owns (§3.2, the ownership line); or it works
around a held-stable item of the split (§3.2) silently instead of routing
it as a finding (§14).

## 3. The consumed surface, and the findings rule

### 3.1 What morphe takes from themelios-syntax

The whole of syntax.md §15's formatter-facing surface:

- **`parse` and `Parse<T>`** (syntax.md §5.5, §6.1) — the entry, and
  `has_errors()`, `diagnostics()`, `tree()`, `syntax()`, `is_incomplete()`.
- **The typed AST** (syntax.md §8) — `ast::Program` and the statement,
  head, body, term, aggregate, and theory enums and their accessors;
  `HasDocs` and `HasGuards`; the token wrappers; `ScriptStatement::body`
  and `ScriptBody::value` for the opaque script region.
- **The kind roster and `role`** (syntax.md §5.2, §5.4, Appendix A) —
  `TokenRole::{Documentation, Trivia, Significant}`.
- **Comment attachment** (syntax.md §9) — `attachments` (bulk),
  `attachment`, `comments`, `Slot::{Leading, Trailing, Dangling}`, and
  the whitespace facts `same_line`, `empty_line_between`,
  `line_breaks_between`.
- **The fusion oracle** (syntax.md §10) — `separator`, `separator_between`,
  `lex_mode_of`, `Separator::{Nothing, Whitespace, LineBreak}`.
- **Token-stream equivalence** (syntax.md §11) — `Certificate::{LayoutOnly,
  UpToSpelling}`, `equivalent`, `Mismatch`/`Side`, `non_whitespace_tokens`,
  `token_stream`, `comment_sequence`, and `canonical_spelling`.
- **`Dialect`** (syntax.md §3), and through the tier's re-export of base,
  the line index and the diagnostic views.

### 3.2 The findings rule (syntax.md §15's split)

syntax.md §15 fixes what is **held stable** across the checkpoint and what
is **free to move** on its findings. This split *is* morphe's
findings-routing rule (§14):

- A finding against a **held-stable** item — the kind roster's names, the
  tree laws and `role`, attachment's three slots and four facts, the two
  certificates' definitions, the oracle's exactness, the diagnostic
  identities, the entry points, the token-source door — is the checkpoint
  firing *at the tier's design*. It reopens syntax.md through a ruling;
  morphe does not work around it silently.
- A finding against a **free** item — accessor names and shapes in `ast`,
  the whitespace-fact helper names, message texts, the exact `ERROR`-node
  shapes under recovery, the convenience of the two forms of attachment
  and oracle — is ergonomics. morphe absorbs it, or the tier tweaks the
  convenience layer without a ruling.

### 3.3 kallos, and why its scars cannot recur here

kallos (Rust, tree-sitter-backed, MIT) is a working pure-layout ASP
formatter. Its three recorded defects each trace to a mechanism kallos had
to own because *its parser was not its own*, and each is now the tier's:

| kallos's choice (the scar) | what morphe consumes instead |
|---|---|
| comments in a **side table** keyed by `Node::id()`, stripped from the walk then re-injected | attachment as a **pure reading** of a lossless tree (§8; syntax.md §9) — nothing stored, nothing to go stale |
| an `≈` self-check comparing the token stream and the comment stream as **two independent projections** — structurally blind to a comment moved across a token; the transposition was caught only by an idempotence/differential corpus, never by the gate | the certificate compares the **single interleaved non-whitespace sequence** (§5.2; syntax.md §11.1) — "only the interleaved sequence sees it" |
| a **hand-maintained** "reachable-honest, default-keep" fusion table (kallos could not relex) | the **relexing oracle** (§5.5; syntax.md §10) — exact, no default to hedge |

morphe keeps kallos's genuinely good calls, re-derived: the Wadler/Lindig
document engine (§6), explicit-stack depth-safe walks (§5.6), the one-knob
black-style config (§7), the meaning-preserving / comments-verbatim /
idempotent property triad (§13), the `--safe`-equivalent gate always on,
an out-of-band clingo differential (§13), and kallos's clingofmt-derived
corpus as adversarial *inputs* (MIT, inputs-only, attributed — already
vendored by themelios, spec §10.3).

## 4. Architecture

### 4.1 Crates

- **`morphe`** (library) — the formatter's whole logic: the typed-AST →
  document lowering, the house style, the emission spine under the oracle
  floor, comment placement, the certificate gate, the `EmbeddedFormatter`
  trait and registry, and the conservative reorder transform (§15). **Pure
  and I/O-free at its core**: it reads text and a configuration and returns
  text or a typed refusal. It never opens a file, spawns a process, or
  discovers a tool; I/O enters only through embedded backends a caller
  injects (§9).
- **`morphe-cli`** (binary, the `morphe` command) — the thin adapter:
  argument parsing, file and directory walking, stdin, `--check`/`--diff`
  rendering, diagnostics rendering, and the *wiring* of the default
  embedded backends. All of morphe's I/O lives here.

The Wadler/Lindig document engine (§6) is a **well-bounded module inside
`morphe`** for v1, not its own crate — extractable later if a second
consumer wants it (kallos split it out; that is a YAGNI departure here).
Its edges are kept clean so extraction stays cheap.

morphe's lint and ngo-style rewrite capabilities (§11.2, §16) land as a
**future `morphe-rewrite` crate** over `themelios-syntax` — the
guarded-rewrite registry, deliberately outside the formatter library — not
built in v1.

### 4.2 The library API

The primary entry mirrors the tier's `parse` posture — total over its
inputs, pure, its failure semantics stated on the operation:

```rust
/// Format a source program. Parses under `options.dialect`, lays it out,
/// and returns the formatted text with the certificate it earned — or a
/// typed refusal. Pure; no I/O beyond any embedded backend in `options`.
pub fn format(source: &Source, options: &FormatOptions)
    -> Result<Formatted, FormatError>;

/// Format a program already parsed — for a consumer holding a
/// `Parse<Program>` (the language server, the program tier; syntax.md
/// §15), so morphe never forces a re-parse. The dialect is read from
/// `parse.dialect()` (syntax.md §5.5); this path ignores
/// `options.dialect`, never restating it, so the two cannot disagree.
/// **Precondition:** the `Parse` was built by the file door `parse` (at
/// `NestingLimit::DEFAULT`), the one parsing mechanism morphe uses — its
/// step-4 re-parse runs at `DEFAULT` too (§5.6). A `Parse` built deeper
/// (via the tier's finer-grained limit door at `CEILING`) is outside this
/// contract, a v1 non-need (§16).
pub fn format_parsed(parse: &Parse<Program>, options: &FormatOptions)
    -> Result<Formatted, FormatError>;

/// Reorder a program's top-level units into the canonical signature order
/// (§15) — a *distinct* operation from `format`, which never changes order.
/// Formats each unit, moves whole units (carrying their comments) within the
/// regions the barriers divide (§15.1, §15.4), and holds every barrier in
/// place. `options` carries the reorder toggles (`reorder_includes`, §10.2).
/// Pure. (No `reorder_parsed` sibling in v1 — the reorder re-parses
/// internally for its permutation check, §15.3; a `Parse`-taking form is a
/// future seam for an in-process consumer, as `format_parsed` is.)
pub fn reorder(source: &Source, options: &FormatOptions)
    -> Result<Reordered, ReorderError>;
```

```rust
/// The formatted result. `certificate` is the claim morphe proved of the
/// output's **ASP program (skeleton)** against the input (§5.2, §5.3),
/// bodies held verbatim for that proof. When `embedded_changed` is true a
/// `#script` interior was reformatted (§9.3): that interior is *outside*
/// the certificate, under its backend's delegated guarantee plus the
/// equal-except-script-bodies check, so the whole-text relation to the
/// input is "certificate ∧ equal-except-`SCRIPT_BODY`", not the
/// certificate alone. `changed` is whether the text differs from the
/// input; `notes` carries the per-script embedded outcomes and other
/// warnings (§9.4, §11).
#[derive(Clone, Debug)]
pub struct Formatted {
    pub text: String,
    /// The certificate proved of the ASP skeleton (§5.2).
    pub certificate: Certificate,
    /// Whether the embedded pass reformatted at least one `#script` body
    /// (§9.3) — the signal that the whole-text guarantee is the composed
    /// one, not the certificate alone.
    pub embedded_changed: bool,
    pub changed: bool,
    pub notes: Vec<Diagnostic>,
}

/// Why morphe declined to produce certified output.
#[derive(Clone, Debug)]
pub enum FormatError {
    /// The input is not a member — it has parse errors. morphe does not
    /// format a non-member (§5.7); the diagnostics are returned for the
    /// caller to render, unchanged from the tier.
    HasParseErrors(Vec<SyntaxError>),
    /// morphe produced output it could not certify against the input.
    /// This is a morphe **bug**, never the input's fault; `reason` carries
    /// the witness matched to the class of defect. It must never occur;
    /// that it is a typed value rather than a panic is the totality
    /// discipline (§12), and the CLI maps it to `Outcome::InternalError`
    /// with a bug-report ask (§10.3).
    CertificateBroke { certificate: Certificate, reason: CertificateBreak },
}

/// Which class of certificate break — the witness matched to the defect,
/// so the loud `Outcome::InternalError` report (§10.3) locates it: a
/// `TokenMismatch` when the candidate is a member whose tokens diverge,
/// `NotAMember` (carrying the tier's own diagnostics) when morphe's output
/// does not re-parse as a member at all.
#[derive(Clone, Debug)]
pub enum CertificateBreak {
    TokenMismatch(Mismatch),
    NotAMember(Vec<SyntaxError>),
}

/// The reorder result (§15). `text` is formatted, and reordered when any
/// reorderable unit moved (the per-unit `certificate` and the **permutation
/// certificate** (§15.3) both proved); `reordered` is false when the program
/// is all barriers or already in order — `text` is then merely formatted.
/// The reorder never skips a member: order-sensitivity is handled by barriers
/// (§15.4), not a whole-program precondition.
#[derive(Clone, Debug)]
pub struct Reordered {
    pub text: String,
    /// The **uniform claim class** every unit earned (§5.3), not a per-unit
    /// value: `preserve_spellings` fixes one target for all, so this is
    /// `UpToSpelling` if any unit canonicalized, else `LayoutOnly` (weakest
    /// wins). The permutation certificate (§15.3) carries no value of its
    /// own — its success is the `Ok` return.
    pub certificate: Certificate,
    /// Whether the permutation was applied and proved (§15.3) — false when
    /// the program is barrier-only or already in order (nothing moved); then
    /// `text` is merely formatted. The reorder never skips a member (§15.4).
    pub reordered: bool,
    pub changed: bool,
    pub notes: Vec<Diagnostic>,
}

/// Why morphe declined to produce a reorder result.
#[derive(Clone, Debug)]
pub enum ReorderError {
    /// A non-member — parse errors; refused as `format` does (§5.7).
    HasParseErrors(Vec<SyntaxError>),
    /// morphe produced a reorder it could not prove a pure permutation of
    /// the input — a morphe **bug** (the reorder analogue of
    /// `CertificateBroke`), mapped to `Outcome::InternalError` (§10.3,
    /// §15.3). The witness distinguishes the two failure shapes §15.3
    /// checks: a **per-unit** layout break (a token `Mismatch`, from that
    /// unit's own `format`), or a **bijection** breach.
    PermutationBroke { unit: Option<Mismatch>, bijection: Option<BijectionBreach> },
}

/// A breach of the permutation's structural law (§15.3) — always a morphe bug.
/// The check is a both-ways multiset count per barrier-divided region, so it sees
/// a unit's whole subsequence present or absent: a split unit surfaces as one
/// dropped and two added, a duplicated one as an extra added — the variants are
/// exactly what the count draws.
#[derive(Clone, Debug)]
pub enum BijectionBreach {
    UnitAdded { at: usize }, UnitDropped { at: usize },
    BarrierMoved { at: usize },
    /// The reassembly did not re-parse as a member; the tier's diagnostics.
    NotAMember(Vec<SyntaxError>),
}
```

```rust
/// The whole of morphe's configuration: one style knob (the line width), the
/// dialect and spelling policy, the embedded backends, and the reorder's
/// safe/unsafe toggles (§10.2). Style stays opinionated — `line_width` is the
/// *only* layout knob; indent and the rest are fixed (§16). The other fields
/// select the dialect, and which sound transforms run.
#[derive(Clone)]
pub struct FormatOptions {
    /// The one style knob. `DEFAULT_LINE_WIDTH` (§7.1). Indent is fixed
    /// (`INDENT`, §6); configurable indent is a §16 seam.
    pub line_width: usize,
    /// Which dialect to parse and lay out under (syntax.md §3). Used by
    /// `format`; `format_parsed` reads the dialect from the `Parse` and
    /// ignores this field.
    pub dialect: Dialect,
    /// Whether to preserve the author's synonym spellings. Off (the
    /// default) normalizes to canonical spellings (§7.4) and earns
    /// `UpToSpelling`; on preserves them and earns the stricter
    /// `LayoutOnly` (§5.3).
    pub preserve_spellings: bool,
    /// The embedded-script formatters, injected. Empty (the default) or
    /// backend-less means every `#script` body stays verbatim (§9). The
    /// library hardcodes no backend; the CLI supplies the defaults.
    pub embedded: EmbeddedFormatters,
    /// Whether embedded formatting is applied at all (the runtime
    /// opt-in). Off by default — nobody's script is reformatted unasked
    /// (§9.2).
    pub format_embedded: bool,
    /// The reorder's one **unsafe** toggle (§10.2, §15.4): treat a top-level
    /// `#include` as a reorderable, hoistable `Include` section rather than a
    /// barrier — the *caller's assertion* that their includes carry no
    /// order-sensitive construct. Off by default (includes are barriers,
    /// sound). Read only by `reorder`.
    pub reorder_includes: bool,
}
```

`notes` carries base's `Diagnostic` (re-exported through the tier,
syntax.md §1) — a severity, a message, and an optional location — for
embedded skips (§9.4) and other warnings that do not refuse the format;
morphe adds no diagnostic model of its own, so nothing here duplicates what
the tier owns (§16). Every configuration literal above resolves to a named
constant (`DEFAULT_LINE_WIDTH`, and the `Default` impl's dialect and
flags); no bare number carries meaning (§10.4, spec §5.2).

### 4.3 Module layout (the isolation seams)

Inside `morphe`, one module per concern, each understandable and testable
alone:

- `options` — `FormatOptions`, `Formatted`, `FormatError`, `Reordered`,
  `ReorderError`, `BijectionBreach`, the named defaults.
- `doc` — the Wadler/Lindig document IR and its renderer (§6).
- `lower` — the typed-AST → document lowering (§7): the house style, as
  code.
- `emit` — the token joiner that enforces the oracle floor (§5.5); comment
  weaving lives in `lower` (§8).
- `comments` — the reading of `attachments` into the placement plan (§8).
- `certify` — the certificate gate, the equal-except-script-bodies check,
  and the permutation certificate (§5.3, §9.3, §15.3).
- `reorder` — unit-and-barrier extraction, the bucket-emit, and
  applicability detection (§15); it invokes `certify` for the permutation
  certificate (which lives there, §15.3), so it sits next to it.
- `embedded` — the `EmbeddedFormatter` trait, the registry, and the
  backends behind their features (§9).
- `spelling` — the canonicalization pass over the document (§7.4).

The dependency order among them is the crate root's `mod` order (rustfmt
is told not to reorder; §4.1's rustfmt.toml).

## 5. The safety spine

### 5.1 The pipeline

`format` is a fixed sequence, each step total:

1. **Parse.** `parse(source, options.dialect) → Parse<Program>`. If
   `has_errors()`, return `Err(HasParseErrors(diagnostics))` (§5.7).
   `format_parsed` enters here with the parse in hand.
2. **Lower.** Walk the typed AST → a `doc::Doc` (§6, §7), applying
   `canonical_spelling` unless `preserve_spellings` (§7.4), rendering each
   `#script` body as its verbatim `ScriptBody::value()`. The **oracle
   floor is applied here**, at Doc-build time: `emit` consults the oracle
   on the *canonical* texts it will emit and selects the separator node
   accordingly (§5.5), fixing the floor into the document before any width
   choice. Comments are read once via `attachments` and woven in as the
   walk reaches their anchors (§8).
3. **Render.** `doc::render(doc, line_width) → skeleton_text` — the flat-
   or-broken choice by width (§6). `render` is oracle-oblivious: the floor
   was fixed in step 2, and breaking a group only *strengthens* a
   separator (§5.5), never drops below it.
4. **Certificate gate.** Re-parse `skeleton_text` through the file door
   `parse(skeleton, dialect)` under the input's dialect — the baked-in
   `NestingLimit::DEFAULT` (§5.6), the one parsing mechanism morphe uses for
   both `format` and `format_parsed` — then run `equivalent(input, skeleton,
   certificate)` (§5.3). On `Err` — a token `Mismatch`, or (guarded first) a
   skeleton that did not re-parse as a member at all (§4.2's two break
   classes) — return `Err(CertificateBroke)`; the invariant failed, and
   morphe never emits uncertified text.
5. **Embedded pass** (only if `format_embedded` and a backend claims a
   script's language). Format each script body behind the contained seam,
   splice on success, keep verbatim on any failure, and verify the whole
   with the equal-except-script-bodies check (§9.3). Produce `final_text`.
6. **Result.** `Formatted { text: final_text, certificate, changed:
   final_text != input_text, embedded_changed, notes }` — where
   `input_text` is `source.text()` for `format` and
   `parse.syntax().text()` for `format_parsed`, and `embedded_changed`
   records whether step 5 replaced any body (§9.3).

### 5.2 The claim morphe makes

For a member input, morphe's output stands in one of exactly two relations
to the input, and morphe *proves* which before returning it:

- **`LayoutOnly`** — nothing but whitespace changed (syntax.md §11.2);
- **`UpToSpelling`** — as `LayoutOnly`, save that synonym spellings were
  normalized through `canonical_spelling` and nothing else.

Because the certificate compares the single interleaved non-whitespace
sequence — significant tokens and trivia comments in order — a granted
certificate means: the same tokens, the same comments, each still standing
where it stood among the others. A comment moved across a token (kallos's
transposition) is a different sequence and is refused. This is morphe's
meaning-preservation guarantee for a **format** — a layout-only or
up-to-spelling change; the opt-in reorder carries its own (§15.3) — and it
is *checked*, not trusted (§12).

### 5.3 The certificate is an internal invariant, not a mode

kallos's equivalence check was an opt-in `--safe` flag. morphe's is
**always on and internal**: `format` runs step 4 every time and cannot
return `Ok` text it did not certify. `preserve_spellings` selects *which*
certificate is the target — off ⇒ `UpToSpelling` (canonicalization ran),
on ⇒ `LayoutOnly` — and the target is dictated by what the lowering did,
never an independent knob: canonicalizing and then asking for `LayoutOnly`
would (correctly) refuse. A `CertificateBroke` is therefore always a morphe
defect, surfaced loudly (§10.3), never a user-facing "unsafe" mode.

### 5.4 Idempotence

morphe guarantees **`format(format(x)) == format(x)`** for every member
`x`. It follows from three facts, each independently held (§13):

- the house style is a function of the AST, which re-parsing recovers
  unchanged;
- comment placement preserves the four attachment facts (§8), so a
  re-parse attaches every comment to the same anchor and slot and morphe
  re-emits it identically — save one shape, a dangling comment that rides a
  rule's body (§8.2), whose slot the first format changes and the second
  holds fixed;
- `canonical_spelling` is idempotent (syntax.md §11.3), and where embedded
  formatting ran, the backends are idempotent (§9).

That second fact has one deliberate exception, and there idempotence rests
on **convergence** rather than preservation: the neck-to-body ride (§8.2)
re-slots a comment `Dangling`→`Leading` on the first format, but the
re-slotted output is already a fixed point — a second format finds the
comment `Leading`, emits it once at the same place, and returns the same
text. The interleaved sequence the certificate checks (§5.2) is preserved
in every case; where a slot is not, the fixed point is what closes the
argument.

Idempotence is not assumed from these; it is a property test over the
corpus (§13), because it is exactly the invariant kallos's coupled
comment-reclassification broke.

### 5.5 The oracle as a floor

The house style decides the whitespace it *wants* between two tokens; the
fusion oracle decides the minimum any correct formatter *must* leave. Where
these differ, morphe emits the maximum — and in practice they differ only
where the style would abut two significant tokens with no space.

**Where, and on which texts.** The floor is applied in `emit` at
**Doc-build time** (§5.1 step 2), not in `render`. When `emit` would place
significant tokens `l` and `r` adjacent, it asks the oracle for the minimum
separator and selects the document node accordingly: `SoftLine` (or bare
adjacency) only where the answer is `Nothing`, a forced space or `Line`
where `Whitespace`, a `HardLine` where `LineBreak`. Crucially it asks about
**the texts morphe will actually emit**. In the default, canonicalizing
mode those are the *canonical* spellings (§7.4), which are not the input
token's `.text()` — so `emit` cannot use the token-level `separator(l, r,
dialect)` (which reads `left.text()`, syntax.md §10) and instead calls
`separator_between(canonical(l), canonical(r), LexContext { dialect, mode:
lex_mode_of(l) })` (syntax.md §10) on the emitted texts, reconstructing the
mode from the input token. Under `preserve_spellings` the emitted text *is*
the input text and either form of the oracle agrees.

**Why this is width-independent, and why `render` need not know the
oracle.** `separator` depends only on the two texts and the mode, never on
flat-versus-broken (syntax.md §10). So the node `emit` chose fixes the
floor before any width decision, and a `Group` that breaks only ever turns
a `SoftLine` or a `Line`-space into a newline — it *strengthens* a
separator, never weakens one below `Nothing`. (The only oracle answer a
significant-token adjacency can need is `Nothing` or `Whitespace`;
`LineBreak` is a line-comment/doc/shebang property, carried by a
`HardLine`.) `render` is therefore oracle-oblivious by construction.

So the style never fuses `0` `x1` into `0x1`, `#sum` `+` into `#sum+`, or
`.` `.` into `..`; the oracle is the floor, and the certificate (§5.2,
§5.3) is the whole-text backstop should the floor ever be mis-queried.
morphe maintains *no* spacing table of its own — the defect that forced
kallos to (§3.3). That a canonicalizing formatter must reach for
`separator_between` rather than the token-level `separator` convenience is
a §14 checkpoint finding of the *free* kind (§3.2).

### 5.6 Depth safety

themelios bounds the tree's depth (syntax.md §5.4 law 3, the `NestingLimit`
gate) at two limits (syntax.md §6.6): `DEFAULT` (128 frames), safe to
*hold* — build, walk, drop — on a modest stack; and `CEILING` (5,000),
safe only on a `REQUIRED_STACK_BYTES` (64 MiB) thread under
`with_required_stack`. **`format` enters through the file door `parse`,
which parses at `DEFAULT`** (syntax.md §6.1), so it never builds a tree
past 128 frames — input nested deeper is *refused* (`nesting-too-deep`) and
returns `Err(HasParseErrors)` (§5.7), not formatted. Within `DEFAULT`,
morphe's own walks are safe on an ordinary thread, mirroring the tier's own
parser (syntax.md §6.2). The one **input-driven** depth source — **bracket
nesting** (argument lists, pools, tuples, the theory bracket families) — runs
on an **explicit frame stack**, so its nesting is the stack's height and never
call depth; the **rendering** walk (`render`/`fits`) is likewise explicit-stack.
The **structural** walk over the outer grammar — statements, rules, bodies,
heads, aggregates — is **bounded recursive descent**: those constructs do not
self-nest without a bracket between (an aggregate cannot nest inside its own
element's condition unbracketed), and the flat operator chains are one node
apiece — `Comparison` and `TheoryOpTerm` one node, `BinaryTerm` one node *per
precedence level* (syntax.md §8.2, §6.2), so an operator run is a loop, never
recursion. The outer recursion is thus bounded by the grammar, not the input,
and holds on the ordinary stack at `DEFAULT`. This mirrors kallos's one
unambiguously right structural call (spec §5.1) and the tier's own split.

**One parsing door, `DEFAULT`-baked.** morphe uses a single parsing
mechanism throughout — the file door `parse`, with `NestingLimit::DEFAULT`
baked in (syntax.md §6.1) — never the tier's finer-grained `parse_program`
limit door. So `format`'s input parse *and* the step-4 re-parse of both
`format` and `format_parsed` all run at `DEFAULT`. This fixes
`format_parsed`'s contract by a **precondition**: the `Parse` it is handed
must itself have been built by `parse` (at `DEFAULT`). Under that
precondition the re-parse is exact — a layout / up-to-spelling transform
preserves bracket depth, so the skeleton is as deep as the input
(≤ `DEFAULT`) and `parse` never refuses it — and both rowan trees (the input
and the re-parsed skeleton) and every walk are held and dropped on an
**ordinary stack**, so morphe owes *no* `with_required_stack` / 64 MiB
thread. A `Parse` built deeper (at `CEILING`, which syntax.md §6.6 invites
for a general consumer) is **outside `format_parsed`'s contract**: morphe
does not carry the deep-parse consumer path in v1 — a formatter has no real
need to lay out a program nested past 128 brackets — and re-parsing such an
input at `DEFAULT` may refuse it, a documented precondition violation rather
than a `format`-path guarantee. Should a consumer ever demonstrate the need,
a `CEILING`-aware `format_parsed` (and the `Parse::nesting_limit()` accessor
or `with_required_stack` obligation it would entail) is a reserved seam
(§16), not a v1 carry.

### 5.7 What morphe does with a non-member

A file with parse errors is **refused, not partially formatted** (v1).
`format` returns `Err(HasParseErrors(diagnostics))`; the CLI renders the
tier's diagnostics through base's human view (§11) and touches nothing.
The rationale: a formatter that reflows around `ERROR` regions must reason
about recovery shapes, which syntax.md §15 lists as *free to move* — a poor
foundation for a stable v1 — and refusing exercises the diagnostics
surface cleanly. Formatting the members around errors is a named later
seam (§16), to be opened when a consumer (an editor formatting a
half-typed buffer) demonstrates the need.

## 6. The document model

morphe lowers the AST into a **Wadler/Lindig document** and renders it at
the target width — the same discipline `black`, `ruff`, and `prettier`
use, and the one part of kallos worth re-deriving whole. The IR is small:

```rust
/// A layout-independent document. Its text is owned: rowan's token text is
/// cursor-scoped, not tree-lifetime, so the walk owns each spelling as it
/// lowers (§16 reserves a borrowing `Doc<'a>` for a future tier accessor).
/// Rendered to text at a target width by `render`, which chooses each group
/// flat or broken.
enum Doc {
    /// Verbatim text — one token's owned spelling, a fixed separator (a
    /// `'static` literal, borrowed without allocating), or a whole verbatim
    /// region (a doc line, a script body). Any newlines it contains are
    /// *content*, not layout: `render` emits them exactly and advances the
    /// column past the last one.
    Text(Cow<'static, str>),
    /// A soft break: a space when its group is flat, a newline (at the
    /// current indent) when broken.
    Line,
    /// A soft break that is *nothing* when flat, a newline when broken —
    /// for abutting tokens the oracle permits (§5.5).
    SoftLine,
    /// An unconditional newline — for a line comment, a doc line, one
    /// statement ending before the next begins.
    HardLine,
    /// A blank line kept as one (§7.3): two newlines when it stands.
    BlankLine,
    /// Increase the indent of everything within by one level.
    Nest(Box<Doc>),
    /// A choice point: render flat if it fits the remaining width,
    /// otherwise broken.
    Group(Box<Doc>),
    /// Concatenation.
    Concat(Vec<Doc>),
}
```

`render(doc, width)` is the standard algorithm: walk the document tracking
the column; for each `Group`, if its flat width fits the remaining columns
render it flat (every `Line`→space, `SoftLine`→nothing), else render it
broken (every `Line`/`SoftLine`→newline at the current indent, `Nest`
adding one indent level). The walk is **explicit-stack** (§5.6) and
allocation-frugal; its cost is linear in the document, which is linear in
the tree. `Text` holding a verbatim region (a doc line, a script body) is
never re-wrapped — its content is content, not layout (§7.3, §8); where it
carries newlines, `render` emits them verbatim and resets the tracked
column to the width of its final line.

`render` is oracle-oblivious by construction: `emit` fixed every
significant-token adjacency into the document at build time, on the texts
morphe emits, and breaking only strengthens a separator (§5.5). The one
layout constant the renderer holds is the indent unit `INDENT` (§10.4;
kallos's private `4`, named here); morphe carries no neck-width constant —
a broken neck's continuation is indented by `INDENT` like any other break
(§7.2).

## 7. The house style

One fixed style, `kallos`'s ASP-specific decisions re-decided at the
`black`/`ruff` bar. The catalog below is the design of record for layout;
the `lower` module encodes it, and every rule has a golden (§13).

### 7.1 The one knob, and the global shape

- **`line_width`**, `DEFAULT_LINE_WIDTH = 100` (kallos's and rustfmt's
  default). The sole configurable value.
- **One statement per line.** Each `Statement` renders on its own line(s),
  terminated by its `.` — or, for the ASP-Core-2 query, its trailing `?`,
  which hugs its atom like the `.` terminator (`a?`, `a(X)?`); no two
  statements share a line.
- **Blank-line runs collapse to at most one** (§7.3).
- **A trailing newline** terminates the file; none is doubled.
- **`INDENT = 4` spaces** per level; tabs are never emitted.

### 7.2 Spacing and breaking, per construct

The rules, stated as the `black`/`ruff` reflow discipline mapped onto
ASP's bracketed and `;`/`,`-separated constructs:

- **Necks.** `:-` and `:~` are spaced on both sides (`h :- b.`). A rule
  whose body does not fit breaks after the neck, one body element per
  line, indented one level; the head stays on the first line.
- **Argument lists and function terms** hug their parens tight:
  `p(X, Y)`, `@f(a, b)` — no space inside the parens, a space after each
  comma. A list that does not fit **explodes** one argument per line
  (§7.5).
- **Aggregates.** The set/function braces are spaced inside — `#count{ E1;
  E2 }`, `{ a; b }` — and the aggregate keyword hugs its brace (`#sum{`).
  Elements are separated by `; `; a non-fitting aggregate explodes one
  element per line.
- **Optimize statements** (`#minimize`, `#maximize`) are laid out **like an
  aggregate** — the principle of least surprise, an optimize statement being
  an aggregate-shaped construct: the keyword hugs its brace (`#minimize{`),
  the braces are spaced inside, the elements (`weight@priority, terms :
  condition`) are separated by `; `, and a non-fitting statement explodes one
  element per line — `#minimize{ 1@1, T : p(T) }`. (`#minimise`/`#maximise`
  normalize to `#minimize`/`#maximize`, §7.4.)
- **Comparisons and guards** space their relations: `X < Y`, `1 #count{…}
  3`. Guard sequences render in source order (syntax.md §8.2).
- **Disjunction** spaces its `|`: `a | b`. (The `|` is an anchor, not a
  closer — syntax.md §9.2; §8.)
- **Operator chains** render flat with spaced binary operators
  (`1 + 2 - 3`) and hugged unary operators (`-X`, `not p`); precedence and
  associativity are read from the node (syntax.md §8.2), never
  re-derived, and parentheses the author wrote are preserved (a `Pool` of
  one tuple of one term is the parenthesized form; syntax.md §8.2).
- **Bracket depth tightens spacing** — kallos's rule, which the §13 goldens
  pin. A `,`/`;` separator is spaced at one bracket deep and tight from two:
  `p(X, Y)` and `@f(a, b)` (spaced), but a nested list tightens — `p(f(a,b))`,
  a tuple inside an aggregate brace. A term operator is spaced only at the top
  level and tight inside any bracket: `X = Y + Z` and `1 + 2 - 3`, but `p(X+Y)`.
  (Comparison and guard relations are always spaced (above); `not` is always
  spaced; a classical or unary `-`/`~` always hugs its operand.)
- **Annotations** (the one `[…]` bracket family — weak-constraint
  `[w@p, t]`, `#heuristic`, `#external`, `#const` policy) hug their
  brackets and space after commas, like argument lists.
- **Directives** (`#show`, `#program`, `#const`, `#external`, `#edge`,
  `#project`, `#defined`, `#include`, `#theory`) render with a single
  space after the keyword and their family's punctuation spaced as above.
- **`#script(lang) … #end.`** — the header renders normally *up to and
  including the `)`*, after which the `SCRIPT_BODY` runs to `#end` (grammar
  §4.8). The body is emitted **byte-exact** from immediately after that
  `)`: leading and internal body whitespace is content (`value()` compares
  it, syntax.md §11.1), and the *only* layout latitude is the trailing
  blanks/tabs before `#end`, which `value()` already trims — so morphe may
  neither indent the body nor alter the whitespace at the `)`/body boundary
  (§8, §9). `#end.` stands on its own line where the body ends in a newline;
  where it does not, forcing one would change `value()` and break the
  certificate, so `#end` abuts the body's last line with a single space.

### 7.3 What is content, not layout

Three regions are emitted **verbatim**, never reflowed, because syntax.md
makes their bytes significant to the certificate:

- **Documentation** — the `%!` doc lines a statement owns (`HasDocs`, the
  `%!` line form; syntax.md §5.4, §8.2). Emitted exactly, above their
  statement, each on its line.
- **`#script` bodies** — `ScriptBody::value()`, verbatim (unless the
  embedded pass replaces the whole value; §9).
- **A line/block comment's text** — its bytes, though *trailing horizontal
  whitespace* on a line comment is layout and is trimmed (syntax.md §8.3,
  §11.1).

A single author blank line between statements is preserved as one blank;
a run of blanks collapses to one; leading and trailing file blanks are
removed. This is the `BlankLine` document node (§6), and it interacts with
attachment's block-aware detach (§8).

### 7.4 Spelling normalization

Unless `preserve_spellings`, morphe normalizes each synonym token to its
canonical spelling via `canonical_spelling` (syntax.md §11.3): `=` for EQ,
`!=` for NEQ, `#inf`, `#sup`, `#minimize`, `#maximize`. This is applied in
`spelling` as the document is built, converges the output on what clingo
prints (syntax.md §11.3), and moves the earned certificate from
`LayoutOnly` to `UpToSpelling` (§5.3). The table is language knowledge and
lives once, in the tier; morphe reads it and decides nothing about which
spellings are synonyms.

### 7.5 The trailing separator

`black` reads a redundant trailing separator as an author's request to
explode a group one element per line even when it would fit — its *magic
trailing comma*. morphe **declines** that lever, matching kallos: a trailing
separator is **preserved and hugged**, never a forced break, and **width
alone decides** whether the group stays flat or explodes. A tuple `(a, b,)`
stays `(a, b,)` when it fits and explodes — trailing comma and all — only
when it does not; the comma is layout the format keeps, never a signal it
acts on.

Two reasons the rule is width-only. First, **consistency with kallos** (the
ASP formatter morphe re-derives): kallos preserves and hugs a trailing
separator and never synthesizes one, and morphe matches it where it can.
Second, in ASP a trailing separator is **not always pure layout** — a
one-tuple `(a,)` is arity-distinct from the parenthesized `(a)` (syntax.md
§8.2), so the comma there is meaning-bearing, not a formatting toggle;
black's lever presumes an inert trailing comma, which ASP's tuple grammar
does not grant. One width-only rule covers every trailing separator alike.

The separator is preserved exactly, because it is a real token in the
certificate's sequence (§5.2): morphe keeps the author's choice and reads
only *width* from the group, never rewriting the token.

- **Tuples.** `(a,)` (a one-tuple) and `(a, b,)` (a redundant trailing comma)
  are each preserved and hugged; `(a)` (the parenthesized `a`) is left as it
  is. The tier exposes `Tuple::trailing_comma_token` (syntax.md §8.2), but the
  width-only rule needs no lever — the trailing comma rides through as an
  ordinary token.
- **Pools** admit trailing/empty `;` alternatives, meaning-bearing at their
  boundaries; these too are preserved and laid out by width.

morphe **never synthesizes** a trailing separator, and argument lists never
carry one (`f(a,)` is a non-member — syntax.md §7.1,
`Hint::TrailingCommaInArguments`). The absence of a trailing-separator
accessor beyond `Tuple::trailing_comma_token` is recorded as a §14
*free-item* finding (§3.2); it costs nothing while the layout is width-only.

## 8. Comment placement

Comments are every formatter's hardest problem and were the source of
kallos's transposition and non-idempotence. morphe's advantage is that it
does not *classify* comments — the tier does, as a total reading of a
lossless tree (syntax.md §9). morphe's job is the inverse: given each
trivia comment's `(anchor, slot)`, **emit it in the position that makes a
re-parse attach it to the same anchor and slot**. In all but one deliberate
case that is exactly preserving the four attachment facts (syntax.md §9.2),
and it is what keeps the certificate granted and the format idempotent. The
one exception — a dangling comment that rides a rule's body, re-attaching as
`Leading` (§8.2) — changes the slot on purpose, and stays idempotent by
converging to a fixed point (§5.4) rather than by preserving it.

### 8.1 The placement plan

Before rendering, `comments` folds `attachments(root)` into a plan keyed by
anchor: for each node or significant token that anchors comments, its
leading run, its trailing run, and (for a parent) its dangling comments,
each in source order. The bulk `attachments` pass is O(tree); the plan is
consulted in O(1) as the lowering reaches each anchor, so morphe pays O(n)
overall, never the O(m²) of asking `attachment` per comment (syntax.md
§9.3).

One datum lives beyond the plan, at emit time. A statement's leading comment
that falls among its documentation lines (§7.3) must be emitted in source
order *there* — deferring it to the anchor would move it past those lines and
break the certificate's interleaved sequence (§5.2). So morphe weaves such a
comment in place, and a small set records that it was, so the anchor's later
leading-run pass does not emit it a second time. It is a spent-marker on
placement the plan already fixes, not placement state of its own.

### 8.2 The emission rule, per slot

As `lower` reaches an anchor, it weaves its comments so each fact holds
on re-parse:

- **Leading.** Emitted on their own lines *directly above* the anchor,
  in order, with **no blank line inside the run and none between the run
  and the anchor** — so `next` (the anchor) is unchanged, no empty line
  breaks the run, and re-parse yields `Leading(anchor)` (syntax.md §9.2
  rule 2). A leading run is never emitted directly before a *closer*; the
  tree structure never puts one there, and were the plan to, it would be a
  dangling comment (rule 3).
- **Trailing.** Emitted on the **same line** as the anchor's last token,
  after it, separated by a space — so no line break stands between the
  anchor and the comment and re-parse yields `Trailing(anchor)` (rule 1).
- **Dangling.** Emitted within the parent's group, **before its closer** or
  **separated by a blank line** — the two shapes rule 3 admits (a comment
  before `)`/`]`/`}`/`.`, or a comment a blank line cut off from what
  precedes it). Which shape is *derived* from the comment's own position in
  the tree (whether a following sibling stands before the closer), not
  stored: the plan records only `(anchor, slot)` (§8.1).

One dangling position is emitted a third way. A comment that falls
**between a rule's neck and its body** — dangling on the rule, a blank line
detaching it from the body below it — cannot keep that blank: the house
style breaks a rule's body onto its own line below the neck (§7.2), and a
blank held there would read as a statement break *inside* a single rule. So
morphe rides the comment down with the body, on its own line directly above
it, and drops the detaching blank. On re-parse the comment is no longer
dangling on
the rule but **`Leading` on the body** — a deliberate change of slot, the
one place morphe re-classifies a comment rather than preserves its slot. It
is safe on both spine guarantees: the certificate's interleaved sequence is
unchanged (the comment still sits between the neck and the body, §5.2), and
the format is idempotent by **convergence** — a second format finds the
comment already `Leading`, emits it once at the same indent, and changes
nothing (§5.4). A blank line stranded mid-rule would be the less faithful
artifact; this is the more legible one.

The **block-aware detach** (syntax.md §9.2) is honored by the `BlankLine`
node (§6, §7.3): morphe never *introduces* a blank line inside a leading
run (which would detach its tail) and, save the neck-to-body ride just
described, never *removes* the blank an author placed to detach a comment.
Blank-line handling and comment placement are therefore one concern, not
two.

### 8.3 Why this is safe by construction

Because morphe emits every comment adjacent to the very anchor the tier
gave it, and the anchor's place in the output is fixed by the source-order
walk, a comment can never jump the token it leads or trails — the
transposition kallos recorded is unreachable. And the certificate's
interleaved sequence (§5.2) is the backstop: any placement that *did* move
a comment across a token would change the sequence and be refused before a
byte is written.

## 9. Embedded script formatting

A `#script(python) … #end.` (or `lua`) body is opaque text to themelios,
handed to morphe with an exact span (syntax.md §8.2, §17: "a formatter
that handles the region hands the text to that language's own tool").
morphe, opt-in, formats Python with **ruff** and Lua with **StyLua**.

### 9.1 The seam

One thin abstraction is the whole of what morphe's core knows about a
backend — never its name, never whether it links a library or (in a
future) shells out:

```rust
/// One embedded language's formatter. Total; bounded; produces formatted
/// text or a typed reason it could not.
pub trait EmbeddedFormatter {
    fn language(&self) -> &LanguageKey;      // matched to the #script's declared name
    fn availability(&self) -> Availability;  // Available | Unavailable { reason }
    fn format(&self, body: &str, cx: &EmbeddedContext)
        -> Result<String, EmbeddedError>;
}

/// Assembled from configuration; the core asks it by language. Backend
/// construction — the library handle, the config translation — lives
/// here, out of the core's sight.
pub struct EmbeddedFormatters { /* LanguageKey -> Box<dyn EmbeddedFormatter> */ }
impl EmbeddedFormatters {
    pub fn format(&self, language: &str, body: &str, cx: &EmbeddedContext)
        -> Result<String, EmbeddedError>;    // Err(Unsupported) when nothing claims it
}

/// Typed reasons — the "typed errors for the unknowns" posture (§12).
pub enum EmbeddedError {
    Unsupported,                    // no backend for this language
    Unavailable { reason: String }, // backend disabled/absent
    Failed { message: String },     // the backend rejected the body (its own syntax error)
    Unverified,                     // the backend's own round-trip check failed
    Panicked,                       // caught at the boundary (§9.5)
}
```

`EmbeddedContext` carries only what a backend needs beyond the source:
morphe's `line_width`, passed through so the whole file shares one width. It
does **not** carry a base indentation — a `#script` is a top-level
`Statement` (grammar §4.8), so its body sits at column 0 and morphe emits
the reflowed body there (Python's own indentation is content it must not
touch, §7.2); a base-indent field would be idle in v1, so it is omitted (a
nested-script dialect would reserve it). It also does **not** unify the
backends' configs — ruff's and StyLua's option surfaces differ, and each
backend owns and translates its own privately. The seam unifies the *verb*
(format this string), not the *knobs*.

### 9.2 Feature-gated backends, runtime opt-in

Two orthogonal gates keep the core lean and the default safe:

- **Compile time** — the concrete `RuffLibBackend` and `StyLuaLibBackend`
  sit behind Cargo features `embedded-python` and `embedded-lua`. The
  `EmbeddedFormatter` trait and registry are always present; the backends
  and their dependency closures (`ruff_python_formatter` and its
  `ruff_*` cluster; StyLua and `full_moon`) are compiled only when their
  feature is on. A consumer wanting only ASP layout (a lean LSP) pays
  neither closure.
- **Run time** — `format_embedded` is off by default; even with backends
  compiled in, no `#script` body is reformatted unless the caller asks
  (the CLI's `--format-embedded`). The default is verbatim.

### 9.3 The composed safety, and the certificate boundary

Reformatting a script body changes its `ScriptBody::value()`, which the
certificate compares (syntax.md §11.1) — so an embedded change is, by
construction, **outside** `LayoutOnly`/`UpToSpelling` for the ASP program,
and `equivalent` would (correctly) refuse it. Embedded formatting is
therefore a **second, composed transformation with a delegated guarantee**:

1. The **ASP skeleton** is certified `LayoutOnly`/`UpToSpelling` with every
   body held verbatim (§5.1 step 4) — the tier's certificate, unchanged.
   This holds *regardless of what a backend does*.
2. **Each script interior** is delegated to the backend's own
   meaning-preservation (StyLua's `OutputVerification::Full` reparse;
   ruff's stability, plus morphe's own reparse/idempotence check on the
   body). morphe accepts this as a client (§12); it does not re-derive
   Python or Lua semantics.
3. **morphe's own check** binds the two: after the embedded pass, morphe
   re-parses `final_text` and verifies that `non_whitespace_tokens` of the
   skeleton and of the final text are **identical except at `SCRIPT_BODY`
   positions** — same kinds, same order, only `SCRIPT_BODY` content
   differing. Any other difference means a backend's output disturbed the
   ASP structure (e.g. re-lexed a script terminator early); that body
   reverts to verbatim and a diagnostic is recorded (§9.4).

This composition — expressed entirely on the *public* surface
(`non_whitespace_tokens` plus the certificate) — is itself a checkpoint
probe: whether the tier makes "equal except at script bodies" ergonomic,
or morphe ends up wishing for a `SCRIPT_BODY`-excluding projection, is a
§14 finding.

### 9.4 Graceful degradation: verbatim, note, continue

**Every** way a backend can fail collapses to one outcome — *that body
stays verbatim, a diagnostic is recorded, and nothing short-circuits.* The
taxonomy (§9.1's `EmbeddedError`) all lands there: `Unsupported`,
`Unavailable`, `Failed`, `Unverified`, `Panicked`. "Continue on" holds at
both granularities: within a file, one failed `#script` never stops the
other scripts *or* the ASP skeleton from formatting; across a CLI batch,
one file's script failure never aborts the run. The failure path
**converges with the safe default** — a body left verbatim on failure is
byte-identical to what `format_embedded`-off would have produced — so a
failure degrades to the baseline, never to something worse, and
idempotence still holds (a body that fails deterministically fails the
same way on re-run). Embedded outcomes are warning-level: they never flip
the CLI exit code (§10.3).

### 9.5 Panic containment

A linked backend is not process-isolated, so a panic inside ruff or StyLua
would otherwise take down the whole run. Each library backend wraps its
call in `std::panic::catch_unwind`, converting a panic into
`EmbeddedError::Panicked` → verbatim + note (§9.4), contained to the
backend module and invisible to morphe's core. This assumes the unwinding
panic strategy — morphe's profiles keep `panic = "unwind"`; under `panic =
"abort"` `catch_unwind` catches nothing, and the subprocess seam (§16) is
the abort-profile fallback. It is 100% safe Rust; the wrapping needs no
`unsafe` (§12). A true *hang* (no timeout on an in-process call) is the one
residual an in-process integration cannot absorb; it is not reachable on
real script bodies, and the escape hatch, if it ever is, is a watchdog or
an opt-in subprocess backend *behind the same seam*, with no core change.

### 9.6 The backends, concretely

- **Python → `RuffLibBackend`** (`embedded-python`): calls
  `ruff_python_formatter::format_module_source(body, PyFormatOptions)`,
  pinned at an exact `=0.0.x` (ruff's formatter is a published but
  explicitly internal, no-semver-guarantee crate). The pin is upgraded
  only on compelling need; because `RuffLibBackend` is the *only* module
  that names any `ruff_*` type, a breaking change across an upgrade
  touches one file and never the core.
- **Lua → `StyLuaLibBackend`** (`embedded-lua`): calls
  `stylua_lib::format_code(body, config, None, OutputVerification::Full)`
  — a published, documented 2.x library API whose `OutputVerification::Full`
  reparse *is* the delegated guarantee of §9.3 step 2.

## 10. The CLI

`morphe-cli` is the thin binary; it should read like a first-class member
of its class (`black`/`ruff`/`rustfmt`/`gofmt`): the expected options,
sane defaults, good errors, CI- and editor-friendly.

### 10.1 Options

- **Modes.** `morphe <paths…>` formats files and directories in place
  (recursive; write-in-place the default, as `black`/`ruff`/`rustfmt`).
  In-place writes are **atomic** — write a temp file beside the target and
  rename over it — so an interrupted run never truncates a source: the
  certificate makes the *content* safe (§5.2), atomic replacement makes the
  *write* safe. `morphe -` / piped stdin formats stdin→stdout, never in
  place. `--check` writes nothing and signals whether anything would
  change. `--diff` writes nothing and prints a unified diff (combinable
  with `--check`).
- **Input selection.** `--stdin-filename <path>` (the assumed path for
  stdin — config, dialect, diagnostics; the editor hook).
  `--exclude <glob>` / `--extend-exclude <glob>`; default match `*.lp`.
  gitignore-aware directory walking (via `ignore`), `--no-respect-gitignore`
  to override.
- **Style & behavior.** `--line-width <n>` (default `DEFAULT_LINE_WIDTH`).
  `--dialect <clingo|asp-core-2>` (default clingo; both dialects share the
  `.lp` extension, so nothing disambiguates by extension — the default holds
  unless set). `--preserve-spellings`. `--format-embedded` /
  `--no-format-embedded`. `--reorder` — apply the opt-in conservative
  signature reorder (§15; off by default; it holds order-sensitive
  statements and `#include` as barriers and organizes within the regions
  they divide, §15.4). `--reorder-includes` — the **unsafe** toggle that
  hoists `#include` too (the caller asserts their includes are pure, §15.4).
  `--config <path>` and `--isolated`; config discovered as the nearest
  `morphe.toml` walking up from each file.
- **Output & UX.** `-q/--quiet`, `-v/--verbose`; `--color
  <auto|always|never>` (TTY-auto); a summary line (`N reformatted, M
  unchanged`, or in `--check` `N would be reformatted`) and the list of
  changed files.
- **Meta.** `-h/--help`, `-V/--version`.

### 10.2 Config file

A dedicated `morphe.toml` (there is no ASP-world `pyproject.toml`
convention to host a table), discovered by walking up from each formatted
file, overridden by `--config`, ignored under `--isolated`. Its keys are
the style/behavior options above; unknown keys are a usage error, not
silently dropped.

**Safe/unsafe toggles.** Style stays black-class opinionated — `line_width`
is the one layout knob, indent and the rest fixed (§16). What the config
*does* grow is a **safe/unsafe** section, `ruff`'s applicability model at
config scope: an option is **safe** (a morphe-*proved* transform) or
**unsafe** (one the caller *asserts* sound, morphe verifying only out of
band). The first unsafe toggle is `reorder_includes` (§15.4) — hoisting
`#include`, which morphe cannot prove because it never resolves an include,
so the caller vouches for the "domain-encoding library" discipline. Future
lint autofixes (§11.2) join this section under the same split. The toggles
are *orthogonal to style* — they select which sound transforms run, never
how code lays out — so they do not spend the one-way stance.

### 10.3 The outcome contract

Exit statuses are a **typed enum**, the integers confined to the
`Termination` conversion and named nowhere else (§10.4):

```rust
/// morphe's process outcome. The single place a raw code is produced is
/// the Termination impl.
pub enum Outcome {
    Clean,          // formatted (or --check found nothing to change)
    ChangesNeeded,  // --check / --diff: input is not already in morphe-form
    UserError,      // a parse refusal (§5.7), a usage or I/O error
    InternalError,  // the never-should-happen CertificateBroke — a morphe bug, loud
}
```

`CertificateBroke` earns its own outcome rather than folding into a
generic error, because the name should tell the user "this is our bug, not
your file," and it prints a bug-report ask; a broken permutation
certificate (§15.3) is the same class of morphe bug and maps to
`InternalError` likewise. Embedded-script failures are warning-level and
never change the outcome (§9.4). The reorder does not skip a member — order
sensitivity is handled by barriers (§15.4), not a whole-program skip — so it
adds no warning-level outcome of its own beyond those. Parse-error refusals
render the tier's diagnostics through base's human view, at the rust-analyzer
bar (spec §2).

### 10.4 No magic numbers

Every semantically-loaded literal — the exit codes above,
`DEFAULT_LINE_WIDTH`, the document engine's `INDENT`, any threshold — is a
documented named constant, so the code reads as its meaning (the estate
legibility standard; spec §5.2). No bare number carries meaning in the
design or the build.

## 11. Diagnostics, and the deferred lint seam

### 11.1 morphe's diagnostics

morphe emits diagnostics of its own — a parse-error refusal (§5.7), an
embedded skip (§9.4), a `CertificateBroke` (§10.3) — as **base
`Diagnostic` values lowered through base's human view**, the same channel
and quality bar the tier uses (syntax.md §7.3, spec §2 item 9). A
parse-error refusal simply forwards the tier's own `SyntaxError`s
unchanged: morphe adds no parse diagnostics of its own, because parse-error
diagnosis is the tier's, not the formatter's — the ownership line, that the
tier owns what a formatter must *know* while morphe decides only style
(§3.2; syntax.md §15).

### 11.2 The deferred lint face

The north star is `ruff`-class: a formatter *and* a linter over the same
lossless parse. v1 ships the formatter and **designs the lint seam without
filling it** (§1). The seam is cheap to leave open precisely because the
parse is lossless and shared: a lint is a rule over the typed AST producing
a `Diagnostic` with an optional applicability-tagged fix (the `ruff` shape),
and v1's clean typed-AST walk and diagnostic channel are exactly its
substrate. What v1 owes the seam is only *not to foreclose it*: the AST walk
stays reusable and the diagnostic channel stays uniform. When built, the
lint face lands as the **`morphe-rewrite` crate** (§4.1, §16) — a lint is
the *suggest-mode* of the same guarded rewrites whose *apply-mode* is a
transform (§16): the linter detects an available rewrite and offers it as a
fix; `--fix`, or the corresponding transform, applies it.

That framing settles which lints come *first*, and it turns on soundness.
The lint face's **soundest** first tenants are **optimization-availability
lints whose precondition is syntactically decidable** — "an ngo-class
rewrite applies here" (§16) — because each such rewrite is a *proven*
theorem, and where its precondition can be *checked on the AST* the
suggestion is sound, not a guess, and its fix is the `optimize` transform.
(An ngo precondition that itself needs whole-program or semantic analysis —
"this variable is unused," "this rule is projectable" — is no more decidable
than a usage lint; those ride the same accepted-and-verified guarantee as
`optimize`, §16, not a static proof.) **Signature/usage lints** — an unused
`#const`, a `#show`n atom
never derived, a `#defined`/`#external` never consumed — read a program's
declared interface (input `#defined`/`#external`/`#const`, output `#show`,
modules `#include`/`#program`), largely the signature the reorder organizes
(§15). But ASP in the clingo dialect is **untyped and open** (modular via
`#include`/`#program`, data-parameterized, `#script`-extensible), so these
are **non-local and unsound statically** — heuristics ("likely unused"),
not proofs. So the sound lints ride the optimizer's proven rewrites; the
usage heuristics are the harder, honesty-tagged kind — a further reason the
lint face is deferred and demonstrated-need-driven (§14.3).

### 11.3 Lints versus the tier's diagnostic/Hint pass

Two diagnostic bodies of work must not be conflated (the carried sequencing
question; syntax.md §7.1):

- the tier's **diagnostic/`Hint` pass** hardens *parse-error* diagnostics
  on **non-members** — "why this isn't valid ASP" — and lands in
  themelios-syntax (a change to its closed `Hint` enum);
- morphe's future **lint face** emits *style/smell* lints on **valid
  members** and lives in morphe's own namespace.

They share the base `Diagnostic` model and the typed AST, and there is a
feeder relationship (syntax.md §7.1 admits a `Hint` "when a golden shows a
reader needs it"). v1's decision (§1, §14.3): the formatter goes first, and
its real findings *drive* how much of the tier's diagnostic/Hint pass is
warranted — checkpoint-driven, not front-loaded.

## 12. Trust and dependency posture

morphe is a **leaf client**, not a foundation: nothing hardens on it as a
trust anchor, so it does not inherit themelios's *foundational*
obligations (a committed hostile-input threat model over arbitrary input;
a minimal, audited dependency closure). It does hold **everything it
authors** to the estate's safety bar, and it *contains* what it cannot
control.

- **`forbid(unsafe_code)` crate-wide**, backends included. This is free: a
  formatter over safe APIs (the tier's surface; ruff's and StyLua's safe
  library fns) has no genuine need for `unsafe`, and even the containment
  of §9.5 is 100% safe Rust. The "compelling reason" escape is theoretical;
  none is foreseen.
- **Guaranteed vs accepted, stated plainly:**
  - *Guaranteed — verified, trusts nothing:* the ASP skeleton is
    `LayoutOnly`/`UpToSpelling` (§5.2) plus the equal-except-script-bodies
    check (§9.3). A bug in ruff or StyLua can never silently corrupt the
    **ASP program** — only a script interior, and the structural check
    still bounds it to that interior.
  - *Accepted — delegated:* each script interior's meaning-preservation,
    handed to the backend's own guarantee (§9.3). morphe accepts ruff and
    StyLua as clients — treats them as untrusted behind the hardened
    `EmbeddedFormatter` boundary, trusting no output until its own checks
    pass — rather than auditing their internals or their closures to
    themelios's bar. This acceptance is the narrow, stated exception; it
    is proportionate, because morphe formats the user's *own* script code
    with tools they already trust for it, and is not a hostile-input
    boundary the way the tier's parser must be.
- **The safety/quality bar morphe *does* adopt** (the instruments in §13):
  totality — morphe's own code never panics, formatting is total, refusals
  are typed; depth safety (§5.6); and the property/fuzz/differential
  instruments a formatter warrants.

The `ruff_*` and `full_moon` closures enter only under their features
(§9.2); their weight and their internals are accepted, not audited — and
documented as such, not hand-waved.

**Escalated to the security gate.** Whether this leaf-client scoping is
*adequate* — a hostile `#script` body crafted against a linked ruff/StyLua,
and the in-process hang residual (§9.5) in CLI batch mode — is a
threat-model question this design records but does not rule; it is carried
to the security review at build (spec §10), where morphe's committed threat
model is written.

## 13. Assurance instruments

Per spec §10.1's language-first discipline, morphe is not done until its
instruments are green; each is documented with what it proves and what it
cannot (spec §10.2). The build follows the estate's **test-driven
discipline**: the property or golden that pins a behaviour is written
*before* the code that satisfies it — red, then green, one behaviour at a
time. **`proptest` and `criterion` are standing
dev-dependencies from week one**, as every estate tier carries them (themelios
`Cargo.toml`): `proptest` drives the laws below (idempotence, the certificate
grant, comment-fact preservation, the permutation certificate), `criterion`
the scaling shapes; the fuzz crate stands from the first weeks likewise (spec
§10.1).

- **The certificate, always on** (§5.3): every `format` proves
  `LayoutOnly`/`UpToSpelling` before returning `Ok`. A shipped invariant,
  not a test.
- **Idempotence:** `format(format(x)) == format(x)`, checked over **every**
  corpus member (§5.4) — the property kallos's comment-reclassification broke;
  generated members are covered by the fuzz target (below), and the one
  convergence-not-preservation edge (the neck-to-body ride, §8.2) carries its
  own deterministic fixed-point test, so it is not the fuzz target's to sample
  alone.
- **Certificate reflexivity and grant:** for every corpus member, `format`
  returns `Ok` with the expected certificate, and `equivalent(input, output,
  certificate)` holds — no member is ever refused for a `CertificateBroke`.
- **Comment placement (proptest):** read through the tier's public `attachments`
  (syntax.md §9.2's law, from the consumer side), placement is a function of
  *structure*, not of the input — every comment survives with its content, and
  its slot follows the program's shape, not the whitespace the author used. Drop,
  add, and transposition the always-on certificate refuses (§5.2); the slot- and
  anchor-drift it cannot see is what this law adds. Framed on the **output**, not
  the input, it needs no corpus property to stay true: it holds for every member,
  the one shape the format deliberately re-slots among them — the neck-to-body
  ride (§8.2), whose slot converges to a **stable** `(anchor, slot)` rather than
  keeping the input's, pinned by its own deterministic fixed-point test.
- **The reorder transform (§15):** the **permutation certificate** is
  always on — every reorder proves the per-unit certificate, the bijection,
  and every barrier held before writing (§15.3), never returning a reorder it
  did not prove. Property tests: `--reorder` is idempotent and its permutation
  certificate holds over the corpus; each comment travels with its unit (a
  moved statement's Leading/Trailing trivia and owned docs move with it; a
  top-level Dangling comment and every order-sensitive statement — a barrier,
  §15.4 — hold their place, reordering never crossing one); and **`section`'s
  codomain is the set of sections `ORDER` holds** (the codomain law, §15.2), a
  test-time exhaustiveness check so no unit is silently dropped. The clingo differential
  (below) grounds a reordered member to the same ground-rule set — the
  backstop for the unsafe `reorder_includes` case where the includes resolve.
- **The fuzz target** (from week one, spec §10.1): arbitrary text under
  both dialects through `format` — no panic; a member always yields `Ok`
  with a granted certificate; a non-member always yields
  `HasParseErrors`; the output re-parses to a member; idempotence holds.
- **The corpus** (spec §10.3), vendored with provenance: kallos's
  clingofmt-derived inputs (MIT, inputs-only, attributed), the themelios
  syntax corpus, and morphe's own adversarial cases — the comment-torture
  rule that surfaced kallos's transposition foremost.
- **The clingo differential** (out of band, feature-gated, clingo the
  authority — grammar §3): a formatted member is cross-checked against its
  input by parsing **both** in clingo and comparing clingo's *own* canonical
  rendering of each parse (the text clingo prints from its AST) — they must
  be identical, since layout and synonym spelling cannot change what clingo
  parses. For a **reorder** (§15), whose canonical rendering differs *by
  order*, the differential instead grounds both programs and compares the two
  **ground-rule sets** for equality (clingo `--text`, canonicalized) — a
  *proxy* for answer-set preservation under a pure permutation, not an
  answer-set-equivalence decision (spec §7.1: structural equality is never
  presented as equivalence). It is a **backstop**, not the shipped guarantee,
  and it cannot backstop the unsafe `reorder_includes` case on ordinary
  corpus files: grounding a program with `#include` requires resolving the
  includes a self-contained corpus does not carry, so that case rests on the
  caller's assertion, checked only where the includes are present to ground
  (§15.4). A meaning-preservation witness *beyond* the token certificates,
  the exact clingo invocation pinned (executable-claims, spec §10.4).
- **Golden snapshots**, reviewed: the formatted output of each house-style
  rule (§7) and each comment-placement case (§8), and the diagnostics
  corpus (§11) rendered through base's human view.
- **Embedded round-trip** (feature-gated): for each backend, a script body
  formats, re-formats identically (idempotent), and the
  equal-except-script-bodies check holds; every `EmbeddedError` degrades
  to verbatim + note (§9.4).
- **Depth gate** (§5.6): a program nested to `DEFAULT` formats on an
  ordinary thread without overflow, and input nested past `DEFAULT` is
  refused as `HasParseErrors` (the `format` path); `format_parsed` on a
  `DEFAULT`-built `Parse` formats on an ordinary thread likewise, the
  two-tree hold-and-drop included. morphe uses the baked-in `parse` door
  throughout, so no `with_required_stack` / 64 MiB thread is ever owed.
- **Scaling shapes (criterion):** `format` linear in input; the document
  render linear in the tree; the certificate linear in both texts
  (inherited). Shape assertions in the gate; absolute numbers out of band.
- **Standing gates:** the estate lint floor (§4.1), documentation examples
  that run, and the executable-claims standard for anything morphe says
  about itself (spec §10.4).

## 14. The checkpoint: routing findings back

morphe exists to reveal themelios-syntax's ergonomics; this section is how
what it reveals returns to the tier.

### 14.1 The ledger

morphe keeps a **findings ledger** in its repository (`docs/findings.md`),
one entry per friction point met while building against the surface, each
tagged by syntax.md §15's split (§3.2): **stable** or **free**. An entry
records the surface item, what morphe wanted, what it got, and the cost of
the gap.

### 14.2 Routing

- A **free**-item finding morphe resolves itself (adapting) or proposes as
  a convenience tweak; it needs no tier ruling.
- A **stable**-item finding is the checkpoint firing at the tier's design.
  It is carried to themelios-syntax and resolved there — syntax.md
  reopens to eat it (spec §11) — never worked around silently in morphe
  and never patched into the tier speculatively mid-build.

### 14.3 The diagnostic/Hint sequencing, resolved

The carried question (§11.3) is answered by v1's formatter-first scope: the
tier's diagnostic/`Hint` pass is **checkpoint-driven**. morphe surfaces,
through real use, where a reader of a diagnostic needed more — and *those*
demonstrated needs (syntax.md §7.1's "when a golden shows a reader needs
it") drive the pass, rather than a speculative `Hint` vocabulary built
ahead of a consumer. A thin proactive floor is warranted only where the
formatter itself already needs a hint; everything else waits for the
finding.

## 15. Transformations

Beyond the always-on formatter (§5–§8), morphe offers **opt-in,
order-changing transformations** — the ASP analogue of `rustfmt`'s import
reordering, distinct from both the certified formatter (which never changes
order) and the deferred lint face (§11.2). v1 ships exactly one: the
**conservative signature reorder**, off by default and requested explicitly
(`--reorder` / config, §10).

### 15.1 The unit, and the barrier

A transformation moves whole **units**. A unit is a top-level statement
together with everything the tier binds to it: its owned `%!` documentation
(in docs position, structure it owns — syntax.md §5.4, §8) and its Leading
and Trailing attached trivia (§8). Because attachment is a total reading of
a lossless tree (§8; syntax.md §9), "move the statement" carries "move its
comments" for free — the capability a side-table parser cannot offer, and a
reason the estate's lossless tree exists (spec §5.1).

A **barrier** is a top-level element that is *not* reordered and that
reordering never crosses; the reorder permutes units only *within* the
regions barriers divide. The barriers are a **top-level Dangling comment**
(§8 — floating with no owner at program level, the block-aware detach; "a
floating comment stays where it is") and every **order-sensitive top-level
statement** (§15.4). The generating principle, stated once so the barrier
set is *checkable for completeness* rather than enumerated by folklore: **a
top-level element is a barrier iff its position can affect the program's
answer sets — including through content morphe cannot see.** (A Dangling
comment *interior* to a statement — one §8.2 emits before a `)`/`]`/`}`
closer — is part of that unit's text and moves *with* the unit; only
program-level elements partition the unit stream.)

### 15.2 The reorder, as a bucket-emit

The reorder is a **stable bucket-sort driven by data**, not a comparator.
Each reorderable unit has a **section** — a total map from its statement's
kind to a **`Section`** the active `ORDER` lists — and `ORDER` is a data list;
the transform emits each section in that order, its units in source order:

```text
reorder(program, opts) =
    program
    |> units_and_barriers                        // reorderable [Unit]; barriers (§15.1, §15.4)
    |> split_on(barriers)                        // the regions the barriers divide
    |> map(region ->                             // within a region only:
         ORDER.flat_map(section ->               //   emit each section in ORDER,
           region.filter(u -> u.section == section)))  //   units in source order — stable, no comparator
    |> weave_back(barriers)                      // re-insert every barrier at its held position
    |> map(render_unit)                          // format each unit with the certified formatter (§5–§8)
    |> assemble(house_spacing)                   // §7's blank-line and one-statement-per-line rules
    // ORDER = [Defined, Const, Body, Show] by default (#include is a barrier, §15.4);
    //         [Include, Defined, Const, Body, Show] under opts.reorder_includes (§15.4)
```

The default `ORDER` realizes the signature reading (§11.2): **`#defined`**
(the input signature) → **`#const`** (the parameters, non-override only) →
**Body** (rules and every other order-insensitive statement, in source
order) → **`#show`** (the output signature); **`#include` is a barrier**,
held in place (§15.4), unless `opts.reorder_includes` hoists it to a leading
`Include` bucket. `section` returns a **`Section`** — a bucket the active
`ORDER` lists, not a position in it — and the **codomain law** (§13) holds it
to `ORDER`: the codomain of `section` over the reorderable kinds *is* the set of
sections `ORDER` holds, so no unit maps to a bucket outside `ORDER` and is
silently dropped from the emit. A named `Section` rather than a bare index is
what lets `ORDER` stay a reorderable table a later release makes configurable
(§16) without touching the mechanism — a position would not survive the table
being reordered — and lets the emit stay total: a section `ORDER` omits is
appended after its buckets, never dropped (the law forbids that state; the emit
does not rest on it). morphe works in **text**, not the tree: each
unit carries its certified formatted text and the transform concatenates
texts — it never mutates the rowan tree (tree editing is the tier's reserved
seam, syntax.md §17; text is the edit medium, spec §6.8). An author's blank
line *between* two units is unattached whitespace, not trivia any unit owns
(§15.1), so it does not travel a move; `assemble` normalizes the inter-unit
spacing to §7's rules — at most one blank — rather than preserving the
input's. Trivia a unit *does* own — its leading and trailing comments and its
documentation (§15.1) — travels with it.

The reorder is a **distinct library operation** (`reorder`, §4.2), not a
`format` flag, so `format`'s certificate contract (§5.3) stays pure —
`format` never changes order. The CLI's `--reorder` invokes it; it takes the
`FormatOptions` (for the per-unit formatting) plus the reorder toggles
(`reorder_includes`, §10.2), and returns the reordered text with the
certificates it proved (§15.3).

### 15.3 The safety model, as a checkable algorithm

A reorder changes the interleaved sequence, so the layout certificate (§5.2)
*correctly refuses the whole program* — that is the point. The reorder earns
its own guarantee, which factors into three checks. The two **syntactic**
checks are *proofs*: on a break the transform **refuses, leaving the input
untouched, as a morphe bug** (§5.3's discipline, one layer up). The
**semantic** layer is not a whole-program precondition in v1 but a
per-statement *classification* (§15.4): each order-sensitive statement is a
**barrier** (held, never crossed), so the reorder always applies —
organizing the reorderable units within the regions barriers divide — and a
program that is all barriers moves nothing (`reordered = false`). There is
no whole-program skip; the only refusals are a non-member and the
never-should-happen certificate break.

1. **Per-unit layout, discharged by `format` itself.** Each unit's text is
   formatted through `format` (§5.1), whose always-on step-4 certificate
   *is* the per-unit `LayoutOnly`/`UpToSpelling` proof — `equivalent` is
   invoked there, on the unit's one-statement `Parse`, not on a bare cursor
   (which it cannot take, syntax.md §11.2). A per-unit break surfaces as that
   unit's `CertificateBroke`, carried out as `PermutationBroke` (§4.2).
2. **Permutation — a *structural* check, not a second equivalence run.**
   With every unit already layout-certified by (1), the permutation check
   establishes only what (1) does not: that the whole differs from the input
   *only by order*. morphe re-parses the assembled output — which must itself
   be a **member** (a reassembly that does not re-parse cleanly is the
   `NotAMember` bijection breach, carrying the tier's own diagnostics) —
   re-derives its top-level units and barriers, and verifies a **total
   bijection** — every input unit matches exactly one output unit *by identity of
   its non-whitespace token+comment subsequence* (syntax.md §11.1), with
   **nothing added or dropped** (a both-ways multiset count per region — a split
   or duplicated unit surfaces as an add and a drop, not a finer witness) — and
   that **every barrier holds**: each top-level Dangling comment re-parses to
   `Dangling(PROGRAM)`, not re-attached as `Leading`/`Trailing` of a
   now-adjacent unit (the four-facts hazard, syntax.md §9.2), and each
   order-sensitive barrier statement keeps its position and its neighbours on
   each side. Postcondition: *the output's non-whitespace sequence is a
   block-permutation of the input's — blocks being units, barriers being
   fixed points — covering every non-whitespace token of both trees exactly
   once.* `PermutationBroke` is the negation: a per-unit `Mismatch`, or a
   bijection breach (§4.2).
3. **Applicability — the barrier classification (§15.4),** not a
   whole-program precondition: which statements are barriers (held) and which
   are reorderable, sound by the order-sensitivity theorem.

The first two are **syntactic**, on the public §8/§11 surface, and prove
"the parts each preserve equivalence; the whole differs only by order." The
third is where answer-set preservation is argued (§15.4).

### 15.4 Applicability: order-sensitivity, derived and grounded

The reorder's answer-set safety rests on one theorem, stated with its scope
and grounded against the pinned authority (clingo, grammar §3) as the estate
requires of every semantic claim (as syntax.md §11.3 grounds the canonical
spellings): **for a self-contained program, permuting its order-insensitive
top-level statements within the regions its order-sensitive statements and
top-level comments divide preserves the answer sets.** The classification,
each side argued rather than enumerated:

- **Order-insensitive (reorderable units).** A logic program is a *set* of
  rules, so permuting rules preserves the grounding and the answer sets
  (spec §7.1, the set semantics). `#defined` and `#show` are *global
  declarations* collected regardless of position (they annotate/filter the
  output signature, not the derivation); a **plain, non-override `#const`**
  is a single global substitution, at most once per name (a second plain
  definition of a name is a clingo error — no answer sets to preserve).
  These are the units the reorder moves.
- **Order-sensitive (barriers).** `#program` sets the part context for the
  statements that *follow* it (spec §7.1: parts are "ordered where meaning
  demands order"); a `#script` executes with side effects at its position;
  `#const [override]` resolves by position; **`#include`** is a *textual
  splice* of another file at its position (syntax.md §17: "parsed and never
  resolved"), whose unseen content can carry any of the above and whose
  part-context leaks across the boundary; and the **ASP-Core-2 query** must
  be the program's *final* statement (grammar §6.1 — a `?` anywhere else is
  the bitwise-or operator; syntax.md §11's corollary flags this
  position-sensitivity), so it is pinned last. Each is held as a **barrier**
  — not moved, never crossed — which *preserves* its document-order
  relationship to every statement, so the property it governs is unchanged.

Every classifier is **syntactic** — `Statement::{ProgramPart, Script,
Include, Query}` and `ConstStatement::policy() == Some(Override)` (syntax.md
§8.2) — so the barrier set is computed without resolving anything (no
`#include` followed, no `#script` run). **Completeness, precisely.** The
reorder *moves* only the signature directives to their buckets — `#defined`
to the front, plain `#const` after, `#show` to the end — each argued
order-insensitive above; **everything else stays in source order.** So a
`Body` statement whose position *might* matter relative to another `Body`
statement — a `#theory` definition and a theory atom that uses it, say —
never has that relative order changed, because the bucket-sort is **stable**
and both share the `Body` bucket. The safety thus rests on two legs — the
moved directives are position-insensitive globals, and same-bucket order is
preserved — not on every `Body` kind being independently order-insensitive.
A non-member reassembly (a barrier mis-placed, a query no longer final) is
itself caught by the permutation certificate as a bijection breach (§15.3).
*The §16 configurable-`ORDER` seam must preserve `Body`'s source-order
stability, or it reopens this argument.*

**The `#include` toggle — safe by default, opt-in to hoist.** By default a
top-level `#include` is a barrier: sound, because morphe cannot see its
contents. `opts.reorder_includes` (§10.2) downgrades `#include` to a
reorderable `Include` section, hoisted to the top — an **unsafe** option in
`ruff`'s sense: the *caller's assertion* that their includes carry no
order-sensitive construct (the common "domain-encoding library" discipline),
not a morphe proof, with the clingo differential (§13) verifying a given run
out of band. Because that assertion can reach a run through a discovered config
the invoking user did not write (§10.2), and morphe cannot *prevent* an unsound
hoist, the transform is made **never silent**: whenever `reorder_includes`
actually hoists an `#include`, a warning names the move and states it rests on an
assertion morphe did not verify (the config-discovery posture, threat model
§3.4). It is the first tenant of the config's safe/unsafe toggle framework
(§10.2). The **general reorder** — one that discharges the unsafe
cases by *verified* answer-set equivalence rather than a caller's assertion —
is a reserved seam (§16), arriving with the solve tier or on demonstrated
demand. Because morphe cannot see included content, the `rustfmt`-import
analogy holds only for the *safe default plus a trusting toggle*: Rust `use`
is order-insensitive and always movable; ASP `#include` is a
position-significant paste, movable only on the caller's word.

### 15.5 Why this is the alignment, and the rewriting frame

The reorder puts morphe where `rustfmt` is for the ASP-meaningful case — it
organizes the program's signature the way `rustfmt` organizes imports — but
*certified and conservative*: layout and spelling by the syntactic
certificate (§5), reordering by the per-unit certificate, the permutation
certificate, and the derived-and-grounded applicability (§15.3, §15.4). That
discipline — a guarantee matched to what each rewrite changes and to ASP's
semantics — is the whole of morphe.

The **rewriting frame** generalizes it, as *motivation for the v2+
`morphe-rewrite` crate* (§16), not a v1 structural commitment: format,
reorder, and `optimize` are morphe rewriting the program's units for
different ends — a **reader** (layout, syntactic certificate), the
**organization** (order, permutation certificate), or a **solver** (an
ngo-class optimization, a delegated semantic guarantee). A rewrite could in
principle be *applied* (a transform) or *suggested* (a lint, §11.2); v1
exhibits only the apply side, and only for two deliberately *unlike*
operations (format always-on and whole-program; reorder opt-in and
per-unit), so the apply/suggest symmetry is a v2 design target, not a shape
v1 already has. §16 is the frame's home; §11.2 and here cross-reference it.

## 16. Reserved seams and non-goals

Named seams — deferred with reasons and their arriving consumers, not gaps:

- **The `morphe-rewrite` crate — the lint face and `optimize`** (§11.2,
  §15.5): designed-open, unfilled in v1; a v2+ crate over
  `themelios-syntax`'s typed AST housing a registry of **guarded rewrites**,
  each with two modes — **suggest** (a lint: a `Diagnostic` with an
  applicability-tagged fix, the ruff-class linter, in morphe's namespace) and
  **apply** (`morphe optimize`: an ngo-inspired [docs.potassco.org/ngo]
  rewrite, semantics-changing-for-a-solver but answer-set-preserving). Its
  guarantee is **accepted, not re-derived**: each rewrite is a proven theorem
  (ngo / the literature) applied under its precondition, verified per
  application out of band by the clingo differential (§13) — the
  guaranteed-vs-accepted posture of §12, as for the embedded backends. The
  ownership rests on spec §7.5's rule that a satellite's **specific
  transforms stay theirs** (the program tier ships the transformation
  *machinery*; the rules are the consumer's) — not on "AST-level," which is
  true only for the *syntactic* rewrite subset. A semantic ngo rewrite
  (projection, inlining) reasons over the Program value's structure (spec
  §7.1), genuinely program-tier facts; `morphe-rewrite` builds over the
  syntax AST because ngo is natively AST→AST, forgoing the program tier's
  provenance (spec §7.4) until it *leans on* §7.5 — the tradeoff named, not
  assumed. Its sound first tenants are the syntactically-decidable
  optimization-availability lints (§11.2).
- **Formatting around parse errors** (§5.7): v1 refuses a non-member;
  member-region formatting arrives when an editor consumer demonstrates the
  need, and must reckon with recovery shapes (syntax.md §15, *free to
  move*).
- **Range formatting** (`--range`, editor "format selection"): deferred —
  a partial reflow certified over a sub-range is genuinely hard and not
  needed to reveal the tier's ergonomics.
- **A `CEILING`-aware `format_parsed`** (§5.6): v1 uses the baked-in `parse`
  door (`DEFAULT`) throughout and requires `format_parsed`'s `Parse` to have
  been built the same way; supporting a consumer that parsed at `CEILING` —
  re-parsing the skeleton at the input's own depth, on a
  `with_required_stack` thread — arrives if a deep-parse consumer ever
  demonstrates the need, and would want the `Parse::nesting_limit()` accessor
  the tier does not expose.
- **A subprocess embedded backend / a formatter watchdog** (§9.5): behind
  the same seam, if a hang or an environment without the linked backends
  ever demands it.
- **The general (semantic-verified) reorder** (§15.4): v1 holds every
  order-sensitive statement and `#include` as a barrier, organizing within
  the regions they divide; the only escape is the caller-asserted
  `reorder_includes` toggle. A reorder that *moves across* barriers —
  hoisting includes, or reordering across `#program`/`#script`/override-`#const`
  — by *verifying* answer-set equivalence rather than trusting a caller's
  assertion is deferred to the solve tier or demonstrated demand.
- **Configurability beyond `line_width`** (§1, §7), including the reorder's
  canonical `ORDER` (§15.2): the black-class one knob is v1; rustfmt-class
  options — and a user-defined section order — accrete only toward the north
  star and on demonstrated demand.
- **A library backend for ruff** in place of the pinned internal crate
  (§9.6): a drop-in behind the seam if Astral ever publishes a stable
  formatter API.

Non-goals, absolutely: styled or semantic re-writing of a program beyond
the opt-in reorder (§15) — morphe changes layout and synonym spelling
always, and top-level *order* only under the opt-in, certified reorder,
nothing else; a language server; a REPL; evaluation, admission, or any
semantics of ASP (those are other tiers'); parsing or running an embedded
script itself (morphe hands it to that language's tool and never interprets
it); and any spacing, comment, or equivalence logic of morphe's own that
duplicates what the tier owns.
