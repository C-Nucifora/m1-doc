# AGENTS.md — m1-doc

Documentation generator for MoTeC M1 projects.

## Architecture

`loader` (m1-typecheck `Project` → `SymbolTable`, m1-core annotations (P2)) builds a
plain `DocModel` (the single source of truth). `markdown` renders it to files —
the canonical output. `html` (P3) renders the Markdown via pulldown-cmark.
Nothing downstream of the loader touches toolchain types.

## The data contract

- Symbols and annotations come from m1-typecheck / m1-core — never re-parse
  `.m1prj` or `.m1scr` here.
- `DocModel` is plain data; no toolchain type crosses it.
- Markdown is canonical; HTML is rendered from it so the two cannot diverge.

## Build / test gate

```sh
cargo test
cargo clippy --all-targets -- -D warnings
cargo fmt --all -- --check
```

Loader tests use `Project::from_xml`; the CLI test uses `assert_cmd`. A corpus
smoke test (P-later) runs over EV-M1 when present (`$M1_CORPUS_PATH`).

## Releases

Cut on a `Cargo.toml` version bump landing on `main` (binary repos upload
per-platform binaries). Depends on m1-core and m1-typecheck at pinned tags — bump
both together to keep a single m1-core version.
