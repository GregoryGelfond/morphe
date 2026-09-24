# About this book

morphe is an opinionated formatter for Answer Set Programming. It gives the
clingo/clingcon dialect and the ASP-Core-2 standard one fixed house style — the
way `rustfmt`, `black`, and `ruff` do for their languages — and it never changes
what a program means: before it writes a file, it re-reads its own output and
*proves* that only the layout changed (and, unless you ask otherwise, the spelling
of interchangeable operators). If it cannot prove that, it writes nothing.

This book covers using the `morphe` command, the house style and the safety
argument behind it, and embedding morphe as a Rust library. morphe is built on
[themelios-syntax](https://github.com/GregoryGelfond/themelios) — the ASP program
representation it parses and lays out — and it formats only: it never evaluates,
grounds, or solves.

## Start here

| I want to… | Read |
|---|---|
| See the whole idea in one worked example | [A guided tour](guide/tour.md) |
| Understand how morphe proves it didn't change my program | [How morphe proves it didn't change your program](guide/safety.md) |
| Know exactly what the house style does | [The house style](guide/style.md) |
| Reorder my statements safely | [Reordering](guide/reorder.md) |
| Format the Python or Lua in my `#script` blocks | [Embedded scripts](guide/embedded.md) |
| Configure morphe for a project | [Configuration](guide/configuration.md) |
| Call morphe from Rust | [Getting started](library/getting-started.md) |
| Look up a command or flag | [Using the morphe command](reference/commands.md) |

## The two parts

**Part I — The formatter** is for the ASP author: what morphe guarantees and how it
proves it, the house style construct by construct, how comments are kept with the
code they annotate, the opt-in reorder, embedded `#script` formatting, and
configuration. The **morphe by example** chapters run the whole thing on five real
programs.

**Part II — The Rust library** is for the tool builder embedding morphe: the pure
`format` API and the certificate it returns, the typed diagnostics and exit codes,
and the contained boundary around the embedded formatters. morphe is a library
first; the `morphe` command is a thin shell over it.

## Results and limits

morphe is a formatter, and only that:

- **It changes layout always**, operator spelling unless `--preserve-spellings`,
  and statement order only under the opt-in `--reorder`. It never rewrites your
  encoding, and it never evaluates, grounds, or solves — those belong to other
  tools.
- **It proves it before it writes.** Every format re-parses morphe's own output and
  checks it against the input; if that check ever fails, morphe treats it as a bug
  in itself, reports an internal error, and writes nothing — see
  [How morphe proves it didn't change your program](guide/safety.md).
- **Your comments stay put**, next to the code they annotate, exactly as written —
  morphe does not even add a space after `%`.
- **Embedded `#script` bodies stay verbatim** unless you opt in, and even then a
  backend's output is bounded to the interior of its own script (see
  [Embedded scripts](guide/embedded.md)).

The design of record and the security posture live beside the book, not in it:
[`docs/design/morphe.md`](https://github.com/GregoryGelfond/morphe/blob/main/docs/design/morphe.md)
is the design of record, and
[`docs/security/threat-model.md`](https://github.com/GregoryGelfond/morphe/blob/main/docs/security/threat-model.md)
is the threat model the security review answers to.
