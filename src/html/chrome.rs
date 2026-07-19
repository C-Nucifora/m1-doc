//! Page chrome built as hand-written raw HTML: the sidebar navigation tree, the
//! sticky toolbar (menu / search box / theme toggle / TOC slot), and the
//! security/tag filter panel. All user-facing strings pass through
//! [`super::escaping`] so M1 names with markup-significant characters stay safe.

use super::escaping::{attr_escape, html_escape};
use crate::model::DocModel;

pub(super) fn build_nav(model: &DocModel) -> String {
    use std::collections::BTreeMap;
    let by_path: BTreeMap<&str, &crate::model::GroupDoc> =
        model.groups.iter().map(|g| (g.path.as_str(), g)).collect();

    let mut nav = String::from("<nav><h2>Navigation</h2>");
    nav.push_str("<a href=\"index.html\">Index</a>");
    if !model.enums.is_empty() {
        nav.push_str("<a href=\"enums.html\">Enums</a>");
    }
    nav.push_str("<ul>");
    // Start the tree from the forest roots (groups whose parent is not itself a
    // documented group); descend recursively. The interactive collapse widget
    // is the HTML-polish issue (#33); this is the nested structure it needs.
    for g in &model.groups {
        let parent = match g.path.rfind('.') {
            Some(i) => &g.path[..i],
            None => "",
        };
        if parent.is_empty() || !by_path.contains_key(parent) {
            push_nav_node(&mut nav, g, &by_path);
        }
    }
    nav.push_str("</ul></nav>");
    nav
}

/// Append one `<li>` for a group node and, recursively, a nested `<ul>` for its
/// children — reflecting the real group hierarchy in the sidebar.
fn push_nav_node(
    nav: &mut String,
    g: &crate::model::GroupDoc,
    by_path: &std::collections::BTreeMap<&str, &crate::model::GroupDoc>,
) {
    let label = g.path.rsplit('.').next().unwrap_or(&g.path);
    // M1 names may contain spaces and markup-significant characters, so escape
    // both the href (attribute context) and the visible label (text context).
    // `attr_escape` deliberately leaves spaces verbatim so the href matches the
    // on-disk page filename (`<group path>.html` keeps spaces literal too).
    nav.push_str(&format!(
        "<li><a href=\"{}.html\">{}</a>",
        attr_escape(&g.path),
        html_escape(label)
    ));
    if !g.children.is_empty() {
        nav.push_str("<ul>");
        for child in &g.children {
            if let Some(cg) = by_path.get(child.as_str()) {
                push_nav_node(nav, cg, by_path);
            }
        }
        nav.push_str("</ul>");
    }
    nav.push_str("</li>");
}

/// The sticky toolbar: a menu button (narrow screens), the live search box and
/// its results list, a theme toggle, and an empty slot the script fills with a
/// per-page table of contents. The search box is wired to the inline index by
/// the script; with JS off it is an inert text box (the index is also a plain
/// list reachable by browsing), so the site degrades rather than breaks.
pub(super) fn build_toolbar() -> String {
    let mut t = String::from("<div class=\"toolbar\">");
    t.push_str("<button id=\"menu-toggle\" class=\"btn\" title=\"Toggle navigation\">☰</button>");
    t.push_str(
        "<input id=\"search-box\" type=\"search\" placeholder=\"Search symbols, functions, tables…\" autocomplete=\"off\">",
    );
    t.push_str("<button id=\"theme-toggle\" class=\"btn\" title=\"Toggle dark mode\">◐</button>");
    t.push_str("</div>");
    t.push_str("<ul id=\"search-results\"></ul>");
    t.push_str("<div id=\"toc-slot\"></div>");
    t
}

/// The security/tag filter panel for a group page: a checkbox per security
/// level and per tag present in the project, each ticking the matching rows
/// visible (#34). Returns an empty string when the project declares neither, so
/// untagged/security-free projects get no empty panel. A short legend explains
/// what the controls do, complementing the per-level legend on the index.
pub(super) fn build_filters(model: &DocModel) -> String {
    let levels = model.security_levels();
    let tags = model.tags();
    if levels.is_empty() && tags.is_empty() {
        return String::new();
    }
    let mut f =
        String::from("<details id=\"filters\" class=\"filters\"><summary>Filter rows</summary>");
    if !levels.is_empty() {
        f.push_str("<div><strong>Security</strong> ");
        for level in &levels {
            let esc = attr_escape(level);
            f.push_str(&format!(
                "<label><input type=\"checkbox\" data-sec=\"{esc}\"> {esc}</label>"
            ));
        }
        f.push_str("</div>");
    }
    if !tags.is_empty() {
        f.push_str("<div><strong>Tags</strong> ");
        for tag in &tags {
            let esc = attr_escape(tag);
            f.push_str(&format!(
                "<label><input type=\"checkbox\" data-tag=\"{esc}\"> {esc}</label>"
            ));
        }
        f.push_str("</div>");
    }
    f.push_str(
        "<div><small>Tick levels/tags to show only matching rows; all unticked shows everything.</small></div>",
    );
    f.push_str("</details>");
    f
}
