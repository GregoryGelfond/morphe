# Comments

Comments are the hardest thing a formatter handles, and the usual place one goes
wrong — sliding a comment onto the wrong line, or reflowing on the second run what
it left alone on the first. morphe is built so neither can happen.

## Comments ride with the code they annotate

morphe does not decide what a comment "belongs to" — themelios-syntax already reads
that from the source, giving every comment an **anchor** (the statement or token it
attaches to) and a **slot**:

- **Leading** — on its own line(s) directly above the thing it annotates;
- **Trailing** — on the same line, after it;
- **Dangling** — inside a construct, before its closing bracket, or held apart by a
  blank line.

morphe's job is the inverse of reading them: it emits each comment in the position
that makes a re-parse attach it to *the same anchor and slot*. A leading comment
stays a tight run directly above its anchor; a trailing comment stays on the
anchor's last line; a dangling comment stays inside its construct. Nothing crosses
the code it sits beside.

```clingo
%reachability
edge(1, 2).
reach(X, Y) :- edge(X, Y). %transitive
```

The `%reachability` header stays leading, the `%transitive` note stays trailing on
its rule — and morphe does not even add a space after `%`.

## Blank lines

A blank line you put between two statements is kept (a run of several collapses to
one). A blank line you put *after* a comment to hold it apart from what follows is
kept too, because that blank is what tells the parser the comment stands on its
own. morphe treats blank lines and comment placement as one concern, so it never
introduces a blank that would re-attach a comment, nor removes one you placed to
separate it.

## The one place a comment is re-slotted

There is a single, deliberate exception. A comment written *between a rule's neck
and its body* — when the body is wide enough that morphe breaks it onto its own
line — cannot leave a blank line stranded in the middle of one rule. morphe rides
the comment down with the body, on its own line just above it, so it reads as a
leading comment on that body. The program is unchanged — the comment still sits
between the neck and the body — and formatting again changes nothing more.

## Why this is safe

Two things guarantee it. First, because morphe always emits a comment right beside
the anchor the parser gave it, a comment can never jump the token it leads or
trails — so the transposition that plagues naïve formatters is unreachable. Second,
the certificate is the backstop: any placement that *did* move a comment across a
token would change the program's interleaved sequence of tokens and comments, and
morphe would refuse it before writing a byte (see
[How morphe proves it didn't change your program](safety.md)).
