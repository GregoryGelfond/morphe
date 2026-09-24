# Embedded scripts

A clingo program can embed Python or Lua in a `#script(python) … #end.` block.
That body is opaque to the ASP parser — and, by default, opaque to morphe: it is
kept **byte-for-byte verbatim** while the ASP around it is formatted.

```clingo
count(N) :- N = #count{ X : item(X) }.
#script(python)
def main( prg ):
        prg.ground([("base",[])])
        prg.solve()
#end.
item(a).
item(b).
```

The rules and facts are laid out in the house style; the Python is left exactly as
written, down to its spacing and indentation. morphe touches a `#script` body only
if you ask.

## Opt-in formatting

Turn embedded formatting on and morphe hands each script body to that language's
own formatter — **ruff** for Python, **StyLua** for Lua — and splices the result
back in. It is gated twice, so the lean default costs nothing:

- **At build time**, the backends live behind the Cargo features `embedded-python`
  and `embedded-lua`; a build without them compiles neither tool nor its
  dependencies.
- **At run time**, it is still off unless you pass `--format-embedded`.

## The safety, composed

morphe's certificate is a statement about your *ASP program*, and reformatting a
script body changes that body's text — so an embedded change is, by construction,
outside the layout-only guarantee. morphe keeps the two cleanly separated and
proves each in its own way:

1. **The ASP program is certified with every body held verbatim** — the same
   guarantee as an ordinary format, and it holds *no matter what a backend does*.
2. **Each script interior is delegated** to the backend's own meaning-preservation
   (StyLua's full re-parse check; ruff's stability, plus morphe's own re-parse of
   the result). morphe treats ruff and StyLua as tools trusted for their own
   language, not as things it re-derives.
3. **morphe binds the two together**: after formatting the bodies, it re-parses the
   whole file and confirms the ASP token structure is *identical* to the certified
   skeleton, differing only inside the script bodies. If a backend's output
   disturbed anything outside its own script, that body reverts to verbatim.

The upshot: a bug — or even hostile output — in ruff or StyLua can never reach your
ASP program. The worst it can do is change the interior of its own script, and the
structural check bounds even that.

## When a backend can't, or won't

Every way a backend can fail — the language unsupported, the backend absent, the
body rejected, its own check failed, or a panic caught at the boundary — collapses
to a single outcome: **that body stays verbatim, a warning is recorded, and nothing
stops.** One failed script never halts the others or the ASP format, and across a
batch one file's failure never aborts the run. Embedded outcomes are warnings; they
never change morphe's exit code. A failure lands exactly where the safe default is,
never anywhere worse.

The residuals an in-process formatter cannot fully bound are a backend that *hangs*
(unbounded time) and one that exhausts *memory* (unbounded allocation): there is no
timeout and no allocation cap on the in-process call, and the two need different
remedies — a watchdog bounds time, but only a subprocess under an OS resource limit
bounds memory. Neither is reachable on real script bodies, and the escape hatch, if
one is ever needed, is a watchdog or an opt-in subprocess backend behind the same
seam, with no change to morphe's core. The full account is in the
[threat model](https://github.com/GregoryGelfond/morphe/blob/main/docs/security/threat-model.md).
