//! The `morphe` binary — a thin shim over [`morphe_cli::run`]. See
//! docs/design/morphe.md §10; the driver and the exit contract live in the
//! library.

use morphe_cli::outcome::Outcome;

fn main() -> Outcome {
    morphe_cli::run()
}
