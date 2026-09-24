# Security policy

## Supported versions

morphe is developed on `main`, and the supported version is the latest tagged release built from it. Fixes land on `main` and ship in the next release; there is no separate maintenance branch.

## Reporting a vulnerability

Please report a suspected vulnerability privately, not as a public issue. Use GitHub's private vulnerability reporting — the **Security** tab of the repository, then **Report a vulnerability** — which opens a private advisory visible only to the maintainer.

Include what a maintainer needs to reproduce it: the input (an ASP program, a `morphe.toml`, or a `#script` body), the command or library call, and what happened versus what you expected. Please allow a reasonable window for an acknowledgement and a fix before any public disclosure.

## Scope

morphe formats a user's own ASP source under their own configuration; it invokes no solver, runs no embedded script itself, and is not a network or multi-tenant boundary. Its input surfaces — an ASP program, a discovered `morphe.toml`, and (opt-in) a `#script` body handed to a linked backend — and what is defended at each are stated in the threat model of record, [`docs/security/threat-model.md`](docs/security/threat-model.md), the right starting point for a security review.

Reports of the following are especially in scope:

- **a format that changed a program's meaning** — the one thing morphe exists to prevent. morphe proves every format is layout-only (or up-to-spelling) before writing; a member input whose formatted output is not its equivalent is the paramount bug;
- a crash, hang, or unbounded resource use reachable from an admitted input — morphe aims to refuse a non-member as a typed diagnostic and to bound its own walks, never to abort;
- content from a `#script` body escaping its backend to disturb the surrounding ASP program, or an embedded failure that is not contained to *verbatim + a note*;
- the `reorder_includes` opt-in hoisting an `#include` without the warning that it rests on the caller's unverified assertion.

morphe pins `themelios-syntax` by exact git revision, and pins its embedded backends (`ruff`, `StyLua`) behind Cargo features; a report tied to a specific pinned version is welcome, and the pin's hardening is re-owed on any deliberate bump.
