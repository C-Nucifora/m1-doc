//! Relationship-graph widgets (#37). The Markdown renderer drops a sentinel
//! comment + a ` ```mermaid ` fallback for each graph; here we swap that pair for
//! the interactive force-directed widget — a `<canvas>` plus an inline JSON
//! payload the page's `buildGraph` renders with no library and no network. The
//! diagram is regenerated from the model using the sentinel's `mode:depth:group`,
//! so the two outputs always agree.

use super::escaping::html_escape;
use crate::diagram::Diagram;
use crate::model::{AnchoredKind, DocModel};
use std::collections::HashMap;

/// Map every graph-eligible documented entity's path to its page link
/// (`<group>.html#<anchor>`), so a graph node can deep-link to where it is
/// documented. Built once from the model's shared [`DocModel::anchored_entities`]
/// walk (so it can never drift from the search index again) and filtered to the
/// kinds that can appear as graph nodes — i.e. everything anchored on a group
/// page (symbols, functions, tables, objects, CAN messages and signals,
/// references); enums live on the shared reference page and are not graph nodes.
pub(super) fn node_hrefs(model: &DocModel) -> HashMap<String, String> {
    model
        .anchored_entities()
        .into_iter()
        .filter(|e| e.kind != AnchoredKind::Enum)
        .map(|e| (e.path.to_string(), e.href()))
        .collect()
}

/// Rebuild the diagram a sentinel refers to. `mode` is `group` (seed on the
/// group's direct members) or `subtree` (the whole `--graph` subsystem).
fn diagram_for(model: &DocModel, mode: &str, group: &str, depth: usize) -> Diagram {
    match mode {
        "subtree" => Diagram::subsystem(&model.graph, group, depth),
        _ => {
            let members: Vec<&str> = model
                .groups
                .iter()
                .find(|g| g.path == group)
                .map(|g| {
                    g.symbols
                        .iter()
                        .map(|s| s.path.as_str())
                        .chain(g.functions.iter().map(|f| f.path.as_str()))
                        .chain(g.references.iter().map(|r| r.path.as_str()))
                        .collect()
                })
                .unwrap_or_default();
            Diagram::for_group(&model.graph, &members, group, depth)
        }
    }
}

/// The `<figure>` markup for one diagram: the canvas stage, the legend slot, and
/// the inline JSON the renderer consumes. An edge-less diagram degrades to a
/// note rather than an empty canvas.
fn graph_figure(diagram: &Diagram, hrefs: &HashMap<String, String>) -> String {
    if diagram.is_empty() {
        return format!(
            "<figure class=\"m1-graph\"><div class=\"m1-graph-empty\">No documented \
relationships{}.</div></figure>",
            if diagram.title.is_empty() {
                String::new()
            } else {
                format!(" for {}", html_escape(&diagram.title))
            }
        );
    }
    let json = diagram.to_json(|p| hrefs.get(p).cloned());
    format!(
        "<figure class=\"m1-graph\">\
<div class=\"m1-graph-head\">\
<span class=\"m1-graph-title\">{title}</span>\
<span class=\"m1-graph-hint\">drag · scroll to zoom · click a node to open its page</span>\
<button class=\"m1-graph-reset\" type=\"button\">Fit</button>\
</div>\
<div class=\"m1-graph-stage\"><canvas></canvas><div class=\"m1-graph-tip\" hidden></div></div>\
<div class=\"m1-graph-legend\"></div>\
<script type=\"application/json\" class=\"m1-graph-data\">{json}</script>\
</figure>",
        title = html_escape(&diagram.title),
    )
}

/// Replace every `<!--m1-graph:mode:depth:group-->` sentinel (and the Mermaid
/// `<pre>` block pulldown-cmark rendered right after it) with the interactive
/// graph figure. Other content is untouched.
pub(super) fn swap_graphs(html: &str, model: &DocModel, hrefs: &HashMap<String, String>) -> String {
    const OPEN: &str = "<!--m1-graph:";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(OPEN) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + OPEN.len()..];
        let Some(close) = after.find("-->") else {
            // Malformed — emit verbatim and stop scanning.
            out.push_str(&rest[pos..]);
            return out;
        };
        let spec = &after[..close];
        // mode:depth:group  (group may contain ':'? paths use '.', so splitn is safe)
        let mut it = spec.splitn(3, ':');
        let mode = it.next().unwrap_or("group");
        let depth: usize = it.next().and_then(|d| d.parse().ok()).unwrap_or(1);
        let group = it.next().unwrap_or("");
        // Advance past the comment, then past the trailing Mermaid <pre> block.
        let mut tail = &after[close + 3..];
        if let Some(ps) = tail.find("<pre>")
            && let Some(pe) = tail[ps..].find("</pre>")
        {
            tail = &tail[ps + pe + "</pre>".len()..];
        }
        let diagram = diagram_for(model, mode, group, depth);
        out.push_str(&graph_figure(&diagram, hrefs));
        rest = tail;
    }
    out.push_str(rest);
    out
}
