//! Leaf formatting and path helpers shared across the Markdown page renderers:
//! filename/anchor conventions, rate/number formatting, URL encoding, the
//! jump-to-declaration [`SourceLinker`], and the relationship-graph sentinel
//! block. These carry no page structure of their own — the per-page renderers
//! ([`super::group`], [`super::pages`]) compose them.

use crate::diagram::Diagram;
use crate::model::AnnotationDoc;
use std::fmt::Write as _;

/// The filename of the project-wide Enums reference page.
pub(super) const ENUMS_FILE: &str = "enums.md";

/// `Root.Engine` -> `Root.Engine.md` (a flat, link-safe filename keyed by the
/// full group path, so every node in the tree has a distinct page).
pub(super) fn group_filename(group_path: &str) -> String {
    format!("{group_path}.md")
}

/// The leaf segment of a dotted path (`Root.Engine.Fuel` -> `Fuel`) — the label
/// to show for a group in breadcrumbs and sub-group lists.
pub(super) fn last_segment(path: &str) -> &str {
    path.rsplit('.').next().unwrap_or(path)
}

/// Render a `Root › Engine › Fuel` breadcrumb: every ancestor segment is a link
/// to its own page; the current (last) segment is plain text.
pub(super) fn render_breadcrumb(path: &str) -> String {
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
pub(super) fn format_annotation(ann: &AnnotationDoc) -> String {
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

/// Percent-encode a project-relative path for use as a URL target, preserving
/// the `/` segment separators. M1 object names may contain spaces (Development
/// Manual, *Naming Objects*: "Space may be used between two name constituents"),
/// so the on-disk path — and thus a `{base}/{path}` link — can contain spaces
/// and other URL-unsafe characters that browsers/Markdown renderers truncate or
/// mangle. Only the URL target is encoded; the visible link *text* keeps the
/// raw, human-readable path.
///
/// Each `/`-delimited segment is encoded independently: every byte outside the
/// unreserved set (`A–Z a–z 0–9 - . _ ~`) becomes `%XX`. The `/` separators are
/// emitted verbatim so the path structure (and GitHub line anchors) survive.
pub(super) fn url_encode_path(path: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(path.len());
    for (i, segment) in path.split('/').enumerate() {
        if i > 0 {
            out.push('/');
        }
        for &byte in segment.as_bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    out.push(byte as char);
                }
                _ => {
                    out.push('%');
                    out.push(HEX[(byte >> 4) as usize] as char);
                    out.push(HEX[(byte & 0xf) as usize] as char);
                }
            }
        }
    }
    out
}

/// Build a function's Source line. With a `source_base` the path becomes an
/// external link (`{base}/{path}`); without one it is shown verbatim so the
/// reader still sees which `.m1scr` implements the function (#30).
pub(super) fn source_line(path: &str, base: Option<&str>) -> String {
    match base {
        Some(b) => {
            let b = b.trim_end_matches('/');
            format!("**Source:** [{path}]({b}/{})\n", url_encode_path(path))
        }
        None => format!("**Source:** `{path}`\n"),
    }
}

/// Builds jump-to-declaration links for project-sourced entities (channels,
/// parameters, constants, tables, objects, references) (#57). Every such entity
/// is declared in the same `Project.m1prj`, so the path lives once on the model;
/// only the per-entity `def_line` varies. A link is built only when a
/// `--source-base` *and* the project path are both known.
pub(super) struct SourceLinker<'a> {
    /// `--source-base` (a blob-URL prefix), trailing slash trimmed. `None`
    /// degrades every link to plain text — the row is unchanged.
    pub(super) base: Option<String>,
    /// The project-relative `.m1prj` path the entity `def_line`s index into.
    pub(super) m1prj_path: Option<&'a str>,
}

impl<'a> SourceLinker<'a> {
    /// A Markdown `[src](url)` link to an entity's declaration when both a
    /// source base and the project path are known and the entity carries a
    /// `def_line` — otherwise `None` (degrade to no link, never invent one).
    /// `def_line` is 0-based; GitHub line anchors are 1-based, so it is bumped
    /// by one (`def_line 41` → `#L42`).
    fn link(&self, def_line: Option<u32>) -> Option<String> {
        let base = self.base.as_deref()?;
        let path = self.m1prj_path?;
        let line = def_line?;
        Some(format!(
            "[src]({base}/{}#L{})",
            url_encode_path(path),
            line + 1
        ))
    }

    /// The `link` rendered as a leading ` ` + the link, ready to append to a
    /// heading or cell; empty when no link could be built. Keeps the common
    /// (no-link) case byte-identical to before.
    pub(super) fn suffix(&self, def_line: Option<u32>) -> String {
        match self.link(def_line) {
            Some(l) => format!(" {l}"),
            None => String::new(),
        }
    }
}

/// Format an `f64` for a CAN cell: trim trailing zeros so `0.50` → `0.5` and
/// `2.0` → `2`.
pub(super) fn fmt_f64(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s.is_empty() || s == "-0" {
        "0".to_string()
    } else {
        s.to_string()
    }
}

/// The tag-index page filename for a tag, slugged so it is link-safe
/// (`fuel` → `tag.fuel.md`, `Powertrain Fuel` → `tag.powertrain-fuel.md`).
pub(super) fn tag_filename(tag: &str) -> String {
    format!("tag.{}.md", crate::model::anchor_slug(tag))
}

/// The filename of a `--graph <group>` subsystem page (`graph.root-engine.md`).
pub(super) fn graph_page_filename(group: &str) -> String {
    format!("graph.{}.md", crate::model::anchor_slug(group))
}

/// Emit a relationship-graph block: a sentinel comment the HTML renderer swaps
/// for the interactive widget, followed by a ` ```mermaid ` fallback so the
/// canonical Markdown still shows a diagram where Mermaid renders (GitHub). The
/// blank line between the comment and the fence keeps pulldown-cmark treating
/// the comment as a raw-HTML block and the fence as a real code block (#37).
pub(super) fn emit_graph_block(
    out: &mut String,
    mode: &str,
    group: &str,
    depth: usize,
    diagram: &Diagram,
) {
    let _ = writeln!(out, "<!--m1-graph:{mode}:{depth}:{group}-->\n");
    let _ = writeln!(out, "```mermaid");
    out.push_str(&diagram.to_mermaid());
    let _ = writeln!(out, "```\n");
}
