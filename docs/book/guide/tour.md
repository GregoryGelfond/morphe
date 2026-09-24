# A guided tour

morphe takes an Answer Set Programming source file, gives it one fixed house
style, and proves — before it writes — that it changed only the layout. This tour
runs the whole motion on a small program; the house style is then shown construct
by construct in [The house style](style.md).

## A program, as typed

A small reachability encoding, typed the way programs really arrive — statements
run together, spacing uneven, a comment or two:

```clingo
%reachability
edge(1,2). edge(2,3).edge(3,4).
reach(X,Y):-edge(X,Y).
reach(X,Z):-reach(X,Y),edge(Y,Z).  %transitive
#show reach/2.
```

## Formatted

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

One statement per line; the neck `:-` and the commas spaced; and your comments
left exactly where they were — morphe doesn't even add a space after `%`.

## What just happened — and what didn't

morphe changed **layout only**. Before it wrote the file, it re-parsed its own
output and confirmed the result is the same program — the same statements and
comments, in the same order, differing only in whitespace (and, unless you pass
`--preserve-spellings`, the spelling of interchangeable operators). That check is
not a mode you switch on; it is always on, and if it ever failed, morphe would
report a bug in itself and write nothing. Your original file is never at risk. The
next chapter, [How morphe proves it didn't change your program](safety.md), is
that guarantee in full.

morphe never rewrites your encoding, and it never evaluates, grounds, or solves —
it lays text out and proves it left the meaning alone.

Read on for [the house style](style.md) construct by construct, or jump to
[any command](../reference/commands.md).
