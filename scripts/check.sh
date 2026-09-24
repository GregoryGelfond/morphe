#!/bin/sh
# morphe's single check entry point. Each mode is a slice of the standing gate
# morphe holds every change to; `full` runs them all. Run from anywhere in the
# repository. Requires a stable toolchain with clippy and rustfmt
# (`rust-toolchain.toml` pins the toolchain; `Cargo.toml` pins the edition);
# `coverage` needs cargo-llvm-cov, `differential` needs clingo
# (via pixi, or on PATH), and `book` requires mdBook 0.5.4.
#
# Hosted CI is paused (the GitHub Actions quota; it resumes at the October
# refresh), so this local gate is the gate — run `full` green before you push.
#
#   scripts/check.sh portable      fmt, clippy (+ embedded backends), test (+ embedded), doc (-D warnings)
#   scripts/check.sh coverage      line coverage, floor 90 (cargo-llvm-cov)
#   scripts/check.sh differential  the out-of-band clingo differential (via pixi)
#   scripts/check.sh book          build the mdBook manual
#   scripts/check.sh full          portable, coverage, book, then differential (only when clingo is present)
set -eu

# Run from the repository root regardless of the caller's directory.
cd "$(dirname "$0")/.."

usage() {
	echo "usage: scripts/check.sh <portable|coverage|differential|book|full>" >&2
	exit 2
}

portable() {
	cargo fmt --all --check
	cargo clippy --workspace --all-targets --locked -- -D warnings
	cargo clippy -p morphe -p morphe-cli --all-targets \
		--features embedded-python,embedded-lua --locked -- -D warnings
	cargo test --workspace --locked
	cargo test -p morphe -p morphe-cli \
		--features embedded-python,embedded-lua --locked
	RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
}

coverage() {
	# The fuzz crate is a harness, not a tested surface; it is excluded from the
	# population, as the standing gate excludes it.
	cargo llvm-cov --workspace --exclude morphe-fuzz --locked --fail-under-lines 90
}

# True when the clingo differential can run here: pixi (which supplies clingo and
# its Python module, pixi.toml), or clingo already on PATH for the direct path.
have_differential_env() {
	command -v pixi >/dev/null 2>&1 || command -v clingo >/dev/null 2>&1
}

differential() {
	if command -v pixi >/dev/null 2>&1; then
		pixi run differential
	elif command -v clingo >/dev/null 2>&1; then
		# The direct path assumes clingo's Python module is present too (the format
		# leg drives it); pixi is the pinned authority that bundles both.
		cargo test -p morphe --features differential --test differential \
			-- --nocapture --test-threads=1
	else
		echo "differential mode needs clingo 5.8.2 (via pixi, or on PATH)" >&2
		echo "install: pixi is the pinned authority (pixi.toml); run 'pixi run differential'" >&2
		exit 3
	fi
}

book() {
	want="mdbook v0.5.4"
	have="$(mdbook --version 2>/dev/null || true)"
	if [ "$have" != "$want" ]; then
		echo "book mode needs $want (found: ${have:-none})" >&2
		echo "install: cargo install --locked --version '=0.5.4' mdbook" >&2
		exit 3
	fi
	mdbook build
}

[ $# -eq 1 ] || usage
case "$1" in
	portable) portable ;;
	coverage) coverage ;;
	differential) differential ;;
	book) book ;;
	full)
		portable
		coverage
		book
		# The differential is out of band: run it when clingo is present, else
		# note the skip and let the rest of the gate stand.
		if have_differential_env; then
			differential
		else
			echo "note: differential skipped (no pixi or clingo on PATH)" >&2
		fi
		;;
	*) usage ;;
esac
