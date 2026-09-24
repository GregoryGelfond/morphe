# Building the documentation

The book is built with mdBook 0.5.4, pinned so the rendered output matches what
the checks build.

Install the exact version:

```sh
cargo install --locked --version '=0.5.4' mdbook
```

Build the book — output lands under the git-ignored `target/book/`:

```sh
mdbook build
```

Or run it through the project's check entry point:

```sh
scripts/check.sh book
```

The Markdown sources live under [`docs/book/`](.); `book.toml` at the repository
root configures the build. The design-of-record documents — the design of record
[`docs/design/morphe.md`](https://github.com/GregoryGelfond/morphe/blob/main/docs/design/morphe.md),
the findings ledger
[`docs/findings.md`](https://github.com/GregoryGelfond/morphe/blob/main/docs/findings.md),
and the threat model
[`docs/security/threat-model.md`](https://github.com/GregoryGelfond/morphe/blob/main/docs/security/threat-model.md)
— are kept outside the book and linked from [About this book](index.md).
