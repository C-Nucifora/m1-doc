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
`--format` is `markdown`, `html`, or `both` (default). Markdown is the canonical
output; HTML is rendered from it (so the two never diverge) into a self-contained
site with a group sidebar.

## Publishing to GitHub Pages

Copy [`examples/docs.yml`](examples/docs.yml) into your M1 project repo as
`.github/workflows/docs.yml` to build the HTML reference on every push and
publish it to GitHub Pages — an always-current reference of your vehicle code.

Part of the M1 toolchain — see https://github.com/C-Nucifora/m1-tools.
