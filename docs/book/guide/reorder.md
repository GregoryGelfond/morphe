# Reordering

By default morphe never changes the order of your statements — order can carry
meaning in ASP, and `format` holds it exactly. Reordering is a **separate, opt-in**
operation: pass `--reorder` (or set `reorder = true`) and morphe also tidies the
order of your statements into a canonical shape — proving, as always, that it only
moved them.

## What it does

```clingo
#show reach/1.
reach(Y) :- reach(X), edge(X, Y).
#defined edge/2.
reach(X) :- start(X).
#const max = 3.
```

```console
$ morphe --reorder program.lp
```

```clingo
#defined edge/2.
#const max = 3.
reach(Y) :- reach(X), edge(X, Y).
reach(X) :- start(X).
#show reach/1.
```

The canonical order is the program's *signature*, read top to bottom: **`#defined`**
(the input signature) → **`#const`** (the parameters) → **the rules and everything
else**, in their original source order → **`#show`** (the output signature). The
rules keep their relative order; only the signature directives move to their groups.

A statement carries its documentation and its leading and trailing comments with it
when it moves — morphe moves whole *units*, comments included, never bare lines.

## What stays put: barriers

Some statements can't be moved without risking a change in meaning, and morphe
treats each as a **barrier** — held in place, never reordered across. Reordering
happens only *within* the regions the barriers divide. A top-level element is a
barrier exactly when its position can affect the program's answer sets, including
through content morphe cannot see:

- **`#include`** — a textual splice of another file whose contents morphe never
  resolves (see below);
- **`#program`**, **`#script`**, and an overriding **`#const`** — each governs the
  statements around it by position;
- an **ASP-Core-2 query**, which must remain the program's final statement;
- a **floating top-level comment**, which stays where you put it.

## The proof

A reorder deliberately changes the program's text, so the layout certificate would
(correctly) refuse it — and morphe earns a *different* guarantee instead, a
**permutation certificate**. After reordering, morphe re-parses its own output and
checks a total, both-ways match: every original statement present exactly once,
nothing added, dropped, or split, and every barrier still holding its place and its
neighbours. Each moved statement is also individually layout-certified, exactly as
`format` certifies it. If any of that fails, morphe treats it as a bug in itself,
reports an internal error, and leaves your file untouched.

## `#include`, and the one unsafe toggle

Because morphe never resolves an `#include`, it cannot see whether the included file
carries something order-sensitive — so by default `#include` is a barrier. If you
know your includes are pure (the common "library of definitions" discipline), you
can lift that restriction with `--reorder-includes` (or `[unsafe] reorder_includes
= true`), which hoists `#include` to the front with the other signature directives.
morphe cannot prove this safe, so it never does it silently: whenever the toggle
actually moves an `#include`, morphe emits a warning that names the move and states
it rests on your unverified assertion. It is the one transform in morphe that rests
on your word rather than a morphe proof.
