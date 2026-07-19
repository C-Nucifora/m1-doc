//! Whole-page renderers other than the group page: the `index.md` landing page
//! (stats line, structure tree, security legend, tag facet), the per-tag index
//! pages, the project-wide Enums reference, and the focused `--graph <group>`
//! subsystem page.

use super::helpers::{
    ENUMS_FILE, emit_graph_block, graph_page_filename, group_filename, tag_filename,
};
use super::{GraphSpec, RenderOptions};
use crate::diagram::Diagram;
use crate::model::{DocModel, EnumDoc};
use std::fmt::Write as _;

/// Pluralise a count for the stats line: `1 channel`, `2 channels`, `0 tables`.
fn count_phrase(n: usize, singular: &str) -> String {
    if n == 1 {
        format!("{n} {singular}")
    } else {
        format!("{n} {singular}s")
    }
}

/// The one-line project summary: total components and the per-kind breakdown,
/// computed from the model (#32). Only non-zero kinds appear so the line stays
/// readable on small projects, but `components` and `groups` are always shown.
fn stats_line(model: &DocModel) -> String {
    let s = model.stats();
    let mut parts = vec![count_phrase(s.total_components(), "component")];
    for (n, word) in [
        (s.channels, "channel"),
        (s.parameters, "parameter"),
        (s.constants, "constant"),
        (s.functions, "function"),
        (s.tables, "table"),
        (s.objects, "object"),
        (s.can_messages, "CAN message"),
        (s.enums, "enum"),
    ] {
        if n > 0 {
            parts.push(count_phrase(n, word));
        }
    }
    parts.push(count_phrase(s.top_level_groups, "top-level group"));
    parts.join(" · ")
}

pub(super) fn render_index(model: &DocModel, opts: &RenderOptions) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", model.title);

    // Target hardware: degrade-never-fake. The project API does not expose it
    // yet (#32), so we say so explicitly rather than invent a value.
    match &model.target_hardware {
        Some(hw) => {
            let _ = writeln!(out, "**Target hardware:** {hw}\n");
        }
        None => {
            let _ = writeln!(
                out,
                "**Target hardware:** — *(not exposed by the project API yet)*\n"
            );
        }
    }

    // Summary stats line.
    let _ = writeln!(out, "{}\n", stats_line(model));

    // The group tree: forest roots with per-group direct counts. The full tree
    // is reachable by descending from each root's page.
    let _ = writeln!(out, "## Structure\n");
    for node in model.top_level_tree() {
        // The count is the whole subtree so a top-level group reads its size at
        // a glance; `▸` marks an expandable node (the HTML nav makes it live).
        let suffix = if node.has_children { " ▸" } else { "" };
        let _ = writeln!(
            out,
            "- [{}]({}) ({}){suffix}",
            node.path,
            group_filename(&node.path),
            node.subtree_count,
        );
    }
    out.push('\n');

    // Security legend: every level the project declares, with a one-line gloss
    // (#34). Skipped entirely when the project declares no security at all.
    let levels = model.security_levels();
    if !levels.is_empty() {
        let _ = writeln!(out, "## Security levels\n");
        let _ = writeln!(
            out,
            "Access level required to view or calibrate a value. Levels present in this project:\n"
        );
        for level in &levels {
            let _ = writeln!(out, "- **{level}** — {}", security_gloss(level));
        }
        out.push('\n');
    }

    // Tag facet: link each per-tag index page (#34). Skipped when untagged.
    let tags = model.tags();
    if !tags.is_empty() {
        let _ = writeln!(out, "## Tags\n");
        for tag in &tags {
            let _ = writeln!(out, "- [{tag}]({})", tag_filename(tag));
        }
        out.push('\n');
    }

    // Focused subsystem graph, when one was requested with `--graph` (#37).
    if let Some(spec) = &opts.graph {
        let _ = writeln!(out, "## Subsystem graph\n");
        let _ = writeln!(
            out,
            "- [Subsystem: {}]({})",
            spec.group,
            graph_page_filename(&spec.group)
        );
        out.push('\n');
    }

    if !model.enums.is_empty() {
        let _ = writeln!(out, "## Reference\n");
        let _ = writeln!(out, "- [Enums]({ENUMS_FILE})");
    }
    out
}

/// A short, fixed gloss for the security/access levels MoTeC M1 projects use.
/// Unknown levels degrade to a generic note rather than being dropped (#34).
fn security_gloss(level: &str) -> &'static str {
    match level {
        "Tune" => "tunable at the Tune access level",
        "Calibration" => "calibration data, editable with a Calibration licence",
        "Master Calibration" => "master-calibration data (highest calibration tier)",
        "Resource" => "resource/IO assignment level",
        "Engineering" => "engineering-only, not exposed to calibrators",
        "Read Only" => "read-only; not editable in MoTeC M1 Tune",
        _ => "project-defined access level",
    }
}

/// Render a per-tag index page: every symbol carrying `tag`, deep-linked to its
/// row on the owning group page (#34). Deterministic — symbols are walked in
/// model order (groups sorted, members sorted), so the listing is stable.
pub(super) fn render_tag_index(model: &DocModel, tag: &str) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "[Index](index.md)\n");
    let _ = writeln!(out, "# Tag: {tag}\n");
    let mut any = false;
    for g in &model.groups {
        for s in &g.symbols {
            if s.tags.iter().any(|t| t == tag) {
                any = true;
                let _ = writeln!(
                    out,
                    "- [{}]({}#{})",
                    s.path,
                    group_filename(&g.path),
                    s.anchor,
                );
            }
        }
    }
    if !any {
        let _ = writeln!(out, "(no symbols carry this tag)");
    }
    out
}

/// Render the project-wide Enums reference page: each enum is an anchored
/// section listing its enumerators (container order), default, and open flag.
/// An `open` (firmware) enum is labelled so its member list reads as partial.
pub(super) fn render_enums(enums: &[EnumDoc]) -> String {
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
            // `value = name` — the manual defines an enum as a value→name map,
            // so the numeric value (stored on the wire / in logs) is shown for
            // every enumerator. The default member is marked.
            for m in &e.members {
                let default = if e.default.as_deref() == Some(m.name.as_str()) {
                    " (default)"
                } else {
                    ""
                };
                let _ = writeln!(out, "- {} = {}{default}", m.value, m.name);
            }
            out.push('\n');
        }
    }
    out
}

/// Render the focused `--graph <group>` subsystem page: the whole subtree under
/// the group plus `depth` hops across its boundary, as one interactive graph
/// (#37). Always produced when requested; an edge-less group yields a note.
pub(super) fn render_graph_page(model: &DocModel, spec: &GraphSpec) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "[Index](index.md)\n");
    let _ = writeln!(out, "# Subsystem: {}\n", spec.group);
    let diagram = Diagram::subsystem(&model.graph, &spec.group, spec.depth);
    if diagram.is_empty() {
        let _ = writeln!(out, "No documented relationships under `{}`.", spec.group);
        return out;
    }
    let _ = writeln!(
        out,
        "Calls and data flow under `{}` (depth {}).\n",
        spec.group, spec.depth
    );
    emit_graph_block(&mut out, "subtree", &spec.group, spec.depth, &diagram);
    out
}
