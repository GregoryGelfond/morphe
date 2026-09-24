# Choice rules and aggregates

Choice rules, aggregates, and conditions — the constructs real encodings are built
from — get a consistent shape. However you type this assignment rule, morphe gives
it one form.

```clingo
1{assigned(A,T):agent(A),compat(A,T)}1:-task(T).
#minimize{C,A,T:assigned(A,T),cost(A,T,C)}.
```

```console
$ morphe assign.lp
```

```clingo
1 { assigned(A,T) : agent(A), compat(A,T) } 1 :- task(T).
#minimize{ C, A, T : assigned(A,T), cost(A,T,C) }.
```

The count-and-brace bounds are spaced, the braces are spaced inside, a condition's
`:` is spaced, and an aggregate keyword like `#minimize` hugs its brace. Inside
`assigned(A,T)` the argument comma hugs — it is two brackets deep, by the
[bracket-depth rule](../guide/style.md#spacing-follows-bracket-depth).
