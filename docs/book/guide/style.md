# The house style

morphe has one fixed style. The only thing you set is the line width; everything
else — spacing, indentation, where a line breaks — is decided for you, so the only
question a diff ever raises is what changed in the *program*, never whose
formatting preferences won. This chapter is that style, construct by construct.
Every block below is real morphe output.

## The global shape

- **The one knob is `line_width`** (default 100). Nothing else about layout is
  configurable.
- **One statement per line**, terminated by its `.` (or an ASP-Core-2 query's `?`).
- **Four-space indentation**, never tabs.
- **Blank-line runs collapse to a single blank**, and the file ends in exactly one
  newline:

```clingo
a.

b.
```

## Spacing follows bracket depth

The signature rule, and the one worth learning first: a `,` or `;` separator is
**spaced at the top level and one bracket deep, and tightens from two brackets in**.

```clingo
h :- a, b.
p(X, Y).
q(f(a,b)).
```

`a, b` (a rule body, top level) and `X, Y` (one bracket deep) are spaced; the
`a,b` inside `f(…)` inside `q(…)` is two deep, so it hugs. A **term operator** is
spaced only at the top level and tight inside any bracket:

```clingo
x :- X = Y + Z.
p(X+Y).
```

The interval `..` is a term operator, so it takes the same rule — spaced at the top
level, tight inside a bracket. (clingo parses `1 .. 3` and `1..3` identically; one
rule governs every binary operator.)

```clingo
r(X) :- X = 1 .. 3.
p(1..3).
```

Three spacings never vary with depth: a **relation** (a comparison or guard) is
always spaced; **`not`** is always spaced from its operand; and a **unary `-` or
`~`** always hugs it.

```clingo
q(-1).
a :- not b.
```

## Necks, aggregates, disjunction

The neck `:-` and the weak-constraint neck `:~` are spaced on both sides. Aggregate
and set braces are spaced *inside*, the aggregate keyword hugs its brace, elements
are separated by `; `, and a condition's `:` is spaced. `#minimize` and `#maximize`
lay out exactly like an aggregate.

```clingo
1 { assigned(A,T) : agent(A), compat(A,T) } 1 :- task(T).
#minimize{ C, A, T : assigned(A,T), cost(A,T,C) }.
```

A disjunctive head is spaced at its separator (`win(X) ; lose(X)`, in
[Wrapping a wide rule](../by-example/wrapping.md)). A signature `/` and a priority
`@` hug:

```clingo
#show p/2.
:~ q(X). [1@2, X]
```

## Breaking a line that's too wide

When a construct fits, it stays on one line. When it doesn't, morphe breaks it at
its **list or chain** separator — a body's commas, an argument list's, an
aggregate's `;`, a disjunction's separator, an operator chain's operators — one
element per line, indented one level. A **relation** is not such a separator: a
long comparison chain stays on its line, and a wide operand breaks within itself.
[Wrapping a wide rule](../by-example/wrapping.md) shows the motion in full.

## The trailing separator

morphe does not treat a trailing comma as a signal, the way `black`'s "magic
trailing comma" does. A trailing separator is **preserved and laid out by width
alone** — kept when the group fits, exploded with the group when it doesn't. This
matters in ASP because a trailing comma is not always inert: a one-tuple `(a,)` is
a different term from the parenthesized `(a)`, and morphe keeps that distinction
exactly.

```clingo
u((a,)).
v((a)).
```

morphe never *adds* a trailing separator, and an argument list may not carry one
(`f(a,)` is not a valid program).

## What morphe never reflows

Three regions are content, not layout, and are emitted verbatim:

- **`%!` documentation lines**, kept above their statement;
- **`#script` bodies** (unless you opt into [embedded formatting](embedded.md));
- **the text of every comment** — morphe trims only trailing whitespace on a
  comment line, and never the text, not even the space after `%`.

See [Comments](comments.md) for how a comment is kept with the code it annotates.

## Spelling

Unless you pass `--preserve-spellings`, morphe normalizes interchangeable operator
spellings to one canonical form — `==` becomes `=`, `<>` becomes `!=`, and
`#minimise` / `#maximise` become `#minimize` / `#maximize` — converging on what
clingo prints. That is the one change beyond whitespace morphe makes, and it is
what moves the certificate from `LayoutOnly` to `UpToSpelling` (see
[How morphe proves it didn't change your program](safety.md)). Which spellings
count as synonyms is themelios-syntax's knowledge, not morphe's.
