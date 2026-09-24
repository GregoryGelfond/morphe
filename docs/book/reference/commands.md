# Using the morphe command

`morphe` is an opinionated formatter for Answer Set Programming. It formats the
clingo/clingcon dialect and the ASP-Core-2 standard to one fixed house style, and
it proves — before it writes — that it changed only the layout (see [How morphe
proves it didn't change your program](../guide/safety.md)).

## Formatting in place

`morphe <paths…>` formats files and directories **in place**, recursively, matching
`*.lp` and honoring `.gitignore`. Writes are **atomic** — morphe writes a temp file
beside the target and renames over it — so an interrupted run never truncates a
source.

```sh
morphe program.lp
morphe src/
```

## Checking and previewing

For CI and editors, two modes write nothing:

```sh
morphe --check program.lp   # exit 1 if it would change; writes nothing
morphe --diff program.lp    # print a unified diff; writes nothing
```

## Standard input

With `-`, morphe formats standard input to standard output, never in place:

```sh
cat program.lp | morphe -
```

`--stdin-filename <path>` gives stdin an assumed path, so config discovery,
dialect, and diagnostics behave as they would for that file — the editor hook.

## Options

| Option | Effect |
| --- | --- |
| `--check` | Exit non-zero if the input would change; write nothing. |
| `--diff` | Print a unified diff; write nothing. |
| `--line-width <n>` | The one style knob (default 100). |
| `--dialect <clingo\|asp-core-2>` | The ASP dialect (default clingo). |
| `--preserve-spellings` | Keep synonym spellings instead of normalizing them. |
| `--format-embedded` / `--no-format-embedded` | Reformat `#script` bodies with their language's tool, or keep them verbatim (the default). |
| `--reorder` | Apply the opt-in [reorder](../guide/reorder.md). |
| `--reorder-includes` | Also hoist `#include` (unsafe; implies `--reorder`). |
| `--exclude <glob>` / `--extend-exclude <glob>` | Skip matching files while walking a directory. |
| `--no-respect-gitignore` | Walk files `.gitignore` / `.ignore` would exclude. |
| `--stdin-filename <path>` | The assumed path for stdin. |
| `--config <path>` | Read this config instead of discovering a `morphe.toml`. |
| `--isolated` | Ignore any `morphe.toml`; use the built-in defaults and these flags alone. |
| `-q` / `--quiet`, `-v` / `--verbose` | Less, or more, per-file reporting. |
| `--color <auto\|always\|never>` | Terminal color (default auto; honors `NO_COLOR`). |

`morphe --help` lists them all. Configuration is a `morphe.toml` discovered by
walking up from each file; see [Configuration](../guide/configuration.md).

## Exit codes

| Code | Meaning |
| --- | --- |
| `0` | Clean — already formatted, or formatted in place. |
| `1` | Changes needed (under `--check` / `--diff`). |
| `2` | Usage error — bad arguments or a malformed `morphe.toml`; also a parse refusal. |
| `70` | Internal error — a bug in morphe; it never writes on this path. |
