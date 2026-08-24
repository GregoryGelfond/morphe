//! The reorder laws and goldens (docs/design/morphe.md §13, §15): the certified
//! reorder organizes a program's signature — `#defined` → `#const` → Body →
//! `#show` — within the regions its barriers divide (§15.2), proves the
//! permutation certificate before writing (§15.3), and carries each unit's
//! comments with it (§15.1). Inline goldens pin the reorganization; the property
//! laws run over the vendored corpus.

use morphe::{Dialect, FormatOptions, Reordered, format, reorder};
use morphe_test_support::{cases, options, source};

fn reordered(text: &str, options: &FormatOptions) -> Reordered {
    reorder(&source(text), options).expect("a member reorders")
}

fn formatted(text: &str, options: &FormatOptions) -> String {
    format(&source(text), options)
        .expect("a member formats")
        .text
}

#[test]
fn units_reorganize_within_a_region() {
    // #defined → #const → Body → #show, out of source order into ORDER (§15.2).
    let out = reordered(
        "#show s/1.\nr :- q.\n#defined d/1.\n#const c = 1.\n",
        &FormatOptions::default(),
    );
    assert_eq!(
        out.text,
        "#defined d/1.\n#const c = 1.\nr :- q.\n#show s/1.\n"
    );
    assert!(out.reordered);
    assert!(out.changed);
}

#[test]
fn an_all_barrier_program_is_merely_formatted() {
    // #program parts are barriers (§15.4): nothing reorderable moves, so the
    // reorder is a plain format — reordered is false, though the layout may still
    // change (§15.3).
    let text = "#program  base.\n#program  other.\n";
    let out = reordered(text, &FormatOptions::default());
    assert!(!out.reordered);
    assert_eq!(out.text, formatted(text, &FormatOptions::default()));
    assert!(out.changed); // the doubled space collapsed
}

#[test]
fn the_query_is_pinned_last_while_its_region_reorganizes() {
    // The ASP-Core-2 query must be the final statement (§15.4); it is a barrier,
    // so #show reorganizes before it but is never pushed past it.
    let out = reordered("#show s/1.\na :- b.\np(1)?\n", &options(Dialect::AspCore2));
    assert_eq!(out.text, "a :- b.\n#show s/1.\np(1)?\n");
    assert!(out.reordered);
}

#[test]
fn reorder_includes_hoists_an_include_through_the_public_reorder() {
    // The unsafe toggle flows through the public reorder (§15.4): by default a
    // top-level #include is a held barrier and nothing moves; under
    // reorder_includes it becomes a unit hoisted to the leading section.
    let text = "r :- q.\n#include \"lib.lp\".\n#defined d/1.\n";
    assert!(!reordered(text, &FormatOptions::default()).reordered);

    let unsafe_toggle = FormatOptions {
        reorder_includes: true,
        ..FormatOptions::default()
    };
    let hoisted = reordered(text, &unsafe_toggle);
    assert_eq!(
        hoisted.text,
        "#include \"lib.lp\".\n#defined d/1.\nr :- q.\n"
    );
    assert!(hoisted.reordered);
    // The hoist is surfaced, never silent: the one reorder that can change answer
    // sets emits a warning (§15.4). The default reorder above held the include as
    // a barrier and moved nothing, so it says nothing.
    assert_eq!(
        hoisted.notes.len(),
        1,
        "hoisting an #include under reorder_includes emits the warning note"
    );
    assert!(
        reordered(text, &FormatOptions::default()).notes.is_empty(),
        "the default reorder holds the include and says nothing"
    );
}

#[test]
fn a_moved_units_leading_comment_travels_with_it() {
    // A moved statement carries its Leading trivia: #defined's comment moves with
    // it to the front (§15.1).
    let out = reordered(
        "#show s/1.\n% comment on defined\n#defined d/1.\n",
        &FormatOptions::default(),
    );
    assert_eq!(
        out.text,
        "% comment on defined\n#defined d/1.\n#show s/1.\n"
    );
    assert!(out.reordered);
}

#[test]
fn a_moved_units_trailing_comment_travels_with_it() {
    // A moved statement carries its Trailing trivia as well as its Leading (§13,
    // §15.1): #defined's trailing comment moves with it to the front.
    let out = reordered(
        "#show s/1.\n#defined d/1. % the defined signature\n",
        &FormatOptions::default(),
    );
    assert_eq!(
        out.text,
        "#defined d/1. % the defined signature\n#show s/1.\n"
    );
    assert!(out.reordered);
}

#[test]
fn a_moved_units_owned_documentation_travels_with_it() {
    // A moved statement carries its owned %! documentation — structural, kept
    // verbatim above the statement — with it (§13, §15.1, §7.3).
    let out = reordered(
        "#show s/1.\n%! the defined signature\n#defined d/1.\n",
        &FormatOptions::default(),
    );
    assert_eq!(
        out.text,
        "%! the defined signature\n#defined d/1.\n#show s/1.\n"
    );
    assert!(out.reordered);
}

#[test]
fn a_dangling_comment_holds_between_reordered_regions() {
    // A floating comment is a barrier: the units on each side reorganize, but none
    // crosses the comment, which still re-parses as Dangling (§15.1, §15.3).
    let out = reordered(
        "#show s1/1.\n#defined d1/1.\n\n% floating\n\n#show s2/1.\n#defined d2/1.\n",
        &FormatOptions::default(),
    );
    assert_eq!(
        out.text,
        "#defined d1/1.\n#show s1/1.\n\n% floating\n\n#defined d2/1.\n#show s2/1.\n"
    );
    assert!(out.reordered);
}

#[test]
fn the_reorder_grants_and_is_idempotent_over_the_corpus() {
    // Every member reorders — the permutation certificate holds (§13, §15.3) — and
    // reordering the reordered text changes nothing (idempotent, §13).
    for case in cases() {
        if !case.is_member {
            continue;
        }
        let options = options(case.dialect);
        let once = reorder(&source(&case.text), &options).unwrap_or_else(|err| {
            panic!("{}: reorder refused a member: {err:?}", case.path.display())
        });
        let twice = reorder(&source(&once.text), &options).unwrap_or_else(|err| {
            panic!(
                "{}: reorder refused its own output: {err:?}",
                case.path.display()
            )
        });
        assert_eq!(
            once.text,
            twice.text,
            "{}: reorder is not idempotent",
            case.path.display()
        );
    }
}
