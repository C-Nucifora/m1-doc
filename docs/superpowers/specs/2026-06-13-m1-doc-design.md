# m1-doc — design

Tracking issue: C-Nucifora/m1-tools#19.

## Purpose

Generate an always-current reference for a MoTeC M1 project from the project
itself. m1-doc walks `Project.m1prj` and the project's `.m1scr` scripts and emits
Markdown (canonical) and HTML, publishable to gh-pages, so a team has a live
reference of their vehicle code without hand-maintaining docs.

It is the first *documentation* consumer of m1-core's `@m1:` annotation
framework, which until now only fed diagnostics.

## Goals

- One command turns a project into browsable docs: channels, parameters,
  constants, and functions, organised by group.
- Surface each symbol's storage type, unit, security, and log/call rate, plus
  its `@m1:` annotations as structured metadata.
- Markdown is the single source of truth; HTML is rendered from it so the two
  cannot diverge.
- Self-contained Rust CLI mirroring m1-lint — no external static-site toolchain.
- CI-publishable to gh-pages via an example workflow consumers copy.

## Non-goals (v1)

- Editing or round-tripping the project (read-only).
- Diffing docs across revisions, search indexes, or theming beyond a minimal
  built-in stylesheet.
- A reusable GitHub Actions workflow — v1 ships an *example* only.
- Documenting script bodies line by line; m1-doc documents the project's
  *interface* (its symbols and their metadata), not the implementation.

## Architecture

Four units with one responsibility each, communicating through a plain in-memory
`DocModel`. Everything downstream of the loader reads the model, never the
toolchain directly.

```
Project.m1prj + *.m1scr
        │
        ▼
   ┌─────────┐     ┌──────────┐     ┌────────┐     ┌────────┐
   │ loader  │ ──▶ │ DocModel │ ──▶ │markdown│ ──▶ │  html  │
   └─────────┘     └──────────┘     └────────┘     └────────┘
   m1-typecheck      (the SSOT)      *.md +         render *.md
   + m1-core                         index.md       via pulldown-cmark
```

### 1. `loader`

- Loads `Project.m1prj` into `m1_typecheck::Project` → `SymbolTable` (channels,
  parameters, constants, enums, functions, with `path`, `kind`, `value_type`,
  `declared_type`, `unit`, `qty`, security, call/log rate).
- For each `.m1scr`: `m1_core::parse` → CST, then
  `m1_core::annotations(&cst, &m1_core::Registry::seed())` → `Annotations`.
- Associates annotations with the symbols/functions they decorate and builds the
  `DocModel`.

### 2. `DocModel` (single source of truth)

A plain data structure — no toolchain types leak past this boundary:

- `groups: Vec<GroupDoc>` keyed by top-level group (`Root.Engine`, …), each with
  nested child groups.
- `GroupDoc { path, channels, parameters, constants, functions }`.
- `SymbolDoc { path, kind, declared_type, value_type, unit, security, rate,
  annotations: Vec<AnnotationDoc> }`.
- `FunctionDoc { path, inputs, output, rate, annotations }` (Out type populated
  where the m1-typecheck symbol model exposes it; absent → omitted, not faked).
- `AnnotationDoc { kind, value, source_span }`.

### 3. `markdown` (canonical renderer)

- One file per top-level group (`<group>.md`) with sections for channels,
  parameters, constants, functions. Symbols render as a table
  (name · type · unit · security · rate) with annotations as a sub-list.
- `index.md` with a navigable tree of groups linking to each page.
- Deterministic ordering (sorted by path) so output is snapshot-testable and
  diff-stable across runs.

### 4. `html` (thin render layer)

- Renders each generated Markdown file via `pulldown-cmark` into a templated
  shell: a sidebar nav built from the same group tree, plus a minimal inline
  stylesheet. Zero external assets — every page is self-contained.
- Because it consumes the canonical Markdown, HTML and Markdown never diverge.

### `cli`

clap, mirroring m1-lint:

```
m1-doc [SCRIPTS]... --project <PROJECT> --out <DIR> --format <markdown|html|both> [--title <NAME>]
```

- `--project` defaults to the nearest `Project.m1prj` upward (or `$M1_PROJECT`),
  as the other tools do.
- `--out` directory for generated files (default `m1-doc/`).
- `--format` defaults to `both`.
- `--title` overrides the index heading. The m1-typecheck `Project` API does not
  expose the project's `Name` attribute, so the default is the project file's
  directory name (the closest available proxy).
- Exit codes mirror the sibling CLIs: 0 success, 1 generation error, 2 usage.

## Dependencies

New repo `C-Nucifora/m1-doc` depending on `m1-core v0.10.0` and
`m1-typecheck v0.35.0` as git-tag deps, plus `pulldown-cmark` and `clap`. Both
toolchain deps build against m1-core v0.10.0, keeping a single core version.
Standard repo furniture: README, AGENTS.md, and CI (test / clippy / fmt / MSRV)
mirroring the sibling repos.

## gh-pages publishing

`examples/docs.yml` — a workflow consumers copy into their project: run
`m1-doc --format html --out site`, publish `site/` to gh-pages. No mandatory
coupling to m1-ci; matches m1-ci's `examples/` ethos.

## Testing

- **Markdown snapshot tests** against small fixture projects — the canonical,
  human-readable output is the thing under test; snapshots make regressions
  obvious in review.
- **HTML smoke test** — generated HTML is well-formed, the nav is present, and
  intra-doc links resolve.
- **Corpus smoke test** — `m1-doc` over the EV-M1 corpus emits without error and
  produces a page for every top-level group (skips gracefully when the corpus is
  absent, like the sibling repos).

## Implementation phasing

The spec covers all of v1; the implementation plan builds it incrementally so
each phase is independently reviewable and shippable:

- **P1** — repo scaffold + CI; `loader` (project → SymbolTable) + `DocModel` +
  `markdown` for channels/parameters/constants; CLI with `--format markdown`.
- **P2** — functions (inputs, output where exposed) + `@m1:` annotations
  surfaced as metadata.
- **P3** — `html` renderer (pulldown-cmark + nav + inline CSS); `--format
  html|both`.
- **P4** — `examples/docs.yml` gh-pages workflow + README/AGENTS.

## Risks / open questions

- **Function Out type** depends on what the m1-typecheck symbol model exposes for
  user functions; if unavailable in v0.35.0, document inputs only and omit the
  return (tracked by m1-typecheck#110). No blocker — the model degrades, never
  fakes.
- **Annotation→symbol association**: annotations are parsed per-script against
  CST spans; mapping them to project symbols relies on the annotated declaration
  resolving to a known symbol path. Unresolvable annotations are listed under
  their script rather than dropped.
