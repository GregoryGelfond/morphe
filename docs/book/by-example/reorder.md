# Reordering a program

By default morphe leaves your statements exactly where they are. Add `--reorder`
and it also tidies their order, grouping them into the canonical signature shape
and proving it only moved them.

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

`#defined` leads, `#const` follows, the rules keep their source order in the
middle, and `#show` goes last. Statements whose order can matter — and every
`#include` — stay put and act as fences the reorder won't cross. See
[Reordering](../guide/reorder.md) for the full rules and the permutation proof.
