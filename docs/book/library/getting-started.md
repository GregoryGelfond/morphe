# Getting started

morphe is a library first; the `morphe` command is a thin shell over it. A tool
embeds the formatter directly and gets the formatted text — and the certificate it
earned — with no process boundary and no text protocol in between. The core is
pure and does no I/O.

## Add the dependency

morphe isn't on crates.io yet (it pins `themelios-syntax` to a git revision), so
depend on it by git:

```toml
[dependencies]
morphe = { git = "https://github.com/GregoryGelfond/morphe" }
```

## Format a program

`format` takes a `Source` and `FormatOptions` and returns a `Formatted` — the text
plus the certificate morphe proved — or a typed `FormatError`:

```rust
use morphe::themelios_syntax::base::source::SourceId;
use morphe::{format, FormatOptions, Source};

let text = "edge(1,2). edge(2,3).\nreach(X,Y):-edge(X,Y).\n";
let source = Source::new(SourceId::new(0), text.to_owned())
    .expect("input within Source::MAX_LEN");

match format(&source, &FormatOptions::default()) {
    Ok(formatted) => print!("{}", formatted.text),
    Err(error) => eprintln!("morphe refused: {error:?}"),
}
```

prints

```clingo
edge(1, 2).
edge(2, 3).
reach(X, Y) :- edge(X, Y).
```

`FormatOptions::default()` is the house style at width 100, clingo dialect, every
transform off. The whole ASP vocabulary morphe needs — `Source`, `Parse`,
`Program`, `Dialect`, the `Certificate` — is re-exported through morphe (and the
tier itself as `morphe::themelios_syntax`), so a consumer names it through morphe
alone. The next chapters cover [the `Formatted` value and the
certificate](format.md) and [the diagnostics and exit codes](diagnostics.md).
