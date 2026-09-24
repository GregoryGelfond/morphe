# Formatting a program

The everyday case: run-together statements and loose spacing become one clean,
consistent program. Nothing but layout changes.

```clingo
%reachability
edge(1,2). edge(2,3).edge(3,4).
reach(X,Y):-edge(X,Y).
reach(X,Z):-reach(X,Y),edge(Y,Z).  %transitive
#show reach/2.
```

```console
$ morphe reach.lp
```

```clingo
%reachability
edge(1, 2).
edge(2, 3).
edge(3, 4).
reach(X, Y) :- edge(X, Y).
reach(X, Z) :- reach(X, Y), edge(Y, Z). %transitive
#show reach/2.
```

One statement per line, the neck `:-` and the commas spaced, and the comments left
exactly where they were. morphe checked all of this against your program before it
wrote the file.
