# The embedded fault boundary

The embedded formatters — ruff and StyLua — are the one thing morphe runs that it
does not itself verify. They sit behind a single seam, and everything they can do
wrong is contained there.

## The seam

```rust
pub trait EmbeddedFormatter {
    fn language(&self) -> &LanguageKey;
    fn availability(&self) -> Availability;
    fn format(&self, body: &str, cx: &EmbeddedContext)
        -> Result<String, EmbeddedError>;
}
```

A caller assembles the backends into an `EmbeddedFormatters` registry and puts it
in `FormatOptions::embedded`; the core asks it by language and knows nothing else
about a backend — not its name, not whether it links a library or shells out. The
concrete `RuffLibBackend` and `StyLuaLibBackend` live behind the Cargo features
`embedded-python` and `embedded-lua`.

## Every failure converges to one outcome

```rust
pub enum EmbeddedError {
    Unsupported,                    // no backend for this language
    Unavailable { reason: String }, // backend disabled or absent
    Failed { message: String },     // the backend rejected the body
    Unverified,                     // the backend's own round-trip check failed
    Panicked,                       // a panic, caught at the boundary
}
```

Whatever the reason, the result is the same: **that script body stays verbatim, a
warning goes into `notes`, and nothing short-circuits.** One failed script never
stops the others or the ASP format; across a batch, one file never aborts the run.
A failure lands exactly where `format_embedded = false` would — never anywhere
worse — so embedded outcomes are warnings and never change the exit code.

A panic inside a linked backend is caught with `std::panic::catch_unwind` and
turned into `Panicked`; this relies on the unwinding panic strategy, which morphe's
build profiles pin. See [Embedded scripts](../guide/embedded.md) for the
composed-safety argument, and
[the threat model](https://github.com/GregoryGelfond/morphe/blob/main/docs/security/threat-model.md)
for the residuals morphe records.
