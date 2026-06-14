//! Relationship-graph **diagrams** (#37): turn the project [`ProjectGraph`] into
//! a focused, *interactive* picture — a group's subsystem (its functions/symbols
//! and what they call / read / write) and, on demand, a deeper `--graph` view.
//!
//! Each diagram is rendered two ways, both with no external renderer or CDN:
//! - [`Diagram::to_mermaid`] — a ` ```mermaid ` body that GitHub and most
//!   Markdown viewers render natively (a static fallback for the canonical `.md`).
//! - [`Diagram::to_json`] — a compact node/edge payload the HTML site feeds to a
//!   small, **inline** force-directed canvas renderer (see `html::GRAPH_SCRIPT`):
//!   a physics layout coloured by owning group, sized by degree, with hover,
//!   drag, zoom and click-through to each node's page — the graphify experience,
//!   but self-contained (no library, no network fetch).
//!
//! Both come from the same computed sub-graph, so the two never disagree. The
//! node/edge selection and the colour assignment are deterministic (sorted
//! everywhere), matching the project's degrade-never-fake, reproducible-output
//! guarantees. Only edges whose endpoints are documented symbols appear — the
//! extractor already dropped dynamic/unresolved targets.

use crate::model::{EdgeKind, ProjectGraph};
use std::collections::{BTreeSet, HashMap, VecDeque};
use std::fmt::Write as _;

/// Tableau-10 + extension — a categorical palette (matches the graphify map) so
/// communities are visually distinct. Cycles if a diagram has more communities
/// than colours.
const PALETTE: &[&str] = &[
    "#4E79A7", "#F28E2B", "#E15759", "#76B7B2", "#59A14F", "#EDC948", "#B07AA1", "#FF9DA7",
    "#9C755F", "#BAB0AC",
];

/// One node in a rendered diagram. `primary` marks a node that is part of the
/// focus (a seed) rather than a boundary neighbour pulled in by depth, so the
/// renderer can de-emphasise context nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramNode {
    pub path: String,
    pub primary: bool,
}

/// One typed edge between two diagram nodes (a subset of the project graph).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagramEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

/// A computed sub-graph ready to render. Nodes are sorted by path and edges by
/// `(from, to, kind)`, so every render is byte-for-byte reproducible.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagram {
    /// Human title (e.g. `Subsystem Root.Engine`).
    pub title: String,
    pub nodes: Vec<DiagramNode>,
    pub edges: Vec<DiagramEdge>,
}

impl Diagram {
    /// Build the diagram around a set of `seeds`: every node within `depth` hops
    /// of a seed (following edges in either direction) and every edge among that
    /// set. Seed nodes are `primary`; nodes reached only by crossing the
    /// boundary are context. The shared core of [`Self::subsystem`] and
    /// [`Self::for_group`].
    pub fn around(graph: &ProjectGraph, seeds: &BTreeSet<&str>, depth: usize) -> Diagram {
        let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
        for e in &graph.edges {
            adj.entry(&e.from).or_default().push(&e.to);
            adj.entry(&e.to).or_default().push(&e.from);
        }
        let mut included: BTreeSet<&str> = BTreeSet::new();
        let mut q: VecDeque<(&str, usize)> = VecDeque::new();
        // Only seeds that actually appear in the graph can anchor a diagram.
        for &s in seeds {
            if (adj.contains_key(s)) && included.insert(s) {
                q.push_back((s, 0));
            }
        }
        let primary: BTreeSet<&str> = included.iter().copied().collect();
        while let Some((node, d)) = q.pop_front() {
            if d >= depth {
                continue;
            }
            if let Some(ns) = adj.get(node) {
                for &nb in ns {
                    if included.insert(nb) {
                        q.push_back((nb, d + 1));
                    }
                }
            }
        }
        let edges = collect_edges(graph, &included);
        let mut nodes: Vec<DiagramNode> = included
            .iter()
            .map(|p| DiagramNode {
                path: (*p).to_string(),
                primary: primary.contains(*p),
            })
            .collect();
        nodes.sort_by(|a, b| a.path.cmp(&b.path));
        Diagram {
            title: String::new(),
            nodes,
            edges,
        }
    }

    /// The **subsystem** under `group`: every symbol/function whose path is in
    /// the group, plus `depth` hops across the boundary, and the edges among
    /// them. Used by `--graph <group>` for a focused, possibly deep view.
    pub fn subsystem(graph: &ProjectGraph, group: &str, depth: usize) -> Diagram {
        let prefix = format!("{group}.");
        let seeds: BTreeSet<&str> = graph
            .edges
            .iter()
            .flat_map(|e| [e.from.as_str(), e.to.as_str()])
            .filter(|p| *p == group || p.starts_with(&prefix))
            .collect();
        let mut d = Self::around(graph, &seeds, depth);
        d.title = format!("Subsystem {group}");
        d
    }

    /// The relationships **for one group page**: seeded by the group's own
    /// direct members (`members`), expanded `depth` hops so each member's
    /// immediate neighbours show even when they live elsewhere. Keeps a page's
    /// graph about that page's contents (not the whole subtree).
    pub fn for_group(graph: &ProjectGraph, members: &[&str], group: &str, depth: usize) -> Diagram {
        let seeds: BTreeSet<&str> = members.iter().copied().collect();
        let mut d = Self::around(graph, &seeds, depth);
        d.title = format!("Relationships in {group}");
        d
    }

    /// True when there is nothing worth drawing (no edges). Callers skip the
    /// diagram entirely rather than emit an empty figure.
    pub fn is_empty(&self) -> bool {
        self.edges.is_empty()
    }

    /// Map each node to a community (its owning group = the path minus the leaf),
    /// then to a deterministic palette colour (communities sorted, assigned in
    /// order). Returns `(node path → colour, sorted communities with colour)`.
    fn communities(&self) -> (HashMap<&str, &'static str>, Vec<(String, &'static str)>) {
        let mut names: BTreeSet<String> = BTreeSet::new();
        for n in &self.nodes {
            names.insert(community(&n.path));
        }
        let colour: HashMap<String, &'static str> = names
            .iter()
            .enumerate()
            .map(|(i, c)| (c.clone(), PALETTE[i % PALETTE.len()]))
            .collect();
        let node_colour: HashMap<&str, &'static str> = self
            .nodes
            .iter()
            .map(|n| (n.path.as_str(), colour[&community(&n.path)]))
            .collect();
        let legend: Vec<(String, &'static str)> =
            names.into_iter().map(|c| (c.clone(), colour[&c])).collect();
        (node_colour, legend)
    }

    /// Render the diagram as a Mermaid `graph LR` body (no fence — the caller
    /// wraps it in a ` ```mermaid ` block). The Markdown fallback for viewers
    /// (GitHub) that render Mermaid natively. Node ids are positional so they
    /// are always Mermaid-safe; the real path tail is the quoted label.
    pub fn to_mermaid(&self) -> String {
        let id: HashMap<&str, String> = self
            .nodes
            .iter()
            .enumerate()
            .map(|(i, n)| (n.path.as_str(), format!("n{i}")))
            .collect();
        let mut out = String::from("graph LR\n");
        for n in &self.nodes {
            let _ = writeln!(
                out,
                "  {}[\"{}\"]",
                id[n.path.as_str()],
                mermaid_label(&short_label(&n.path))
            );
        }
        for e in &self.edges {
            let (a, b) = (&id[e.from.as_str()], &id[e.to.as_str()]);
            let link = match e.kind {
                EdgeKind::Call => format!("{a} --> {b}"),
                EdgeKind::Read => format!("{a} -. reads .-> {b}"),
                EdgeKind::Write => format!("{a} -- writes --> {b}"),
                EdgeKind::Reference => format!("{a} -. ref .-> {b}"),
            };
            let _ = writeln!(out, "  {link}");
        }
        out
    }

    /// Render the diagram as the compact JSON payload the inline force-directed
    /// renderer consumes (see `html::GRAPH_SCRIPT`). Each node carries its label,
    /// community colour, degree (edge count), whether it is primary, and its page
    /// link via `href` (already a `.html#anchor`); each edge carries its kind.
    /// A trailing `communities` array drives the legend. Deterministic.
    pub fn to_json(&self, href: impl Fn(&str) -> Option<String>) -> String {
        let (colour, legend) = self.communities();
        let mut degree: HashMap<&str, usize> =
            self.nodes.iter().map(|n| (n.path.as_str(), 0)).collect();
        for e in &self.edges {
            *degree.entry(e.from.as_str()).or_default() += 1;
            *degree.entry(e.to.as_str()).or_default() += 1;
        }
        let mut out = String::from("{\"nodes\":[");
        for (i, n) in self.nodes.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"id\":{},\"label\":{},\"community\":{},\"color\":\"{}\",\"degree\":{},\"primary\":{}",
                js(&n.path),
                js(&short_label(&n.path)),
                js(&community(&n.path)),
                colour[n.path.as_str()],
                degree[n.path.as_str()],
                n.primary,
            );
            if let Some(h) = href(&n.path) {
                let _ = write!(out, ",\"href\":{}", js(&h));
            }
            out.push('}');
        }
        out.push_str("],\"edges\":[");
        for (i, e) in self.edges.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let _ = write!(
                out,
                "{{\"from\":{},\"to\":{},\"kind\":\"{}\"}}",
                js(&e.from),
                js(&e.to),
                e.kind.as_str(),
            );
        }
        out.push_str("],\"communities\":[");
        for (i, (name, col)) in legend.iter().enumerate() {
            if i > 0 {
                out.push(',');
            }
            let _ = write!(out, "{{\"name\":{},\"color\":\"{}\"}}", js(name), col);
        }
        out.push_str("]}");
        out
    }
}

/// Every graph edge whose endpoints are both in `included`, deduped and sorted.
fn collect_edges(graph: &ProjectGraph, included: &BTreeSet<&str>) -> Vec<DiagramEdge> {
    let mut set: BTreeSet<(String, String, EdgeKind)> = BTreeSet::new();
    for e in &graph.edges {
        if included.contains(e.from.as_str()) && included.contains(e.to.as_str()) {
            set.insert((e.from.clone(), e.to.clone(), e.kind));
        }
    }
    set.into_iter()
        .map(|(from, to, kind)| DiagramEdge { from, to, kind })
        .collect()
}

/// A node's community: its owning group (the path with the leaf segment removed,
/// e.g. `Root.Engine.Speed` → `Root.Engine`). A single-segment path is its own
/// community. Communities colour the graph and drive the legend.
fn community(path: &str) -> String {
    match path.rfind('.') {
        Some(i) => path[..i].to_string(),
        None => path.to_string(),
    }
}

/// The visible label for a node: the last two dot-segments (`Root.Engine.Fuel.
/// Pump` → `Fuel.Pump`), compact yet near-unique; the full path is the tooltip
/// and the link target.
fn short_label(path: &str) -> String {
    let segs: Vec<&str> = path.split('.').collect();
    if segs.len() <= 2 {
        path.to_string()
    } else {
        segs[segs.len() - 2..].join(".")
    }
}

/// Escape a label for a Mermaid quoted node label (`"…"`).
fn mermaid_label(s: &str) -> String {
    s.replace('"', "&quot;")
}

/// Encode a string as a JSON string literal (quoted, with the control/quote
/// escapes needed inside an inline `<script type="application/json">`). `<` and
/// `/` are escaped so the payload can never close the embedding element early.
fn js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '<' => out.push_str("\\u003c"),
            '/' => out.push_str("\\/"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GraphEdge;

    fn edge(from: &str, to: &str, kind: EdgeKind) -> GraphEdge {
        GraphEdge {
            from: from.into(),
            to: to.into(),
            kind,
        }
    }

    /// `Sensor` calls `Update`; `Update` calls `Output`, reads `Speed`, writes
    /// `Torque`. A 2-hop node (`Caller`) is excluded at depth 1.
    fn sample() -> ProjectGraph {
        ProjectGraph {
            edges: vec![
                edge("Root.Caller", "Root.Sensor", EdgeKind::Call),
                edge("Root.Sensor", "Root.Update", EdgeKind::Call),
                edge("Root.Update", "Root.Output", EdgeKind::Call),
                edge("Root.Update", "Root.Speed", EdgeKind::Read),
                edge("Root.Update", "Root.Torque", EdgeKind::Write),
            ],
        }
    }

    /// Seeded on a single member, `for_group` is a node's neighbourhood: its
    /// direct callers/callees/reads/writes, with the seed marked primary.
    #[test]
    fn neighbourhood_is_direct_callers_callees_reads_writes() {
        let d = Diagram::for_group(&sample(), &["Root.Update"], "Root", 1);
        let paths: BTreeSet<&str> = d.nodes.iter().map(|n| n.path.as_str()).collect();
        assert!(paths.contains("Root.Update"));
        assert!(paths.contains("Root.Sensor"), "direct caller");
        assert!(paths.contains("Root.Output"), "direct callee");
        assert!(paths.contains("Root.Speed"), "direct read");
        assert!(paths.contains("Root.Torque"), "direct write");
        assert!(!paths.contains("Root.Caller"), "2-hop excluded at depth 1");
        // Only the seed is primary.
        assert!(
            d.nodes
                .iter()
                .find(|n| n.path == "Root.Update")
                .unwrap()
                .primary
        );
        assert!(
            !d.nodes
                .iter()
                .find(|n| n.path == "Root.Output")
                .unwrap()
                .primary
        );
    }

    #[test]
    fn isolated_seed_yields_empty_diagram() {
        let g = ProjectGraph {
            edges: vec![edge("A", "B", EdgeKind::Call)],
        };
        assert!(Diagram::for_group(&g, &["Lonely"], "Root", 1).is_empty());
    }

    #[test]
    fn depth_two_pulls_in_the_second_hop() {
        let d = Diagram::for_group(&sample(), &["Root.Update"], "Root", 2);
        let paths: BTreeSet<&str> = d.nodes.iter().map(|n| n.path.as_str()).collect();
        assert!(paths.contains("Root.Caller"), "2-hop included at depth 2");
    }

    #[test]
    fn subsystem_scopes_to_the_group_prefix() {
        let g = ProjectGraph {
            edges: vec![
                edge("Root.Eng.A", "Root.Eng.B", EdgeKind::Call),
                edge("Root.Eng.B", "Root.Ext.Z", EdgeKind::Write),
            ],
        };
        let d0 = Diagram::subsystem(&g, "Root.Eng", 0);
        assert!(!d0.nodes.iter().any(|n| n.path == "Root.Ext.Z"));
        let d1 = Diagram::subsystem(&g, "Root.Eng", 1);
        assert!(
            d1.nodes.iter().any(|n| n.path == "Root.Ext.Z"),
            "depth 1 crosses boundary"
        );
    }

    #[test]
    fn for_group_seeds_on_direct_members() {
        let d = Diagram::for_group(&sample(), &["Root.Update"], "Root", 1);
        assert!(
            d.nodes
                .iter()
                .find(|n| n.path == "Root.Update")
                .unwrap()
                .primary
        );
        // The pulled-in neighbours are context, not primary.
        assert!(
            !d.nodes
                .iter()
                .find(|n| n.path == "Root.Speed")
                .unwrap()
                .primary
        );
    }

    #[test]
    fn mermaid_has_header_labels_and_typed_links() {
        let d = Diagram::for_group(&sample(), &["Root.Update"], "Root", 1);
        let m = d.to_mermaid();
        assert!(m.starts_with("graph LR\n"));
        assert!(m.contains("[\"Root.Update\"]"));
        assert!(m.contains(" --> "), "call edge");
        assert!(m.contains("-. reads .->"), "read edge");
        assert!(m.contains("-- writes -->"), "write edge");
    }

    #[test]
    fn json_payload_is_self_contained_with_links_colours_and_legend() {
        let d = Diagram::for_group(&sample(), &["Root.Update"], "Root", 1);
        let json = d.to_json(|p| Some(format!("{}.html#x", community(p).to_lowercase())));
        // Parseable shape with the documented fields.
        assert!(json.starts_with("{\"nodes\":["));
        assert!(json.contains("\"kind\":\"call\""));
        assert!(json.contains("\"kind\":\"read\""));
        assert!(json.contains("\"kind\":\"write\""));
        assert!(json.contains("\"communities\":["));
        assert!(json.contains("\"href\":"), "nodes link to their pages");
        assert!(json.contains("#x"));
        // No script/CDN smuggled into the data; `<` and `/` are escaped.
        assert!(!json.contains("</script"));
        assert!(!json.contains("http://"));
    }

    #[test]
    fn render_is_deterministic() {
        let g = sample();
        let a = Diagram::for_group(&g, &["Root.Update"], "Root", 2);
        let b = Diagram::for_group(&g, &["Root.Update"], "Root", 2);
        assert_eq!(a.to_mermaid(), b.to_mermaid());
        assert_eq!(a.to_json(|_| None), b.to_json(|_| None));
    }

    #[test]
    fn js_escapes_script_close_and_controls() {
        assert_eq!(js("a/b"), "\"a\\/b\"");
        assert_eq!(js("<x>"), "\"\\u003cx>\"");
        assert!(js("a\nb").contains("\\n"));
    }
}
