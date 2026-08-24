//! CI shape assertions (docs/design/morphe.md §13): complexity shape only,
//! held by the median over five interleaved wall-clock ratios with tolerances
//! wide enough for any CI machine — `format` linear in the input text, and the
//! document render together with the certificate linear in the tree. What they
//! prove: the claimed class (a linear layout pass, a certificate that re-walks
//! two texts). What they cannot: absolute speed — that lives in the out-of-band
//! benches (`benches/scaling.rs`).
//!
//! The certificate is the tier's `equivalent` over a re-parse of the output
//! (§5.1); its linearity in both texts is inherited from the syntax tier, whose
//! own shape suite times `parse` and `equivalent` directly (docs/design/syntax.md
//! §16). Here it is exercised again within `format` and `format_parsed`, so a
//! regression in morphe's composition of it is caught in this gate too.
//!
//! Each ratio is the median over five runs that time the small case and the
//! large case back-to-back, not the ratio of two separately-median'd batches: a
//! load transient during a run inflates both of that run's halves and cancels in
//! its ratio, so no transient landing on the large measurement alone can push
//! the ratio past its ceiling.

use std::time::Instant;

use morphe::themelios_syntax::base::source::SourceId;
use morphe::themelios_syntax::parse::parse;
use morphe::{Dialect, FormatOptions, Parse, Program, Source, format, format_parsed};

/// One rule with a comment run, a pool, an interval, an aggregate, and a theory
/// atom, so every size exercises the layout families the render walks (§7.2).
const UNIT: &str = "% leading\np(X, f(Y)) :- q(X; Y), not r(X), X = 1..3, #sum { W,T : t(T,W) } >= 4, &sum { x, -y : p } <= 3. % trailing\n";

/// The data-size ratio between the small and large cases.
const SIZE_RATIO: usize = 16;
/// A linear claim at `SIZE_RATIO` may cost at most this factor: fourfold noise
/// headroom above linear (x16) and fourfold separation below quadratic (x256).
const LINEAR_CEILING: u128 = SIZE_RATIO as u128 * 4;
/// Interleaved runs per measurement; the median of their ratios is taken.
const SAMPLES: usize = 5;
/// Ratios are scaled by this factor so the median arithmetic stays in integers;
/// a ceiling `C` is the scaled bound `C * RATIO_SCALE`.
const RATIO_SCALE: u128 = 1000;

/// The small case in copies of `UNIT`, and the large case `SIZE_RATIO` times it.
const SMALL_UNITS: usize = 64;

fn text_of(units: usize) -> String {
    UNIT.repeat(units)
}

fn admitted(units: usize) -> Source {
    Source::new(SourceId::new(0), text_of(units)).expect("test text admits")
}

fn parsed(units: usize) -> Parse<Program> {
    parse(&admitted(units), Dialect::Clingo)
}

/// One elapsed measurement of `work`, in nanoseconds — floored to 1 so a
/// sub-nanosecond reading can still divide.
fn time_once(mut work: impl FnMut()) -> u128 {
    let start = Instant::now();
    work();
    start.elapsed().as_nanos().max(1)
}

/// The median over `SAMPLES` interleaved runs of `big`'s cost over `small`'s,
/// scaled by `RATIO_SCALE`. Each run evaluates `small` then `big` back-to-back.
fn median_ratio(mut small: impl FnMut() -> u128, mut big: impl FnMut() -> u128) -> u128 {
    let mut ratios = Vec::with_capacity(SAMPLES);
    for _ in 0..SAMPLES {
        let s = small().max(1);
        let b = big();
        ratios.push(b * RATIO_SCALE / s);
    }
    ratios.sort_unstable();
    ratios[SAMPLES / 2]
}

#[test]
fn format_is_linear_in_the_input() {
    let small = admitted(SMALL_UNITS);
    let big = admitted(SMALL_UNITS * SIZE_RATIO);
    let options = FormatOptions::default();
    let ratio = median_ratio(
        || {
            time_once(|| {
                std::hint::black_box(format(&small, &options)).expect("a member formats");
            })
        },
        || {
            time_once(|| {
                std::hint::black_box(format(&big, &options)).expect("a member formats");
            })
        },
    );
    let approx = ratio / RATIO_SCALE;
    assert!(
        ratio < LINEAR_CEILING * RATIO_SCALE,
        "format's median ratio was ~x{approx} ({ratio}/{RATIO_SCALE}) over x{SIZE_RATIO} text; the linear shape allows at most x{LINEAR_CEILING}"
    );
}

#[test]
fn the_render_and_certificate_are_linear_in_the_tree() {
    // `format_parsed` takes a tree already parsed, so this measures the layout
    // pass — lower, render, and the certificate over a re-parse of the output —
    // in the tree's size, with the input parse excluded (§4.2, §5.1).
    let small = parsed(SMALL_UNITS);
    let big = parsed(SMALL_UNITS * SIZE_RATIO);
    let options = FormatOptions::default();
    let ratio = median_ratio(
        || {
            time_once(|| {
                std::hint::black_box(format_parsed(&small, &options)).expect("a member formats");
            })
        },
        || {
            time_once(|| {
                std::hint::black_box(format_parsed(&big, &options)).expect("a member formats");
            })
        },
    );
    let approx = ratio / RATIO_SCALE;
    assert!(
        ratio < LINEAR_CEILING * RATIO_SCALE,
        "the render and certificate's median ratio was ~x{approx} ({ratio}/{RATIO_SCALE}) over x{SIZE_RATIO} tree; the linear shape allows at most x{LINEAR_CEILING}"
    );
}
