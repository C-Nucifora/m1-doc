//! Per-entity renderers for the members of a group page: functions, calibration
//! tables, package objects, CAN messages, and the symbol-table cell helpers
//! (type/tags/class/anchor columns). [`super::group`] composes these into the
//! full group page; nothing here knows about page structure.

use super::RenderOptions;
use super::helpers::{
    ENUMS_FILE, SourceLinker, fmt_f64, format_annotation, format_rate, last_segment, source_line,
};
use crate::model::{CanMessageDoc, FunctionDoc, ObjectDoc, SymbolDoc, SymbolDocKind, TableDoc};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Render one function entry as a `### <path>` subsection with its call rate,
/// input list, optional return type, source link, and, when present, an
/// `**Annotations:**` block. With `include_source` the script body is embedded
/// in a `<details>` block (collapsible, and a real code block in HTML).
pub(super) fn render_function(f: &FunctionDoc, opts: &RenderOptions) -> String {
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
        let body = body.trim_end();
        // Escalate the fence past the longest backtick run inside the body so an
        // embedded ``` (block comment, string literal, commented-out docs) can't
        // close the code block early and leak the rest of the script as markup.
        let longest_run = body
            .bytes()
            .fold((0usize, 0usize), |(cur, mx), b| {
                if b == b'`' {
                    (cur + 1, mx.max(cur + 1))
                } else {
                    (0, mx)
                }
            })
            .1;
        let fence = "`".repeat(longest_run.max(2) + 1);
        let _ = writeln!(out, "<details><summary>Source</summary>\n");
        let _ = writeln!(out, "{fence}m1\n{body}\n{fence}\n");
        let _ = writeln!(out, "</details>\n");
    }
    out
}

/// Render one calibration table entry: an anchored `### <path>` heading and a
/// dimensionality line — e.g. `2-D table — 16 (rpm) × 12 (kPa) → deg`. When the
/// shape is unknown (no `.m1cfg` loaded), say so rather than dropping the table.
pub(super) fn render_table(t: &TableDoc, links: &SourceLinker) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "<a id=\"{}\"></a>\n", t.anchor);
    let _ = writeln!(out, "### {}{}\n", t.path, links.suffix(t.def_line));
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
pub(super) fn section_shows_class(rows: &[&SymbolDoc], kind: SymbolDocKind) -> bool {
    let plain = plain_builtin(kind);
    rows.iter()
        .any(|s| s.classname.as_deref().is_some_and(|c| c != plain))
}

/// Whether a section's rows should carry a Tags column: true when any row is
/// tagged. Keeps untagged projects (the common case) uncluttered (#34).
pub(super) fn section_shows_tags(rows: &[&SymbolDoc]) -> bool {
    rows.iter().any(|s| !s.tags.is_empty())
}

/// The inline row anchor that also carries the security level and tags as
/// `data-` attributes so the HTML filter can show/hide the row by level or tag
/// (#34). The element is raw HTML that pulldown-cmark passes through verbatim
/// into the `<td>`, so the metadata reaches the rendered table without a
/// separate data channel. Tags are space-joined; an empty level/tag set is
/// simply absent. The `class="m1-row-anchor"` lets the filter JS find the row.
pub(super) fn row_anchor(s: &SymbolDoc) -> String {
    let mut attrs = format!(" id=\"{}\" class=\"m1-row-anchor\"", s.anchor);
    if let Some(sec) = &s.security {
        attrs.push_str(&format!(" data-security=\"{sec}\""));
    }
    if !s.tags.is_empty() {
        attrs.push_str(&format!(" data-tags=\"{}\"", s.tags.join(" ")));
    }
    format!("<a{attrs}></a>")
}

/// Render a symbol's Tags cell: each tag as inline code, or `—` when untagged.
pub(super) fn tags_cell(s: &SymbolDoc) -> String {
    if s.tags.is_empty() {
        "—".to_string()
    } else {
        s.tags
            .iter()
            .map(|t| format!("`{t}`"))
            .collect::<Vec<_>>()
            .join(" ")
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
pub(super) fn render_object(o: &ObjectDoc, links: &SourceLinker) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "<a id=\"{}\"></a>\n", o.anchor);
    let _ = writeln!(out, "### {}{}\n", o.path, links.suffix(o.def_line));
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
pub(super) fn render_can_message(m: &CanMessageDoc, links: &SourceLinker) -> String {
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
    let _ = writeln!(
        out,
        "### {} (id {id}, dlc {dlc}){}\n",
        m.path,
        links.suffix(m.def_line)
    );
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
pub(super) fn type_cell(s: &SymbolDoc, enum_anchors: &HashMap<&str, &str>) -> String {
    match &s.enum_ref {
        Some(name) => match enum_anchors.get(name.as_str()) {
            Some(anchor) => format!("[{}]({ENUMS_FILE}#{anchor})", s.type_label),
            None => s.type_label.clone(),
        },
        None => s.type_label.clone(),
    }
}
