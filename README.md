# m1-doc

Documentation generator for MoTeC M1 projects. Reads `Project.m1prj` and emits a
Markdown and HTML reference of the project's channels, parameters, constants, and
functions (with their inputs, return types, and `@m1:` annotations) — organised
by group.

## Usage

```sh
m1-doc --project path/to/Project.m1prj --out site --format both
```

`--project` defaults to the nearest `Project.m1prj` upward (or `$M1_PROJECT`).
`--format` is `markdown`, `html`, `both` (default), or `json`. Markdown is the
canonical output; HTML is rendered from it (so the two never diverge) into a
self-contained site.

**Scoped generation.** `--only-security Tune,Calibration` restricts the output to
symbols at the given access level(s); `--only-tag <tag>` restricts to symbols
carrying a tag. Combine them to intersect. A scoped run is symbol-centric — it
documents the matching channels/parameters/constants (and the group tree that
holds them) and omits functions, tables, objects and CAN, so you can produce, for
example, a calibration-focused subset for a given access level.

The HTML site is a single, dependency-free bundle (inline CSS/JS, no CDN, works
from `file://` and GitHub Pages): a project overview landing page (stats, target
hardware, group tree), a collapsible nav tree with an in-page table of contents
and hover permalinks, a responsive layout with dark mode, client-side search over
every symbol/function/table/enum, a security legend, and live filtering of rows
by security level and tag.

**Cross-references.** Each group page lists its `BuiltIn.Reference` aliases in a
`## References` table showing what each one points at (its `<Props Target>`). A
`This`/`Parent`/`Root`-relative or absolute target that names a documented symbol
is deep-linked, and the symbol gets a reverse `## Used by` entry so you can see
who consumes it. A target that doesn't resolve to a documented symbol is shown
verbatim rather than linked — the reference is never dropped and a link is never
invented (some firmware-supplied sensor references point at runtime-internal
values that the project file doesn't declare).

## Machine-readable JSON

`--format json` writes a single `m1-doc.json` — the whole `DocModel` as structured
data, the same information the Markdown and HTML renderers show. It is the
substrate for programmatic consumers: editor tooling, dashboards, doc-diffing,
external search, and CI checks (e.g. "does every tunable parameter declare a
unit?").

The document has a top-level integer `schema_version` (currently `1`); bump it
on any breaking shape change so consumers can gate on it. Output is
deterministic — generating twice yields a byte-identical file — with stable
object-key order and arrays in the loader's sorted order. Missing data is `null`,
never invented.

Top-level: `{ schema_version, title, target_hardware, groups[], enums[] }`. Each
group carries `path`, its `symbols`, `functions`, `tables`, `objects`,
`can_messages`, `references`, and the paths of its immediate `children`. A symbol
carries `path`, `anchor`, `kind` (`channel`/`parameter`/`constant`), `type_label`,
`quantity`, `unit`, `base_unit`, `log_rate_hz`, `security`, `enum_ref`,
`classname`, and `tags`; a function
carries its `inputs` (`{name, type}`), `return_type`, `annotations`,
`call_rate_hz`, and `source_path`; tables carry `axes` and `output_unit`; enums
carry `members`, `default`, and `open`; CAN messages carry `id`, `dlc`, and
their `signals`; a reference carries `path`, `anchor`, `target_raw`, and
`target_resolved` (the canonical symbol path when it resolves to one, else
`null`).

## Publishing to GitHub Pages

Copy [`examples/docs.yml`](examples/docs.yml) into your M1 project repo as
`.github/workflows/docs.yml` to build the HTML reference on every push and
publish it to GitHub Pages — an always-current reference of your vehicle code.

Part of the M1 toolchain — see https://github.com/C-Nucifora/m1-tools.
