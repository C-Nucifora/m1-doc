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

/// Decode the renderer-owned `mode:depth:hex-group` marker strictly.
fn graph_spec(spec: &str) -> Option<(&str, usize, String)> {
    let mut fields = spec.split(':');
    let mode = fields.next()?;
    if !matches!(mode, "group" | "subtree") {
        return None;
    }
    let depth = fields.next()?;
    if depth.is_empty() || !depth.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let depth = depth.parse().ok()?;
    let payload = fields.next()?;
    if fields.next().is_some() || payload.len() % 2 != 0 {
        return None;
    }
    let bytes: Option<Vec<u8>> = payload
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| {
            let high = (pair[0] as char).to_digit(16)?;
            let low = (pair[1] as char).to_digit(16)?;
            Some((high * 16 + low) as u8)
        })
        .collect();
    Some((mode, depth, String::from_utf8(bytes?).ok()?))
}

/// Replace a valid sentinel and its immediately following Mermaid block.
pub(super) fn swap_graphs(html: &str, model: &DocModel, hrefs: &HashMap<String, String>) -> String {
    const OPEN: &str = "<!--m1-graph:";
    const BLOCK: &str = "<pre><code class=\"language-mermaid\">";
    const END: &str = "</code></pre>";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(OPEN) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + OPEN.len()..];
        let Some(close) = after.find("-->") else {
            out.push_str(&rest[pos..]);
            return out;
        };
        let tail = &after[close + 3..];
        let block = tail.trim_start();
        if let Some((mode, depth, group)) = graph_spec(&after[..close])
            && let Some(body) = block.strip_prefix(BLOCK)
            && let Some(end) = body.find(END)
            && !body[..end].contains('<')
        {
            let diagram = diagram_for(model, mode, &group, depth);
            out.push_str(&graph_figure(&diagram, hrefs));
            rest = &body[end + END.len()..];
        } else {
            // A malformed pair cannot consume unrelated prose or source code.
            out.push_str(&rest[pos..pos + OPEN.len() + close + 3]);
            rest = tail;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_graph_pairs_preserve_unrelated_content() {
        let valid = "<!--m1-graph:group:1:526f6f74-->";
        let mermaid = "<pre><code class=\"language-mermaid\">graph LR\n</code></pre>";
        let incomplete = format!(
            "{valid}\n<pre><code class=\"language-mermaid\">incomplete\n<pre><code>Keep this</code></pre>"
        );
        assert_eq!(
            swap_graphs(&incomplete, &DocModel::default(), &HashMap::new()),
            incomplete
        );
        for marker in [
            valid,
            "<!--m1-graph:bad:1:526f6f74-->",
            "<!--m1-graph:group:x:526f6f74-->",
            "<!--m1-graph:group:1:zz-->",
            "<!--m1-graph:group:1:ff-->",
            "<!--m1-graph:group:1:5-->",
            "<!--m1-graph:group:1:52:00-->",
        ] {
            let html = format!("{marker}\n<p>Keep this</p>\n{mermaid}");
            assert_eq!(
                swap_graphs(&html, &DocModel::default(), &HashMap::new()),
                html
            );
            if marker != valid {
                let html = format!("{marker}\n{mermaid}");
                assert_eq!(
                    swap_graphs(&html, &DocModel::default(), &HashMap::new()),
                    html
                );
            }
        }
        for block in [
            "<pre><code>source</code></pre>",
            "<pre><code class=\"language-mermaid\">unclosed",
        ] {
            let html = format!("{valid}\n{block}");
            assert_eq!(
                swap_graphs(&html, &DocModel::default(), &HashMap::new()),
                html
            );
        }
        let html = format!("{valid}\n{mermaid}<p>After</p>");
        let swapped = swap_graphs(&html, &DocModel::default(), &HashMap::new());
        assert_eq!(swapped.matches("<figure").count(), 1);
        assert!(swapped.ends_with("<p>After</p>"));
    }
}
