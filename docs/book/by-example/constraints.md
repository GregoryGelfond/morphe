# Constraint atoms (clingcon)

morphe formats clingcon's constraint atoms too, with the same fixed style. Here is
an N-queens model built from them — one queen per row, none sharing a column or a
diagonal.

```clingo
#const n=8.
row(1..n).
% q(R): the column of the queen in row R.
&dom{1..n}=q(R):-row(R).
&distinct{q(R):row(R)}.
&distinct{q(R)-R:row(R)}.
&distinct{q(R)+R:row(R)}.
```

```console
$ morphe queens.lp
```

```clingo
#const n = 8.
row(1..n).
% q(R): the column of the queen in row R.
&dom{ 1 .. n } = q(R) :- row(R).
&distinct{ q(R) : row(R) }.
&distinct{ q(R) - R : row(R) }.
&distinct{ q(R) + R : row(R) }.
```

The `&`-atom's head hugs its brace, the brace is spaced inside like an aggregate's,
and the guard `=` is spaced like any relation. The theory terms inside keep their
operators spaced — `1 .. n`, `q(R) - R`, `q(R) + R`.
