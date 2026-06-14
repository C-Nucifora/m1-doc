//! Renders a [`DocModel`] to Markdown: one file per group plus an `index.md`.
//! This is the canonical output; the HTML renderer (P3) consumes these files.

use crate::model::{
    AnnotationDoc, CanMessageDoc, DocModel, EnumDoc, FunctionDoc, GroupDoc, ObjectDoc, SymbolDoc,
    SymbolDocKind, TableDoc,
};
use std::collections::HashMap;
use std::fmt::Write as _;

/// The filename of the project-wide Enums reference page.
const ENUMS_FILE: &str = "enums.md";

/// A rendered file: a project-relative path and its Markdown body.
pub struct RenderedFile {
    /// Project-relative path, e.g. `index.md` or `Root.Engine.md`.
    pub path: String,
    /// Full Markdown content ready to write to disk.
    pub body: String,
}

/// Render-time options driven by CLI flags (#30). `source_base`, when set, turns
/// a function's source path into an external link (e.g. a GitHub blob URL);
/// `include_source` embeds the script body in a collapsible block.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Base URL prepended to a function's `source_path` to build a source link
    /// (trailing slash optional). `None` → the path is shown as plain text.
    pub source_base: Option<String>,
    /// When `true`, embed each function's script body in a `<details>` block.
    pub include_source: bool,
}

/// `Root.Engine` -> `Root.Engine.md` (a flat, link-safe filename keyed by the
/// full group path, so every node in the tree has a distinct page).
fn group_filename(group_path: &str) -> String {
    format!("{group_path}.md")
}

/// The leaf segment of a dotted path (`Root.Engine.Fuel` -> `Fuel`) — the label
/// to show for a group in breadcrumbs and sub-group lists.
fn last_segment(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

/// The parent group of a path (everything before the last dot), or `""` for a
/// single-segment root.
fn parent_path(path: &str) -> &str {
    match path.rfind('.') {
        Some(i) => &path[..i],
        None => "",
    }
}

/// Render a `Root › Engine › Fuel` breadcrumb: every ancestor segment is a link
/// to its own page; the current (last) segment is plain text.
fn render_breadcrumb(path: &str) -> String {
    let segs: Vec<&str> = path.split('.').collect();
    let mut crumbs = Vec::with_capacity(segs.len());
    let mut cumulative = String::new();
    for (i, seg) in segs.iter().enumerate() {
        if i > 0 {
            cumulative.push('.');
        }
        cumulative.push_str(seg);
        if i + 1 < segs.len() {
            crumbs.push(format!("[{seg}]({})", group_filename(&cumulative)));
        } else {
            crumbs.push((*seg).to_string());
        }
    }
    crumbs.join(" › ")
}

/// Format one annotation as `@m1:<kind>(<args>)`, omitting the parens when
/// there are no args.
fn format_annotation(ann: &AnnotationDoc) -> String {
    if ann.args.is_empty() {
        format!("@m1:{}", ann.kind)
    } else {
        format!("@m1:{}({})", ann.kind, ann.args.join(", "))
    }
}

/// Format a rate in Hz for a table cell or field: `200 Hz`, `0.5 Hz`, or `—`
/// when absent. Trailing zeros are trimmed so integral rates read cleanly.
pub(crate) fn format_rate(hz: Option<f64>) -> String {
    match hz {
        None => "—".to_string(),
        Some(r) => {
            let s = format!("{r:.3}");
            let s = s.trim_end_matches('0').trim_end_matches('.');
            format!("{s} Hz")
        }
    }
}

/// Build a function's Source line. With a `source_base` the path becomes an
/// external link (`{base}/{path}`); without one it is shown verbatim so the
/// reader still sees which `.m1scr` implements the function (#30).
fn source_line(path: &str, base: Option<&str>) -> String {
    match base {
        Some(b) => {
            let b = b.trim_end_matches('/');
            format!("**Source:** [{path}]({b}/{path})\n")
        }
        None => format!("**Source:** `{path}`\n"),
    }
}

/// Render one function entry as a `### <path>` subsection with its call rate,
/// input list, optional return type, source link, and, when present, an
/// `**Annotations:**` block. With `include_source` the script body is embedded
/// in a `<details>` block (collapsible, and a real code block in HTML).
fn render_function(f: &FunctionDoc, opts: &RenderOptions) -> String {
    let mut out = String::new();
    // Explicit, deterministic anchor (our scheme — not pulldown-cmark's
    // incidental heading slug) so `<group>.md#<anchor>` is stable.
    let _ = writeln!(out, "<a id=\"{}\"></a>\n", f.anchor);
    let _ = writeln!(out, "### {}\n", f.path);
    let _ = writeln!(out, "**Call rate:** {}\n", format_rate(f.call_rate_hz));
    if let Some(src) = &f.source_path {
        let _ = writeln!(out, "{}", source_line(src, opts.source_base.as_deref()));
    }
    if f.inputs.is_empty() {
        let _ = writeln!(out, "(no inputs)\n");
    } else {
        for (name, ty) in &f.inputs {
            let _ = writeln!(out, "- {name}: {ty}");
        }
        out.push('\n');
    }
    if let Some(rt) = &f.return_type {
        let _ = writeln!(out, "**Returns:** {rt}\n");
    }
    if !f.annotations.is_empty() {
        let _ = writeln!(out, "**Annotations:**\n");
        for ann in &f.annotations {
            let _ = writeln!(out, "- {}", format_annotation(ann));
        }
        out.push('\n');
    }
    // Blank lines around the fence let pulldown-cmark treat <details>/</details>
    // as raw HTML blocks while still parsing the fence into a real code block —
    // collapsible in HTML, readable as Markdown (GitHub renders <details>).
    if opts.include_source
        && let Some(body) = &f.source_text
    {
        let _ = writeln!(out, "<details><summary>Source</summary>\n");
        let _ = writeln!(out, "```m1\n{}\n```\n", body.trim_end());
        let _ = writeln!(out, "</details>\n");
    }
    out
}

/// Render one calibration table entry: an anchored `### <path>` heading and a
/// dimensionality line — e.g. `2-D table — 16 (rpm) × 12 (kPa) → deg`. When the
/// shape is unknown (no `.m1cfg` loaded), say so rather than dropping the table.
fn render_table(t: &TableDoc) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "<a id=\"{}\"></a>\n", t.anchor);
    let _ = writeln!(out, "### {}\n", t.path);
    if t.axes.is_empty() {
        let _ = writeln!(out, "Table — shape requires a loaded `.m1cfg`\n");
    } else {
        let axes = t
            .axes
            .iter()
            .map(|a| match &a.unit {
                Some(u) => format!("{} ({u})", a.size),
                None => a.size.to_string(),
            })
            .collect::<Vec<_>>()
            .join(" × ");
        let output = t.output_unit.as_deref().unwrap_or("—");
        let _ = writeln!(out, "{}-D table — {axes} → {output}\n", t.axes.len());
    }
    out
}

/// The plain builtin classname expected for a documented symbol kind. A row
/// whose classname differs from this (or that has none of the expected form)
/// carries information worth showing — a sensor input, a generated IO method, a
/// calibration channel — so the Class column appears only then (#28).
fn plain_builtin(kind: SymbolDocKind) -> &'static str {
    match kind {
        SymbolDocKind::Channel => "BuiltIn.Channel",
        SymbolDocKind::Parameter => "BuiltIn.Parameter",
        SymbolDocKind::Constant => "BuiltIn.Constant",
    }
}

/// Whether a section's rows should carry a Class column: true when any row has a
/// classname that isn't the plain builtin for its kind. Keeps the common case
/// (every row a plain `BuiltIn.Channel`) uncluttered (#28).
fn section_shows_class(rows: &[&SymbolDoc], kind: SymbolDocKind) -> bool {
    let plain = plain_builtin(kind);
    rows.iter()
        .any(|s| s.classname.as_deref().is_some_and(|c| c != plain))
}

/// Format an `f64` for a CAN cell: trim trailing zeros so `0.50` → `0.5` and
/// `2.0` → `2`.
fn fmt_f64(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// Render a CAN signal's scale cell — `×0.5 +2`, `×1 +0`, or `—` when neither a
/// multiplier nor an offset is known.
fn scale_cell(s: &crate::model::CanSignalDoc) -> String {
    match (s.multiplier, s.offset) {
        (None, None) => "—".to_string(),
        (m, o) => {
            let mult = fmt_f64(m.unwrap_or(1.0));
            let off = o.unwrap_or(0.0);
            let sign = if off < 0.0 { "-" } else { "+" };
            format!("×{mult} {sign}{}", fmt_f64(off.abs()))
        }
    }
}

/// Render one package-object entry: an anchored `### <path>` heading, its class,
/// and a bullet list of its immediate members (#28).
fn render_object(o: &ObjectDoc) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "<a id=\"{}\"></a>\n", o.anchor);
    let _ = writeln!(out, "### {}\n", o.path);
    let _ = writeln!(out, "**Class:** {}\n", o.class.as_deref().unwrap_or("—"));
    if o.members.is_empty() {
        let _ = writeln!(out, "(no members)\n");
    } else {
        let _ = writeln!(out, "**Members:**\n");
        for m in &o.members {
            let _ = writeln!(out, "- `{m}`");
        }
        out.push('\n');
    }
    out
}

/// Render one CAN message: an anchored `### <path>` heading with its frame id
/// (hex) and dlc, then a per-signal table of bit layout, scale, range and unit.
/// A message with no signals is still listed (degrade, never drop) (#28).
fn render_can_message(m: &CanMessageDoc) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "<a id=\"{}\"></a>\n", m.anchor);
    let id = match m.can_id {
        Some(id) => format!("0x{id:X}"),
        None => "—".to_string(),
    };
    let dlc = m
        .dlc
        .map(|d| d.to_string())
        .unwrap_or_else(|| "—".to_string());
    let _ = writeln!(out, "### {} (id {id}, dlc {dlc})\n", m.path);
    if m.signals.is_empty() {
        let _ = writeln!(out, "(no signals)\n");
        return out;
    }
    let _ = writeln!(out, "| Signal | Bits | Scale | Range | Unit |");
    let _ = writeln!(out, "| --- | --- | --- | --- | --- |");
    for s in &m.signals {
        let bits = match (s.start_bit, s.length) {
            (Some(start), Some(len)) => format!("@{start}, {len}"),
            (Some(start), None) => format!("@{start}"),
            _ => "—".to_string(),
        };
        let range = match s.range {
            Some((lo, hi)) => format!("{} .. {}", fmt_f64(lo), fmt_f64(hi)),
            None => "—".to_string(),
        };
        let _ = writeln!(
            out,
            "| <a id=\"{}\"></a>`{}` | {} | {} | {} | {} |",
            s.anchor,
            last_segment(&s.path),
            bits,
            scale_cell(s),
            range,
            s.unit.as_deref().unwrap_or("—"),
        );
    }
    out.push('\n');
    out
}

/// Render a symbol's Type cell: an enum-typed symbol links to its entry in the
/// Enums reference; everything else is the plain type label.
fn type_cell(s: &SymbolDoc, enum_anchors: &HashMap<&str, &str>) -> String {
    match &s.enum_ref {
        Some(name) => match enum_anchors.get(name.as_str()) {
            Some(anchor) => format!("[{}]({ENUMS_FILE}#{anchor})", s.type_label),
            None => s.type_label.clone(),
        },
        None => s.type_label.clone(),
    }
}

fn render_group(
    group: &GroupDoc,
    enum_anchors: &HashMap<&str, &str>,
    opts: &RenderOptions,
) -> String {
    let mut out = String::new();
    // Breadcrumb of ancestor links, then the page heading.
    let _ = writeln!(out, "{}\n", render_breadcrumb(&group.path));
    let _ = writeln!(out, "# {}\n", group.path);
    // Sub-groups first so an intermediate (member-less) node is still navigable.
    if !group.children.is_empty() {
        let _ = writeln!(out, "## Sub-groups\n");
        for child in &group.children {
            let _ = writeln!(
                out,
                "- [{}]({})",
                last_segment(child),
                group_filename(child)
            );
        }
        out.push('\n');
    }
    for kind in [
        SymbolDocKind::Channel,
        SymbolDocKind::Parameter,
        SymbolDocKind::Constant,
    ] {
        let rows: Vec<&SymbolDoc> = group.symbols.iter().filter(|s| s.kind == kind).collect();
        if rows.is_empty() {
            continue;
        }
        // A Class column appears only when a row's class is not the plain
        // builtin — so sensor inputs / generated methods are disambiguated
        // without cluttering the common all-`BuiltIn.Channel` case (#28).
        let show_class = section_shows_class(&rows, kind);
        let class_h = if show_class { " Class |" } else { "" };
        let class_s = if show_class { " --- |" } else { "" };
        let _ = writeln!(out, "## {}\n", kind.plural());
        let _ = writeln!(
            out,
            "| Name | Type | Quantity | Unit | Base | Log rate | Security |{class_h}"
        );
        let _ = writeln!(out, "| --- | --- | --- | --- | --- | --- | --- |{class_s}");
        for s in rows {
            // Show the base unit only when it differs from the display unit —
            // collapse the redundant case (and when either is absent).
            let base = match (s.unit.as_deref(), s.base_unit.as_deref()) {
                (Some(disp), Some(base)) if disp != base => base,
                _ => "—",
            };
            let class_cell = if show_class {
                format!(" {} |", s.classname.as_deref().unwrap_or("—"))
            } else {
                String::new()
            };
            // Leading inline anchor in the Name cell makes the row linkable as
            // `<group>.md#<anchor>`; it carries into the HTML table verbatim.
            let _ = writeln!(
                out,
                "| <a id=\"{}\"></a>`{}` | {} | {} | {} | {} | {} | {} |{}",
                s.anchor,
                s.path,
                type_cell(s, enum_anchors),
                s.quantity.as_deref().unwrap_or("—"),
                s.unit.as_deref().unwrap_or("—"),
                base,
                format_rate(s.log_rate_hz),
                s.security.as_deref().unwrap_or("—"),
                class_cell,
            );
        }
        out.push('\n');
    }
    if !group.tables.is_empty() {
        let _ = writeln!(out, "## Tables\n");
        for t in &group.tables {
            out.push_str(&render_table(t));
        }
    }
    if !group.objects.is_empty() {
        let _ = writeln!(out, "## Objects\n");
        for o in &group.objects {
            out.push_str(&render_object(o));
        }
    }
    if !group.can_messages.is_empty() {
        let _ = writeln!(out, "## CAN\n");
        for m in &group.can_messages {
            out.push_str(&render_can_message(m));
        }
    }
    if !group.functions.is_empty() {
        let _ = writeln!(out, "## Functions\n");
        for f in &group.functions {
            out.push_str(&render_function(f, opts));
        }
    }
    out
}

fn render_index(model: &DocModel) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", model.title);
    let _ = writeln!(out, "## Groups\n");
    // List only the forest-root groups (those whose parent is not itself a
    // documented group); the full tree is reachable by descending from them.
    let present: std::collections::HashSet<&str> =
        model.groups.iter().map(|g| g.path.as_str()).collect();
    for g in &model.groups {
        let parent = parent_path(&g.path);
        if parent.is_empty() || !present.contains(parent) {
            let _ = writeln!(out, "- [{}]({})", g.path, group_filename(&g.path));
        }
    }
    if !model.enums.is_empty() {
        let _ = writeln!(out, "\n## Reference\n");
        let _ = writeln!(out, "- [Enums]({ENUMS_FILE})");
    }
    out
}

/// Render the project-wide Enums reference page: each enum is an anchored
/// section listing its enumerators (container order), default, and open flag.
/// An `open` (firmware) enum is labelled so its member list reads as partial.
fn render_enums(enums: &[EnumDoc]) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# Enums\n");
    for e in enums {
        let _ = writeln!(out, "<a id=\"{}\"></a>\n", e.anchor);
        let default = e.default.as_deref().unwrap_or("—");
        if e.open {
            let _ = writeln!(
                out,
                "## {} (open — firmware-supplied, members may be partial; default: {default})\n",
                e.name
            );
        } else {
            let _ = writeln!(out, "## {} (default: {default})\n", e.name);
        }
        if e.members.is_empty() {
            let _ = writeln!(out, "(no enumerators available)\n");
        } else {
            for m in &e.members {
                let _ = writeln!(out, "- {m}");
            }
            out.push('\n');
        }
    }
    out
}

/// Render the whole model with default options (no source links, no embedded
/// source). Convenience wrapper over [`render_with`] — the binary always passes
/// explicit options, so this is used by the tests and the HTML test harness.
#[cfg(test)]
pub fn render(model: &DocModel) -> Vec<RenderedFile> {
    render_with(model, &RenderOptions::default())
}

/// Render the whole model. Always emits `index.md` first, then one file per
/// group in model order (already sorted by the loader), then the Enums
/// reference page when the project uses any enums. `opts` controls function
/// source links and embedding (#30).
pub fn render_with(model: &DocModel, opts: &RenderOptions) -> Vec<RenderedFile> {
    // name -> anchor for linking enum-typed symbols to the reference.
    let enum_anchors: HashMap<&str, &str> = model
        .enums
        .iter()
        .map(|e| (e.name.as_str(), e.anchor.as_str()))
        .collect();
    let mut files = vec![RenderedFile {
        path: "index.md".to_string(),
        body: render_index(model),
    }];
    for g in &model.groups {
        files.push(RenderedFile {
            path: group_filename(&g.path),
            body: render_group(g, &enum_anchors, opts),
        });
    }
    if !model.enums.is_empty() {
        files.push(RenderedFile {
            path: ENUMS_FILE.to_string(),
            body: render_enums(&model.enums),
        });
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        DocModel, EnumDoc, FunctionDoc, GroupDoc, SymbolDoc, SymbolDocKind, TableAxisDoc, TableDoc,
    };

    fn sample() -> DocModel {
        DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    unit: Some("rpm".into()),
                    security: None,
                    ..Default::default()
                }],
                functions: vec![],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                children: vec![],
            }],
        }
    }

    fn sample_with_functions() -> DocModel {
        DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![],
                functions: vec![
                    FunctionDoc {
                        path: "Root.Engine.Reset".into(),
                        inputs: vec![],
                        return_type: None,
                        annotations: vec![],
                        ..Default::default()
                    },
                    FunctionDoc {
                        path: "Root.Engine.Update".into(),
                        inputs: vec![
                            ("Timeout".to_string(), "float".to_string()),
                            ("Enable".to_string(), "bool".to_string()),
                        ],
                        return_type: None,
                        annotations: vec![],
                        ..Default::default()
                    },
                ],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                children: vec![],
            }],
        }
    }

    #[test]
    fn index_links_each_group() {
        let files = render(&sample());
        let index = &files[0];
        assert_eq!(index.path, "index.md");
        assert!(
            index.body.contains("[Root.Engine](Root.Engine.md)"),
            "got:\n{}",
            index.body
        );
    }

    #[test]
    fn group_page_tables_its_channels() {
        let files = render(&sample());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(page.body.contains("## Channels"), "got:\n{}", page.body);
        assert!(
            page.body
                .contains("`Root.Engine.Speed` | f32 | — | rpm | — | — | — |"),
            "got:\n{}",
            page.body
        );
    }

    fn sample_with_constant() -> DocModel {
        DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.MaxRpm".into(),
                    kind: SymbolDocKind::Constant,
                    type_label: "u16".into(),
                    unit: None,
                    security: None,
                    ..Default::default()
                }],
                functions: vec![],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                children: vec![],
            }],
        }
    }

    /// A group containing a Constant symbol must render a `## Constants` section
    /// and include the constant's row in the table. Removing the
    /// `SymbolDocKind::Constant` branch from `render_group` would cause this test
    /// to fail.
    #[test]
    fn group_page_tables_its_constants() {
        let files = render(&sample_with_constant());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("## Constants"),
            "expected Constants section; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("`Root.Engine.MaxRpm` | u16 | — | — |"),
            "expected constant row; got:\n{}",
            page.body
        );
        // Channels and Parameters sections must be absent when there are none.
        assert!(
            !page.body.contains("## Channels"),
            "must not emit Channels when there are none; got:\n{}",
            page.body
        );
        assert!(
            !page.body.contains("## Parameters"),
            "must not emit Parameters when there are none; got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_with_no_functions_omits_functions_section() {
        let files = render(&sample());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            !page.body.contains("## Functions"),
            "must not emit Functions section when there are none; got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_renders_functions_section() {
        let files = render(&sample_with_functions());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("## Functions"),
            "missing Functions section; got:\n{}",
            page.body
        );
        // Function with no inputs shows "(no inputs)".
        assert!(
            page.body.contains("### Root.Engine.Reset"),
            "missing Reset heading; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("(no inputs)"),
            "missing (no inputs) for Reset; got:\n{}",
            page.body
        );
        // Function with inputs lists each param as "- name: type".
        assert!(
            page.body.contains("### Root.Engine.Update"),
            "missing Update heading; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- Timeout: float"),
            "missing Timeout param; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- Enable: bool"),
            "missing Enable param; got:\n{}",
            page.body
        );
    }

    #[test]
    fn function_with_annotations_renders_annotation_list() {
        use crate::model::AnnotationDoc;
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![],
                functions: vec![FunctionDoc {
                    path: "Root.Engine.Update".into(),
                    inputs: vec![],
                    return_type: None,
                    annotations: vec![
                        AnnotationDoc {
                            kind: "requires-finite".into(),
                            args: vec![],
                        },
                        AnnotationDoc {
                            kind: "allow".into(),
                            args: vec!["L010".into()],
                        },
                    ],
                    ..Default::default()
                }],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                children: vec![],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("**Annotations:**"),
            "missing Annotations label; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- @m1:requires-finite"),
            "missing requires-finite annotation; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- @m1:allow(L010)"),
            "missing allow(L010) annotation; got:\n{}",
            page.body
        );
    }

    #[test]
    fn function_without_annotations_omits_annotation_section() {
        let files = render(&sample_with_functions());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            !page.body.contains("**Annotations:**"),
            "must not emit Annotations when there are none; got:\n{}",
            page.body
        );
    }

    #[test]
    fn function_with_return_type_renders_returns_line() {
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![],
                functions: vec![FunctionDoc {
                    path: "Root.Engine.Compute".into(),
                    inputs: vec![("X".to_string(), "float".to_string())],
                    return_type: Some("float".to_string()),
                    annotations: vec![],
                    ..Default::default()
                }],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                children: vec![],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("**Returns:** float"),
            "missing Returns line; got:\n{}",
            page.body
        );
    }

    #[test]
    fn function_without_return_type_omits_returns_line() {
        let files = render(&sample_with_functions());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            !page.body.contains("**Returns:**"),
            "must not emit Returns when return_type is None; got:\n{}",
            page.body
        );
    }

    // ---- #25: rate / quantity / base-vs-display-unit surfacing ----

    #[test]
    fn format_rate_trims_trailing_zeros_and_handles_none() {
        assert_eq!(format_rate(Some(200.0)), "200 Hz");
        assert_eq!(format_rate(Some(0.5)), "0.5 Hz");
        assert_eq!(format_rate(Some(12.25)), "12.25 Hz");
        assert_eq!(format_rate(None), "—");
    }

    #[test]
    fn group_table_shows_quantity_log_rate_and_base_only_when_it_differs() {
        // Display unit (rpm) differs from the stored base (rad/s) → both shown;
        // the channel carries a quantity and a log rate.
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![
                    SymbolDoc {
                        path: "Root.Engine.Speed".into(),
                        kind: SymbolDocKind::Channel,
                        type_label: "f32".into(),
                        quantity: Some("rad/s".into()),
                        unit: Some("rpm".into()),
                        base_unit: Some("rad/s".into()),
                        log_rate_hz: Some(200.0),
                        security: None,
                        ..Default::default()
                    },
                    // Display == base → Base column collapses to "—".
                    SymbolDoc {
                        path: "Root.Engine.Load".into(),
                        kind: SymbolDocKind::Channel,
                        type_label: "f32".into(),
                        quantity: None,
                        unit: Some("%".into()),
                        base_unit: Some("%".into()),
                        log_rate_hz: None,
                        security: None,
                        ..Default::default()
                    },
                ],
                functions: vec![],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                children: vec![],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body
                .contains("`Root.Engine.Speed` | f32 | rad/s | rpm | rad/s | 200 Hz | — |"),
            "rate/quantity/base not surfaced; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("`Root.Engine.Load` | f32 | — | % | — | — | — |"),
            "base must collapse when identical to display; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("| Name | Type | Quantity | Unit | Base | Log rate | Security |"),
            "table header missing new columns; got:\n{}",
            page.body
        );
    }

    #[test]
    fn enums_reference_lists_closed_enum_with_members_and_default(/* #27 */) {
        let model = DocModel {
            title: "Demo".into(),
            groups: vec![],
            enums: vec![EnumDoc {
                name: "MoTeC Types.Switch".into(),
                anchor: "motec-types-switch".into(),
                members: vec!["Off".into(), "On".into()],
                default: Some("Off".into()),
                open: false,
            }],
        };
        let files = render(&model);
        let page = files
            .iter()
            .find(|f| f.path == "enums.md")
            .expect("enums.md should be emitted");
        assert!(
            page.body.contains("## MoTeC Types.Switch (default: Off)"),
            "closed enum heading wrong; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("<a id=\"motec-types-switch\"></a>")
                && page.body.contains("- Off")
                && page.body.contains("- On"),
            "members/anchor missing; got:\n{}",
            page.body
        );
        // The index links the reference.
        let index = &files[0];
        assert!(
            index.body.contains("[Enums](enums.md)"),
            "index should link the enums reference; got:\n{}",
            index.body
        );
    }

    #[test]
    fn open_enum_is_labelled_partial(/* #27 */) {
        let model = DocModel {
            title: "Demo".into(),
            groups: vec![],
            enums: vec![EnumDoc {
                name: "Gear State".into(),
                anchor: "gear-state".into(),
                members: vec!["Neutral".into()],
                default: None,
                open: true,
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "enums.md").unwrap();
        assert!(
            page.body
                .contains("open — firmware-supplied, members may be partial"),
            "open enum must be labelled partial; got:\n{}",
            page.body
        );
    }

    #[test]
    fn enum_typed_symbol_links_to_its_reference_entry(/* #27 */) {
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![EnumDoc {
                name: "Switch".into(),
                anchor: "switch".into(),
                members: vec!["Off".into(), "On".into()],
                default: Some("Off".into()),
                open: false,
            }],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Mode".into(),
                    anchor: "root-engine-mode".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "Switch".into(),
                    enum_ref: Some("Switch".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("[Switch](enums.md#switch)"),
            "enum-typed symbol must link to its reference; got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_renders_tables_section_with_dimensionality(/* #26 */) {
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                tables: vec![TableDoc {
                    path: "Root.Engine.IgnitionMap".into(),
                    anchor: "root-engine-ignitionmap".into(),
                    axes: vec![
                        TableAxisDoc {
                            size: 16,
                            unit: Some("rpm".into()),
                        },
                        TableAxisDoc {
                            size: 12,
                            unit: Some("kPa".into()),
                        },
                    ],
                    output_unit: Some("deg".into()),
                }],
                ..Default::default()
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(page.body.contains("## Tables"), "got:\n{}", page.body);
        assert!(
            page.body.contains("### Root.Engine.IgnitionMap"),
            "got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("2-D table — 16 (rpm) × 12 (kPa) → deg"),
            "dimensionality line wrong; got:\n{}",
            page.body
        );
        // Tables are anchored like every other entity (#24).
        assert!(
            page.body.contains("<a id=\"root-engine-ignitionmap\"></a>"),
            "table anchor missing; got:\n{}",
            page.body
        );
    }

    #[test]
    fn table_without_cfg_metadata_is_still_listed(/* #26 */) {
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                tables: vec![TableDoc {
                    path: "Root.Engine.FuelMap".into(),
                    anchor: "root-engine-fuelmap".into(),
                    axes: vec![],
                    output_unit: None,
                }],
                ..Default::default()
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("### Root.Engine.FuelMap")
                && page.body.contains("shape requires a loaded `.m1cfg`"),
            "unshaped table must still be listed; got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_has_breadcrumb_and_subgroup_links(/* #23 */) {
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine.Fuel".into(),
                children: vec!["Root.Engine.Fuel.Pump".into()],
                ..Default::default()
            }],
        };
        let files = render(&model);
        let page = files
            .iter()
            .find(|f| f.path == "Root.Engine.Fuel.md")
            .unwrap();
        // Breadcrumb: ancestors are links, the current segment is plain.
        assert!(
            page.body
                .contains("[Root](Root.md) › [Engine](Root.Engine.md) › Fuel"),
            "breadcrumb wrong; got:\n{}",
            page.body
        );
        // Sub-groups section links each child by its leaf label.
        assert!(
            page.body.contains("## Sub-groups")
                && page.body.contains("[Pump](Root.Engine.Fuel.Pump.md)"),
            "sub-groups missing; got:\n{}",
            page.body
        );
    }

    #[test]
    fn index_links_only_forest_roots_not_every_node(/* #23 */) {
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![
                GroupDoc {
                    path: "Root".into(),
                    children: vec!["Root.Engine".into()],
                    ..Default::default()
                },
                GroupDoc {
                    path: "Root.Engine".into(),
                    ..Default::default()
                },
            ],
        };
        let files = render(&model);
        let index = &files[0];
        assert!(
            index.body.contains("[Root](Root.md)"),
            "got:\n{}",
            index.body
        );
        // Root.Engine is reachable by descending, not listed at the index top level.
        assert!(
            !index.body.contains("[Root.Engine](Root.Engine.md)"),
            "index must not flat-list child groups; got:\n{}",
            index.body
        );
    }

    #[test]
    fn rows_and_functions_emit_their_stable_anchor(/* #24 */) {
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    anchor: "root-engine-speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    ..Default::default()
                }],
                functions: vec![FunctionDoc {
                    path: "Root.Engine.Update".into(),
                    anchor: "root-engine-update".into(),
                    ..Default::default()
                }],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                children: vec![],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        // Symbol row carries a leading inline anchor → `Root.Engine.md#root-engine-speed`.
        assert!(
            page.body
                .contains("| <a id=\"root-engine-speed\"></a>`Root.Engine.Speed`"),
            "symbol row missing its anchor; got:\n{}",
            page.body
        );
        // Function uses our explicit anchor, not pulldown-cmark's heading slug.
        assert!(
            page.body.contains("<a id=\"root-engine-update\"></a>"),
            "function missing its anchor; got:\n{}",
            page.body
        );
    }

    #[test]
    fn function_renders_call_rate_and_dash_when_absent() {
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![],
                functions: vec![
                    FunctionDoc {
                        path: "Root.Engine.Update".into(),
                        call_rate_hz: Some(100.0),
                        ..Default::default()
                    },
                    FunctionDoc {
                        path: "Root.Engine.Init".into(),
                        call_rate_hz: None,
                        ..Default::default()
                    },
                ],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                children: vec![],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("### Root.Engine.Update")
                && page.body.contains("**Call rate:** 100 Hz"),
            "triggered function must show its call rate; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("**Call rate:** —"),
            "untriggered function must show — ; got:\n{}",
            page.body
        );
    }

    // ---- #28: objects, CAN, classname column ----

    #[test]
    fn group_page_renders_objects_with_class_and_members() {
        use crate::model::ObjectDoc;
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Inputs".into(),
                objects: vec![ObjectDoc {
                    path: "Root.Inputs.OilP".into(),
                    anchor: "root-inputs-oilp".into(),
                    class: Some("MoTeC Input.Sensor".into()),
                    members: vec![
                        "Root.Inputs.OilP.Calibration".into(),
                        "Root.Inputs.OilP.Resource".into(),
                    ],
                }],
                ..Default::default()
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Inputs.md").unwrap();
        assert!(page.body.contains("## Objects"), "got:\n{}", page.body);
        assert!(
            page.body.contains("### Root.Inputs.OilP")
                && page.body.contains("**Class:** MoTeC Input.Sensor"),
            "object class missing; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- `Root.Inputs.OilP.Resource`"),
            "object members missing; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("<a id=\"root-inputs-oilp\"></a>"),
            "object anchor missing; got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_renders_can_message_and_signal_layout() {
        use crate::model::{CanMessageDoc, CanSignalDoc};
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Bus".into(),
                can_messages: vec![CanMessageDoc {
                    path: "Bus.EngineData".into(),
                    anchor: "bus-enginedata".into(),
                    can_id: Some(160),
                    dlc: Some(8),
                    signals: vec![CanSignalDoc {
                        path: "Bus.EngineData.EngineSpeed".into(),
                        anchor: "bus-enginedata-enginespeed".into(),
                        start_bit: Some(24),
                        length: Some(16),
                        multiplier: Some(0.5),
                        offset: Some(0.0),
                        range: Some((0.0, 8000.0)),
                        unit: Some("rpm".into()),
                    }],
                }],
                ..Default::default()
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Bus.md").unwrap();
        assert!(page.body.contains("## CAN"), "got:\n{}", page.body);
        // Frame id is shown in hex with the dlc.
        assert!(
            page.body.contains("### Bus.EngineData (id 0xA0, dlc 8)"),
            "message frame line wrong; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("| Signal | Bits | Scale | Range | Unit |"),
            "signal table header missing; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("`EngineSpeed` | @24, 16 | ×0.5 +0 | 0 .. 8000 | rpm |"),
            "signal row wrong; got:\n{}",
            page.body
        );
    }

    #[test]
    fn can_message_with_no_signals_is_still_listed() {
        use crate::model::CanMessageDoc;
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Bus".into(),
                can_messages: vec![CanMessageDoc {
                    path: "Bus.Empty".into(),
                    anchor: "bus-empty".into(),
                    can_id: None,
                    dlc: None,
                    signals: vec![],
                }],
                ..Default::default()
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Bus.md").unwrap();
        assert!(
            page.body.contains("### Bus.Empty (id —, dlc —)") && page.body.contains("(no signals)"),
            "empty message must degrade, not drop; got:\n{}",
            page.body
        );
    }

    #[test]
    fn class_column_appears_only_for_non_plain_classnames() {
        // One plain BuiltIn.Channel and one sensor-resource channel
        // (MoTeC Input.Sensor.Resource) — the section must show a Class column.
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Inputs".into(),
                symbols: vec![
                    SymbolDoc {
                        path: "Root.Inputs.Plain".into(),
                        kind: SymbolDocKind::Channel,
                        type_label: "f32".into(),
                        classname: Some("BuiltIn.Channel".into()),
                        ..Default::default()
                    },
                    SymbolDoc {
                        path: "Root.Inputs.Sensed".into(),
                        kind: SymbolDocKind::Channel,
                        type_label: "f32".into(),
                        classname: Some("BuiltIn.ChannelCalibratable".into()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Inputs.md").unwrap();
        assert!(
            page.body
                .contains("| Name | Type | Quantity | Unit | Base | Log rate | Security | Class |"),
            "Class column header missing; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("BuiltIn.ChannelCalibratable |"),
            "non-plain class must be shown; got:\n{}",
            page.body
        );
    }

    // ---- #30: function source links + embedding ----

    fn fn_with_source() -> FunctionDoc {
        FunctionDoc {
            path: "Root.Engine.Update".into(),
            anchor: "root-engine-update".into(),
            call_rate_hz: Some(100.0),
            source_path: Some("Engine/Update.m1scr".into()),
            source_text: Some("Out = In.Speed * 2;\n".into()),
            ..Default::default()
        }
    }

    #[test]
    fn function_source_path_shown_plain_without_base() {
        let out = render_function(&fn_with_source(), &RenderOptions::default());
        assert!(
            out.contains("**Source:** `Engine/Update.m1scr`"),
            "plain source path missing; got:\n{out}"
        );
    }

    #[test]
    fn function_source_path_becomes_link_with_base() {
        let opts = RenderOptions {
            source_base: Some("https://github.com/UQRacing/EV-M1/blob/main/".into()),
            include_source: false,
        };
        let out = render_function(&fn_with_source(), &opts);
        assert!(
            out.contains(
                "**Source:** [Engine/Update.m1scr](https://github.com/UQRacing/EV-M1/blob/main/Engine/Update.m1scr)"
            ),
            "source link (trailing slash trimmed) wrong; got:\n{out}"
        );
    }

    #[test]
    fn include_source_embeds_collapsible_body() {
        let opts = RenderOptions {
            source_base: None,
            include_source: true,
        };
        let out = render_function(&fn_with_source(), &opts);
        assert!(
            out.contains("<details><summary>Source</summary>")
                && out.contains("```m1\nOut = In.Speed * 2;\n```")
                && out.contains("</details>"),
            "include_source must embed a collapsible code block; got:\n{out}"
        );
    }

    #[test]
    fn source_off_by_default_no_details() {
        let out = render_function(&fn_with_source(), &RenderOptions::default());
        assert!(
            !out.contains("<details>"),
            "source body must not embed unless --include-source; got:\n{out}"
        );
    }

    #[test]
    fn function_without_source_path_omits_source_line() {
        let f = FunctionDoc {
            path: "Root.Engine.NoFile".into(),
            anchor: "root-engine-nofile".into(),
            ..Default::default()
        };
        let out = render_function(&f, &RenderOptions::default());
        assert!(
            !out.contains("**Source:**"),
            "no source path → no Source line; got:\n{out}"
        );
    }

    #[test]
    fn class_column_absent_when_all_plain() {
        // Every channel is a plain BuiltIn.Channel → no Class column (no clutter).
        let model = DocModel {
            title: "Demo".into(),
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    classname: Some("BuiltIn.Channel".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body
                .contains("| Name | Type | Quantity | Unit | Base | Log rate | Security |\n"),
            "default header must be unchanged; got:\n{}",
            page.body
        );
        assert!(
            !page.body.contains("| Class |"),
            "Class column must be absent when all plain; got:\n{}",
            page.body
        );
    }
}
