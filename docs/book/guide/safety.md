# How morphe proves it didn't change your program

A formatter that silently changed what a program computes would be worse than
useless. morphe's promise — that it changes only how your program *looks* — is not
asserted, it is **checked**, every time, before any output is written.

## The claim

For a program morphe can parse (a *member*), its output stands in one of exactly
two relations to the input, and morphe proves which one holds before returning it:

- **`LayoutOnly`** — nothing but whitespace changed;
- **`UpToSpelling`** — as `LayoutOnly`, except that interchangeable operators were
  normalized to a canonical spelling, and nothing else.

Which relation is the target follows from what you asked for, not from an
independent switch: by default morphe normalizes spellings and proves
`UpToSpelling`; under `--preserve-spellings` it keeps your spellings and proves the
stricter `LayoutOnly`.

## The certificate

The proof is a **certificate**, and morphe earns it the hard way: it re-parses its
own candidate output and compares that against the input. The comparison runs over
the single interleaved sequence of everything significant — the program's tokens
and its comments, in order. A granted certificate therefore means *the same
tokens and the same comments, each still standing where it stood among the
others*. A comment that slid across a token, a rule that changed shape, an
operator that became something it is not — each is a different sequence, and each
is refused.

This is why morphe trusts nothing about its own machinery. It does not assume the
lowering was faithful or the renderer correct; it reads the output back and checks.

## It is always on, never a mode

The certificate is not a `--safe` flag you have to remember. `format` runs the
check every time and **cannot return output it did not certify**. If the check
ever fails, that is a bug in morphe — never a fault in your file — so morphe says
so plainly: it reports an internal error, prints a bug-report request, and writes
nothing. Your original file is untouched. In the library this is the typed
`FormatError::CertificateBroke`; at the command line it is exit code `70`.

## The oracle: a floor morphe cannot undercut

The house style decides the whitespace it *wants* between two tokens. A separate
authority from themelios-syntax — the **fusion oracle** — decides the *minimum* any
correct formatter must leave, so two tokens can never fuse into a third thing:
`0` and `x1` never become `0x1`, `#sum` and `+` never become `#sum+`, `.` and `.`
never become `..`. Where the style and the oracle differ, morphe emits the
maximum. The oracle is a floor the style may exceed but never fall below, and the
certificate is the whole-program backstop should the floor ever be mis-queried.
morphe keeps no spacing table of its own.

## Formatting twice changes nothing more

morphe is **idempotent**: formatting an already-formatted program returns it
unchanged — `format(format(x)) == format(x)`. It follows because the house style
is a function of the program's structure, which re-parsing recovers exactly;
comments re-attach to the same anchor; and spelling normalization is itself
idempotent. This is not assumed — it is a property tested across a corpus, because
it is exactly the invariant a careless comment-reattachment would break.

## A program morphe can't parse

If a file has parse errors it is a *non-member*, and morphe **refuses it whole**
rather than format around the broken regions: it reports the parser's diagnostics
and touches nothing. Formatting a half-typed buffer is a deliberate future
capability, not something v1 guesses at.

## Depth

morphe parses at themelios-syntax's default nesting limit, so a program nested
deeper than that bound is refused as a parse error rather than risking the stack;
within the limit, every walk morphe makes runs comfortably on an ordinary thread.
