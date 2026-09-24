# The format API and the certificate

morphe's core is three pure functions and the values they return.

## The entry points

```rust
pub fn format(source: &Source, options: &FormatOptions)
    -> Result<Formatted, FormatError>;

pub fn format_parsed(parse: &Parse<Program>, options: &FormatOptions)
    -> Result<Formatted, FormatError>;

pub fn reorder(source: &Source, options: &FormatOptions)
    -> Result<Reordered, ReorderError>;
```

`format` parses under `options.dialect`, lays the program out, and certifies the
result. `format_parsed` is for a consumer that already holds a `Parse<Program>` — a
language server, say — so morphe never forces a re-parse; it reads the dialect from
the parse and ignores `options.dialect`. `reorder` is the [opt-in
reorder](../guide/reorder.md), a *separate* operation — `format` never changes
statement order.

## `FormatOptions`

```rust
pub struct FormatOptions {
    pub line_width: usize,           // the one style knob; DEFAULT_LINE_WIDTH = 100
    pub dialect: Dialect,            // the ASP dialect: clingo (default) or ASP-Core-2
    pub preserve_spellings: bool,    // keep synonyms (proves LayoutOnly) vs normalize (UpToSpelling)
    pub embedded: EmbeddedFormatters, // the injected backends (empty by default)
    pub format_embedded: bool,       // reformat #script bodies (off by default)
    pub reorder_includes: bool,      // the one unsafe reorder toggle (off by default)
}
```

`FormatOptions::default()` is width 100, clingo, everything off. `line_width` is the
only layout knob — indent and the rest are fixed.

## `Formatted`, and the certificate

```rust
pub struct Formatted {
    pub text: String,
    pub certificate: Certificate,   // the relation proved of the ASP program
    pub embedded_changed: bool,     // a #script body was reformatted
    pub changed: bool,              // the text differs from the input
    pub notes: Vec<Diagnostic>,     // embedded outcomes and other warnings
}
```

The `certificate` is the heart of it: the proof, re-derived from morphe's own
output, that the text is `LayoutOnly` or `UpToSpelling` with respect to the input
(see [How morphe proves it didn't change your program](../guide/safety.md)).
`format` cannot return an `Ok` it did not certify. When `embedded_changed` is true
the whole-text guarantee is the composed one — the ASP certificate *plus* the
equal-except-script-bodies check — not the certificate alone.

`reorder` returns the parallel `Reordered { text, certificate, reordered, changed,
notes }`, where `reordered` says whether any unit actually moved, `certificate` is
the uniform claim every unit earned, and the permutation proof itself is carried by
the `Ok` return.
