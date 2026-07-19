//! The per-group page: breadcrumb, relationship graph, sub-group list, the
//! channel/parameter/constant symbol tables, and the tables/objects/CAN/function
//! sections — plus the cross-reference machinery (`## References` / `## Used by`)
//! built once from the whole model. Composes [`super::entries`] and
//! [`super::helpers`].

use super::RenderOptions;
use super::entries::{
    render_can_message, render_function, render_object, render_table, row_anchor,
    section_shows_class, section_shows_tags, tags_cell, type_cell,
};
use super::helpers::{
    SourceLinker, emit_graph_block, format_rate, group_filename, last_segment, render_breadcrumb,
};
use crate::diagram::Diagram;
use crate::model::{DocModel, GroupDoc, ProjectGraph, SymbolDoc, SymbolDocKind};
use std::collections::HashMap;
use std::fmt::Write as _;

/// Depth of the per-group relationship graph embedded on each group page: the
/// group's own members plus their immediate (one-hop) neighbours, so the wiring
/// reads in context without pulling in the whole project (#37).
const GROUP_GRAPH_DEPTH: usize = 1;

pub(super) fn render_group(
    group: &GroupDoc,
    enum_anchors: &HashMap<&str, &str>,
    xrefs: &CrossRefs,
    graph: &ProjectGraph,
    opts: &RenderOptions,
    links: &SourceLinker,
) -> String {
    let mut out = String::new();
    // Breadcrumb of ancestor links, then the page heading.
    let _ = writeln!(out, "{}\n", render_breadcrumb(&group.path));
    let _ = writeln!(out, "# {}\n", group.path);
    // Relationship graph first (#37): an at-a-glance picture of what this
    // group's members call and read/write, before the detailed tables.
    render_relationships(&mut out, group, graph);
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
        // without cluttering the common all-`BuiltIn.Channel` case (#28). A
        // Tags column appears only when some row is tagged (#34). Tags sit
        // before Class so the common (#28) `Security | Class |` shape is intact.
        let show_class = section_shows_class(&rows, kind);
        let show_tags = section_shows_tags(&rows);
        let tags_h = if show_tags { " Tags |" } else { "" };
        let tags_s = if show_tags { " --- |" } else { "" };
        let class_h = if show_class { " Class |" } else { "" };
        let class_s = if show_class { " --- |" } else { "" };
        let _ = writeln!(out, "## {}\n", kind.plural());
        let _ = writeln!(
            out,
            "| Name | Type | Quantity | Unit | Base | Log rate | Security |{tags_h}{class_h}"
        );
        let _ = writeln!(
            out,
            "| --- | --- | --- | --- | --- | --- | --- |{tags_s}{class_s}"
        );
        for s in rows {
            // Show the base unit only when it differs from the display unit —
            // collapse the redundant case (and when either is absent).
            let base = match (s.unit.as_deref(), s.base_unit.as_deref()) {
                (Some(disp), Some(base)) if disp != base => base,
                _ => "—",
            };
            let tags_col = if show_tags {
                format!(" {} |", tags_cell(s))
            } else {
                String::new()
            };
            let class_cell = if show_class {
                format!(" {} |", s.classname.as_deref().unwrap_or("—"))
            } else {
                String::new()
            };
            // Leading inline anchor in the Name cell makes the row linkable as
            // `<group>.md#<anchor>`; it also carries the security/tags filter
            // metadata as data attributes for the HTML filter (#34). It passes
            // into the HTML table verbatim. A trailing `[src]` deep-links the
            // symbol's declaration in the `.m1prj` when a source base is set
            // (#57) — absent (and the cell unchanged) otherwise.
            let _ = writeln!(
                out,
                "| {}`{}`{} | {} | {} | {} | {} | {} | {} |{}{}",
                row_anchor(s),
                s.path,
                links.suffix(s.def_line),
                type_cell(s, enum_anchors),
                s.quantity.as_deref().unwrap_or("—"),
                s.unit.as_deref().unwrap_or("—"),
                base,
                format_rate(s.log_rate_hz),
                s.security.as_deref().unwrap_or("—"),
                tags_col,
                class_cell,
            );
        }
        out.push('\n');
    }
    if !group.tables.is_empty() {
        let _ = writeln!(out, "## Tables\n");
        for t in &group.tables {
            out.push_str(&render_table(t, links));
        }
    }
    if !group.objects.is_empty() {
        let _ = writeln!(out, "## Objects\n");
        for o in &group.objects {
            out.push_str(&render_object(o, links));
        }
    }
    if !group.can_messages.is_empty() {
        let _ = writeln!(out, "## CAN\n");
        for m in &group.can_messages {
            out.push_str(&render_can_message(m, links));
        }
    }
    if !group.functions.is_empty() {
        let _ = writeln!(out, "## Functions\n");
        for f in &group.functions {
            out.push_str(&render_function(f, opts));
        }
    }
    render_references(&mut out, group, xrefs, links);
    render_used_by(&mut out, group, xrefs);
    out
}

/// Cross-reference link tables built once from the whole model (#29): where each
/// symbol and reference lives (page filename + anchor), so a target or a
/// who-references entry can be turned into a deep link, plus the inverse
/// used-by map (resolved target symbol path → the references that point at it).
pub(super) struct CrossRefs<'a> {
    symbol_loc: HashMap<&'a str, (String, &'a str)>,
    reference_loc: HashMap<&'a str, (String, &'a str)>,
    used_by: HashMap<&'a str, Vec<&'a str>>,
}

/// Build the [`CrossRefs`] tables from every group's symbols and references.
pub(super) fn build_cross_refs(model: &DocModel) -> CrossRefs<'_> {
    let mut symbol_loc: HashMap<&str, (String, &str)> = HashMap::new();
    let mut reference_loc: HashMap<&str, (String, &str)> = HashMap::new();
    let mut used_by: HashMap<&str, Vec<&str>> = HashMap::new();
    for g in &model.groups {
        let file = group_filename(&g.path);
        for s in &g.symbols {
            symbol_loc.insert(s.path.as_str(), (file.clone(), s.anchor.as_str()));
        }
        for r in &g.references {
            reference_loc.insert(r.path.as_str(), (file.clone(), r.anchor.as_str()));
            if let Some(t) = &r.target_resolved {
                used_by.entry(t.as_str()).or_default().push(r.path.as_str());
            }
        }
    }
    for refs in used_by.values_mut() {
        refs.sort_unstable();
    }
    CrossRefs {
        symbol_loc,
        reference_loc,
        used_by,
    }
}

/// `` [`label`](file#anchor) `` when the path is locatable, else plain `` `label` ``.
fn xref_link(label: &str, loc: Option<&(String, &str)>) -> String {
    match loc {
        Some((file, anchor)) => format!("[`{label}`]({file}#{anchor})"),
        None => format!("`{label}`"),
    }
}

/// `## References` — every `BuiltIn.Reference` in the group and what it aliases.
/// The target deep-links to the symbol when it resolved to one we document, else
/// the raw `<Props Target>` string is shown verbatim (`—` when it has none) so
/// the page never invents or drops a target (#29).
fn render_references(out: &mut String, group: &GroupDoc, xrefs: &CrossRefs, links: &SourceLinker) {
    if group.references.is_empty() {
        return;
    }
    let _ = writeln!(out, "## References\n");
    let _ = writeln!(out, "| Reference | Target |");
    let _ = writeln!(out, "| --- | --- |");
    for r in &group.references {
        let target = match &r.target_resolved {
            Some(t) => xref_link(t, xrefs.symbol_loc.get(t.as_str())),
            None if r.target_raw.is_empty() => "—".to_string(),
            None => format!("`{}`", r.target_raw),
        };
        let _ = writeln!(
            out,
            "| <a id=\"{}\"></a>`{}`{} | {} |",
            r.anchor,
            r.path,
            links.suffix(r.def_line),
            target
        );
    }
    let _ = writeln!(out);
}

/// `## Used by` — the inverse of the references: for each symbol on this page
/// that a reference targets, the references that point at it, deep-linked (#29).
/// A reader on a channel sees who consumes it.
fn render_used_by(out: &mut String, group: &GroupDoc, xrefs: &CrossRefs) {
    let rows: Vec<(&SymbolDoc, &Vec<&str>)> = group
        .symbols
        .iter()
        .filter_map(|s| xrefs.used_by.get(s.path.as_str()).map(|refs| (s, refs)))
        .collect();
    if rows.is_empty() {
        return;
    }
    let _ = writeln!(out, "## Used by\n");
    let _ = writeln!(out, "| Symbol | Referenced by |");
    let _ = writeln!(out, "| --- | --- |");
    for (s, refs) in rows {
        let by = refs
            .iter()
            .map(|rp| xref_link(rp, xrefs.reference_loc.get(*rp)))
            .collect::<Vec<_>>()
            .join(", ");
        // The symbol is on THIS page, so a same-page fragment link suffices.
        let _ = writeln!(out, "| [`{}`](#{}) | {} |", s.path, s.anchor, by);
    }
    let _ = writeln!(out);
}

/// `## Relationships` — the interactive graph of what this group's members call
/// and read/write, seeded on the group's direct members and expanded one hop so
/// each member's immediate neighbours show (#37). Omitted entirely when the
/// group has no documented relationships, so a quiet page stays clean.
fn render_relationships(out: &mut String, group: &GroupDoc, graph: &ProjectGraph) {
    let members: Vec<&str> = group
        .symbols
        .iter()
        .map(|s| s.path.as_str())
        .chain(group.functions.iter().map(|f| f.path.as_str()))
        .chain(group.references.iter().map(|r| r.path.as_str()))
        .collect();
    let diagram = Diagram::for_group(graph, &members, &group.path, GROUP_GRAPH_DEPTH);
    if diagram.is_empty() {
        return;
    }
    let _ = writeln!(out, "## Relationships\n");
    emit_graph_block(out, "group", &group.path, GROUP_GRAPH_DEPTH, &diagram);
}
