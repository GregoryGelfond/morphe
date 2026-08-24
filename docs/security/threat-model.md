# morphe threat model

The security posture of record for morphe: what it guarantees, what it accepts,
and the residuals it carries. morphe's design of record is
`docs/design/morphe.md`; this document commits the threat model that §12 (trust
and dependency posture) and §9 (embedded script formatting) record but, there,
deliberately do not rule. It is the reference morphe's security review answers to — chiefly for the
embedded surface (the seam that wires embedded formatting and the concrete
ruff/StyLua backends that populate it), and for the one opt-in that steps
outside a morphe proof, `reorder_includes` (§3.4).

## 1. Posture: a leaf client, not a trust anchor

morphe is a **leaf client** of themelios-syntax, not a foundation (§12). Nothing
hardens on morphe as a trust anchor: no component derives a security guarantee
from morphe's output that morphe is obligated to keep sound against an adversary.
It therefore does **not** inherit themelios's *foundational* obligations — a
committed hostile-input threat model over arbitrary input, and a minimal, audited
dependency closure — which the tier carries because the whole estate stands on
it.

What morphe *does* hold:

- **Everything it authors**, to the estate's safety bar: `#![forbid(unsafe_code)]`
  crate-wide, totality (its own code never panics; formatting is total; refusals
  are typed), depth safety (§5.6), and the property/fuzz/differential instruments
  a formatter warrants (§13).
- **Containment** of what it does not control — the embedded backends — behind a
  hardened seam (§9.1, §9.5).

morphe formats a user's *own* ASP source under the user's *own* configuration. It
is not a network service, not a multi-tenant boundary, and not an execution
engine: it never runs, grounds, or solves a program, and it never runs an
embedded script — it lays text out and proves it did not change meaning.

## 2. Assets

1. **The meaning of the ASP program.** The paramount asset: a formatter that
   silently changed what a program computes would be worse than useless. morphe's
   central guarantee protects it (§3).
2. **The user's source files.** A formatter writes in place (§10.1); an
   interrupted or faulty write must never truncate or destroy a source. morphe's
   refuse-to-write-uncertified stance (§5.3) makes the *content* safe, and its
   designed atomic write — a temp file renamed over the target (§10.1) — makes the
   *write* safe.
3. **The running process.** A CLI formatting a batch must not be brought down — by
   a crash or an unbounded hang — by one hostile or pathological file (§9.4,
   §9.5).

## 3. The central guarantee, and its boundary

### 3.1 The ASP program: guaranteed, verified, trusts nothing

For a member input, morphe proves — *before returning any output* — that its
output stands in the `LayoutOnly` or `UpToSpelling` relation to the input (§5.2):
the same significant tokens and trivia comments, in the same order; nothing but
whitespace (and, under `UpToSpelling`, synonym spellings) changed. The proof is
the tier's token-stream-equivalence certificate, re-derived by re-parsing morphe's
own candidate output (§5.3). morphe **cannot return `Ok` text it did not
certify**; a broken certificate is a morphe bug, surfaced loudly (§10.3), never a
silent corruption. This guarantee trusts nothing — not the lowering, not the
renderer, not any backend. It is *checked*, not asserted (§12).

### 3.2 The one untrusted boundary: the embedded surface

Reformatting a `#script(python|lua)` body changes that body's text, which is
*inside* the certificate's comparison (syntax.md §11.1) — so an embedded change
is, by construction, outside `LayoutOnly`/`UpToSpelling` for the ASP program, and
the certificate alone would (correctly) refuse it. Embedded formatting is
therefore a **second, composed transformation with a delegated guarantee** (§9.3),
and the linked backends (ruff for Python, StyLua for Lua) are the one component
morphe treats as **untrusted** and **contains** rather than audits.

The containment has four independent layers, each holding whatever the backend
does:

1. **The ASP skeleton is certified with every body held verbatim** (§5.1 step 4).
   This certificate stands *before* any backend runs and *regardless of what a
   backend does*. A bug — or an outright hostile output — in ruff or StyLua can
   never reach the ASP program through this path.
2. **morphe's structural bind: the equal-except-`SCRIPT_BODY` check** (§9.3 step
   3). After the embedded pass, morphe re-parses the whole text and confirms its
   non-whitespace token sequence is the certified skeleton's exactly, save that a
   `SCRIPT_BODY` token's *content* may differ. Any other difference — a backend's
   output that re-lexed a script terminator early, injected an ASP statement, or
   dissolved the body token — is caught, and that body reverts to verbatim (§9.4).
   A backend's output is thus **confined to the interior of its own script**; it
   cannot alter one ASP token.
3. **Panic containment** (§9.5). A linked backend is in-process, so a panic would
   otherwise take down the run. Each backend wraps its call in
   `std::panic::catch_unwind`, converting a panic into `EmbeddedError::Panicked` →
   verbatim + note. It is 100% safe Rust; the containment needs no `unsafe` (§12).
   The `catch_unwind` lives in the backend modules; the seam treats a returned
   `Panicked` like any other failure.
4. **Graceful degradation** (§9.4). *Every* way a backend can fail — unsupported,
   unavailable, rejected, unverified, panicked, or structurally disturbing —
   collapses to one outcome: that body stays verbatim, a warning is recorded, and
   nothing short-circuits. A failure converges with the safe default
   (`format_embedded` off), never to something worse. Embedded outcomes are
   warning-level; they never flip the exit code (§10.3).

### 3.3 Guaranteed vs accepted, stated plainly

- **Guaranteed — verified, trusts nothing:** the ASP program is
  `LayoutOnly`/`UpToSpelling` (§5.2) plus the equal-except-`SCRIPT_BODY` bind
  (§9.3). A bug in ruff or StyLua can never silently corrupt the **ASP program** —
  only a script interior, and the structural check bounds even that to the
  interior.
- **Accepted — delegated:** each script *interior*'s meaning-preservation is
  handed to the backend's own guarantee (StyLua's `OutputVerification::Full`
  reparse; ruff's stability, plus morphe's own reparse/idempotence check on the
  body, §9.3 step 2). morphe accepts ruff and StyLua *as clients* — untrusted
  behind the seam, trusting no output until its own checks pass — rather than
  auditing their internals or their `ruff_*`/`full_moon` closures to the tier's
  bar. This is the narrow, stated exception, and it is proportionate: morphe
  formats the user's *own* script code with tools they already trust for it, and
  the embedded surface is not a hostile-input boundary the way the tier's parser
  must be.

### 3.4 The `reorder_includes` boundary: an accepted transform of the ASP program

Every default operation preserves the paramount asset (§2.1) by a *checked*
proof: `format` and `reorder` (with `#include` a barrier) stand in
`LayoutOnly`/`UpToSpelling` or a proven permutation of it (§5.2,
§15.3), and the embedded surface is bounded to a script interior and cannot
reach the ASP program (§3.2). The one opt-in that steps outside a morphe proof
is **`reorder_includes`** (§15.4): it hoists `#include` — a
*textual, unresolved* splice that can carry hidden order-sensitive content —
past other statements, which *can* change what the program computes. morphe
cannot certify this, because it never resolves the include; the transform rests
on the **caller's assertion** that their includes are pure, not on a morphe
proof.

This is an *accepted* boundary, treated deliberately — not an adversary morphe
defends against. §1's leaf-client posture holds: the caller is vouching for
their own files.

- **Barrier by default (safe).** `#include` is a reorder barrier; the default
  `reorder` holds it in place, and the permutation certificate covers a pure
  rearrangement of the surrounding units (§15.3). No assertion is accepted
  unless the toggle is set.
- **A legible opt-in.** The toggle is `[unsafe] reorder_includes` in
  `morphe.toml` — housed in an `[unsafe]` table apart from the style and
  behavior keys, `deny_unknown_fields` so a typo is a usage error rather than a
  silent enable — or the `--reorder-includes` flag, documented **unsafe**. The
  one meaning-affecting assertion is legible in the governing file, not buried
  in a style key.
- **The backstop is partial.** The clingo differential (§13)
  grounds a reordered program to the same ground-rule set — but only *where the
  includes resolve* at differential time. The residual is therefore every
  `reorder_includes` run whose includes do not resolve (the common case): there
  the caller's assertion is the only thing behind the transform, by design.
- **No containment layers, and why.** Unlike the embedded surface (§3.2), this
  boundary carries no catch/verify/degrade stack — because §1 names no adversary
  here. `reorder_includes` is not hostile input to be contained; it is a
  capability the user turns on for their own program. Containment would answer a
  threat this posture does not assert.

One sharp edge is worth stating, and this posture now rules it. The toggle is
reachable through a **discovered** config, not only a typed flag: `morphe.toml`
is found by walking up from each formatted file (§10.2), so a `[unsafe]
reorder_includes = true` in a parent of a cloned tree would enable the transform
for files the invoking user did not author. Provenance cannot cleanly separate
that from the legitimate case — a project's *own* root `morphe.toml` is also
discovered by walking up — so morphe does not guess whose config it is. **The
ruling: honor the toggle wherever it is set (the deliberate `[unsafe]` opt-in is
trusted), but never let it change meaning silently.** Whenever `reorder_includes`
actually hoists an `#include`, morphe emits a warning that names the move and
states it rests on the caller's unverifiable assertion (the
`morphe::reorder-includes` note, §15.4). morphe cannot *prevent* a bad hoist — it
never resolves the include — so for a capability it cannot verify, visibility is
the honest guarantee: the leaf client trusts the config it is handed, and the one
transform that can change answer sets is loud, not silent. `--isolated` ignores
discovered config entirely and `--config` overrides it, for a caller who wants
neither. The *safe*-but-meaning-relevant toggles settle the same way and need no
note: `reorder` is certified meaning-preserving, and `format_embedded`'s effects
are bounded to a script interior by the certificate (§3.2) — neither can silently
change the ASP program.

## 4. Threats and residuals

### 4.1 A hostile `#script` body crafted against a linked ruff/StyLua

**Scenario.** A `.lp` file carries a `#script(python) … #end.` (or `lua`) body
crafted to exploit the linked formatter — to crash it, to make it emit output that
corrupts the surrounding ASP, or to make it consume unbounded time or memory.

**Containment.**

- *Corruption of the ASP program:* impossible through morphe. The ASP skeleton is
  certified with the body verbatim (§3.2 layer 1), and any backend output that
  would disturb the ASP token structure is caught by the equal-except-`SCRIPT_BODY`
  check and reverted (layer 2). The blast radius of any backend misbehaviour is the
  interior of the offending script, and no further.
- *Crash (panic):* contained by `catch_unwind` → `Panicked` → verbatim + note
  (layer 3). One script never brings down the file or the batch.
- *Memory safety:* morphe's own code is `forbid(unsafe_code)` crate-wide, the
  backends' call sites included, so morphe introduces no memory-unsafety of its
  own. The linked crates are safe-Rust formatters, but their internals — and the
  `ruff_*`/`full_moon` closures they carry — are *accepted, not audited* (§12):
  morphe does not certify them memory-safe, it *contains* them. A fault that
  surfaces as a panic is caught (layer 3); output that disturbs the ASP token
  structure is reverted (layer 2); so a memory-safety fault inside a backend is
  bounded by the same layers rather than eliminated — the accepted dependency
  posture of §12, not a guarantee morphe makes.

**Proportionality.** The body is the *user's own* script, formatted with the tool
they already use for it (ruff/StyLua); this is not arbitrary adversarial input
crossing a privilege boundary. morphe scopes the backends as accepted-but-contained
(§3.3) rather than hardening them to a foundational bar.

**The residuals — an in-process hang or a memory blow-up.** A crafted body that
drives a linked formatter into a non-terminating loop, or into unbounded time or
memory, is the class an in-process integration cannot fully absorb, and the two
forms have *different* bounds: `catch_unwind` catches a panic but neither a hang
nor an allocation storm; a **watchdog bounds time but not memory**; and only a
**subprocess under an OS resource limit (rlimit) bounds memory** (and time). morphe
puts no timeout and no allocation cap on the in-process call (§9.5). Neither form is
reachable on real script bodies. The escape hatch, if it is ever needed, is a
watchdog (for time) or an opt-in subprocess backend with resource limits (for time
*and* memory) — both *behind the same `EmbeddedFormatter` seam*, with no change to
morphe's core (§9.5, §16).

### 4.2 The in-process hang or memory blow-up in CLI batch mode

**Scenario.** morphe formats a directory of `.lp` files in one process (§10.1);
one file's `#script` body hangs a linked backend or drives it into a memory
blow-up (§4.1's residuals), stalling the batch on that file — or, for a memory
storm, ending the whole process.

**Containment.** Graceful degradation makes *every other* failure mode
non-blocking — "continue on" holds across a batch, so one file's script *failure*
never aborts the run (§9.4). Only a true hang or a memory storm is exempt, because
only they deny the seam a return value.

**The residual.** With `--format-embedded` on, a single pathological file can
stall or crash an in-process batch. The safe default already forecloses it:
`format_embedded` is **off by default** (§9.2), so the standard run reformats no
body and cannot hang or blow up on one. The escape hatch is §4.1's — a watchdog
(for time) or a subprocess backend with resource limits (for time and memory)
behind the seam.

## 5. The residuals the security review must rule

morphe records two residuals and asks the security review to rule whether the
leaf-client scoping around them is **adequate**:

1. **A hostile `#script` body against a linked ruff/StyLua** (§4.1): corruption
   and crash are contained; the residuals are an in-process **hang** (unbounded
   time) and a **memory blow-up** (unbounded allocation) — distinct because a
   watchdog bounds time but not memory, and only a subprocess under an OS resource
   limit bounds memory. Both are mitigated by the off-by-default opt-in and the
   behind-the-seam escape hatch.
2. **The in-process hang or memory blow-up in CLI batch mode** (§4.2): the same
   residuals at batch granularity, with the same mitigations.

Neither is a corruption threat nor a memory-safety threat; both are *availability*
residuals of an in-process integration, gated behind an off-by-default opt-in, and
fully bounded only by the opt-in subprocess-with-resource-limits escape hatch.

## 6. Out of scope

- **Arbitrary hostile input as a foundational obligation.** morphe is a leaf
  client (§1); it refuses non-members (§5.7) and never trusts its own output, but
  it does not carry the tier's foundational hostile-input closure. That obligation
  lives in themelios-syntax.
- **The backends' internals and dependency closures.** The `ruff_*` and
  `full_moon` closures enter only under their Cargo features (§9.2); their weight
  and internals are *accepted, not audited* (§12) — documented as such, not
  hand-waved.
- **Execution semantics.** morphe never runs, grounds, or solves a program, and
  never executes an embedded script; there is no code-execution surface to defend.

The `reorder_includes` opt-in was once deferred here; it is now treated in full
as the one accepted transform of the ASP program (§3.4), not held out of scope.
