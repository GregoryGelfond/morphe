# Wrapping a wide rule

A statement that fits the line width stays on one line:

```console
$ morphe game.lp
```

```clingo
win(X) ; lose(X) :- player(X), move(X, Y), not win(Y), reachable(X).
```

When a rule is wider than the line width, morphe breaks its body one element per
line, so the structure stays legible instead of running off the screen:

```console
$ morphe --line-width 40 game.lp
```

```clingo
win(X) ; lose(X) :-
    player(X),
    move(X, Y),
    not win(Y),
    reachable(X).
```

The head stays on the first line, the body breaks after the neck, and each goal
sits on its own indented line. Only the layout changed — the same certificate is
checked whether the rule fits or wraps.
