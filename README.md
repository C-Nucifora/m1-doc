# m1-doc

Documentation generator for MoTeC M1 projects. Reads `Project.m1prj` and emits a
Markdown (and, from P3, HTML) reference of the project's channels, parameters,
constants, and functions — organised by group.

## Usage

```sh
m1-doc --project path/to/Project.m1prj --out site --format markdown
```

`--project` defaults to the nearest `Project.m1prj` upward (or `$M1_PROJECT`).
`--format` is `markdown`, `html`, or `both` (default).

Part of the M1 toolchain — see https://github.com/C-Nucifora/m1-tools.
