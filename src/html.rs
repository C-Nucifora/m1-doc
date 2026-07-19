//! Renders Markdown files (produced by [`crate::markdown`]) to a self-contained
//! HTML site.  Each `.md` file becomes a `.html` file; intra-doc links are
//! rewritten from `*.md` to `*.html`.  External `http(s)://` links are left
//! untouched.  The only inputs are [`crate::markdown::RenderedFile`] slices and
//! a [`crate::model::DocModel`] (for the sidebar and page title).  No m1-core /
//! m1-typecheck types cross this module boundary.
//!
//! The renderer is split into section modules behind small interfaces:
//! [`assets`] (the inline CSS/JS), [`escaping`] (HTML escaping), [`links`]
//! (`.md`→`.html` rewriting), [`search`] (the shared search index), [`graph`]
//! (the interactive relationship-graph widgets), and [`chrome`] (nav, toolbar,
//! filters). This module composes them into each page.

mod assets;
mod chrome;
mod escaping;
mod graph;
mod links;
mod search;

use crate::markdown::RenderedFile;
use crate::model::DocModel;

use assets::{SCRIPT, STYLE};
use chrome::{build_filters, build_nav, build_toolbar};
use escaping::html_escape;
use graph::{node_hrefs, swap_graphs};
use links::rewrite_md_links;
use search::{SEARCH_INDEX_FILE, build_search_index_file, search_index_ref};

/// Convert a slice of Markdown [`RenderedFile`]s to HTML [`RenderedFile`]s.
///
/// For each input file:
/// - renders the Markdown body to an HTML fragment (tables enabled),
/// - wraps it in a minimal self-contained page with inline CSS and a sidebar,
/// - rewrites relative `*.md` hrefs to `*.html`,
/// - changes the output path from `*.md` to `*.html`.
pub fn render(markdown_files: &[RenderedFile], model: &DocModel) -> Vec<RenderedFile> {
    let nav = build_nav(model);
    let toolbar = build_toolbar();
    let filters = build_filters(model);
    // The search index is one shared sibling file loaded via `<script src>`, so
    // every page carries only a tiny reference to it, not the whole index (#31).
    let search_index = search_index_ref();
    // Node → page-link map for the interactive relationship graphs (#37).
    let graph_hrefs = node_hrefs(model);
    let mut out: Vec<RenderedFile> = markdown_files
        .iter()
        .map(|f| {
            // 1. Convert Markdown → HTML fragment (tables enabled).
            let mut fragment = String::new();
            let parser =
                pulldown_cmark::Parser::new_ext(&f.body, pulldown_cmark::Options::ENABLE_TABLES);
            pulldown_cmark::html::push_html(&mut fragment, parser);

            // 2. Swap relationship-graph sentinels (+ their Mermaid fallback)
            //    for the interactive force-directed widget (#37).
            let fragment = swap_graphs(&fragment, model, &graph_hrefs);

            // 3. Rewrite intra-doc .md links → .html links.
            let fragment = rewrite_md_links(&fragment);

            // The row filter only belongs on a group page (one with filterable
            // rows). The landing/enums/tag-index pages have no `.m1-row-anchor`
            // rows, so the panel would filter nothing — omit it there.
            let is_group_page =
                f.path != "index.md" && f.path != "enums.md" && !f.path.starts_with("tag.");
            let filter_panel = if is_group_page { filters.as_str() } else { "" };

            // 3. Wrap in full page. The toolbar (search + theme + menu + TOC
            // slot) sits at the top of <main>; the shared search-index sidecar
            // and the behaviour script are appended before </body>. The only
            // asset is a same-directory `<script src>`, which loads from
            // `file://` — so the site stays self-contained (#31/#33).
            let page = format!(
                "<!doctype html>\
<html lang=\"en\">\
<head>\
<meta charset=\"utf-8\">\
<meta name=\"viewport\" content=\"width=device-width,initial-scale=1\">\
<title>{title}</title>\
<style>{style}</style>\
</head>\
<body>\
{nav}\
<main>{toolbar}{filter_panel}{fragment}</main>\
{search_index}\
<script>{script}</script>\
</body>\
</html>",
                title = html_escape(&model.title),
                style = STYLE,
                script = SCRIPT,
            );

            // 4. Output path: swap .md → .html.
            let out_path = if f.path.ends_with(".md") {
                format!("{}.html", &f.path[..f.path.len() - 3])
            } else {
                format!("{}.html", f.path)
            };

            RenderedFile {
                path: out_path,
                body: page,
            }
        })
        .collect();
    // One shared search index for the whole project, emitted once as a sibling
    // of the pages that reference it.
    out.push(RenderedFile {
        path: SEARCH_INDEX_FILE.to_string(),
        body: build_search_index_file(model),
    });
    out
}

#[cfg(test)]
mod tests {
    use super::escaping::{attr_escape, html_escape, html_escape_into};
    use super::graph::node_hrefs;
    use super::links::rewrite_md_links;
    use super::search::{SEARCH_INDEX_FILE, build_search_entries, search_index_json};
    use super::*;
    use crate::model::{DocModel, GroupDoc, SymbolDoc, SymbolDocKind};

    fn demo_model() -> DocModel {
        DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    anchor: "root-engine-speed".into(),
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
                references: vec![],
                children: vec![],
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        }
    }

    fn render_html(model: &DocModel) -> Vec<RenderedFile> {
        let md_files = crate::markdown::render(model);
        render(&md_files, model)
    }

    /// #37: a group with relationships renders the interactive force-graph
    /// widget (canvas + inline JSON), and the Mermaid fallback is swapped out —
    /// the HTML draws the diagram itself, with no library or CDN.
    #[test]
    fn relationships_become_self_contained_interactive_widget() {
        use crate::model::{EdgeKind, FunctionDoc, GraphEdge, ProjectGraph};
        let mut model = demo_model();
        model.groups[0].functions.push(FunctionDoc {
            path: "Root.Engine.Update".into(),
            anchor: "root-engine-update".into(),
            ..Default::default()
        });
        model.graph = ProjectGraph {
            edges: vec![GraphEdge {
                from: "Root.Engine.Update".into(),
                to: "Root.Engine.Speed".into(),
                kind: EdgeKind::Read,
            }],
        };
        let files = render_html(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains("<figure class=\"m1-graph\">"),
            "interactive graph figure missing"
        );
        assert!(page.body.contains("<canvas>"));
        assert!(page.body.contains("class=\"m1-graph-data\""));
        // A node links to its documentation page (deep-link, .html rewritten).
        assert!(page.body.contains("Root.Engine.html#root-engine-update"));
        // The Mermaid fallback was replaced; the HTML needs no Mermaid runtime.
        assert!(
            !page.body.contains("language-mermaid"),
            "Mermaid block should be swapped for the widget"
        );
        // Self-contained: the widget pulls nothing from the network.
        assert!(!page.body.contains("unpkg.com") && !page.body.contains("cdn"));
    }

    // #24: the symbol's deterministic anchor id survives from Markdown into the
    // HTML, so `Root.Engine.html#root-engine-speed` resolves to its row.
    #[test]
    fn symbol_anchor_id_survives_into_html() {
        let files = render_html(&demo_model());
        let page = files
            .iter()
            .find(|f| f.path == "Root.Engine.html")
            .expect("Root.Engine.html missing");
        assert!(
            page.body.contains("id=\"root-engine-speed\""),
            "expected the symbol anchor id in the HTML; got:\n{}",
            &page.body[..page.body.len().min(800)]
        );
    }

    // The sidebar nav is hand-built raw HTML, so a group/component name with
    // markup-significant characters (`&`, `<`, `>`, `"`) must be escaped in both
    // the visible label and the href. M1 names permit spaces and are not
    // restricted to alphanumerics, so this is a real corpus shape, not a
    // synthetic one. Without escaping, `Root.A & B` would emit a raw `&`,
    // producing a malformed entity reference and invalid HTML.
    #[test]
    fn nav_escapes_markup_in_component_names() {
        let mut model = demo_model();
        model.groups[0].path = "Root.A & B <x> \"q\"".into();
        // The single demo symbol's path is irrelevant to the nav; keep it valid.
        model.groups[0].symbols[0].path = "Root.A & B <x> \"q\".Speed".into();
        let files = render_html(&model);
        let nav = &files[0].body;
        let nav = &nav
            [nav.find("<nav>").expect("nav missing")..nav.find("</nav>").expect("nav end missing")];

        // `&` is escaped to `&amp;`, not left raw (the raw form would be a
        // malformed entity reference).
        assert!(
            nav.contains("&amp;"),
            "nav should escape '&' to '&amp;'; got:\n{nav}"
        );
        assert!(
            !nav.contains("A & B"),
            "nav must not contain a raw, unescaped '&'; got:\n{nav}"
        );
        // `<` / `>` / `"` from the name must not survive as literal markup.
        assert!(
            nav.contains("&lt;x&gt;"),
            "nav should escape '<x>' to '&lt;x&gt;'; got:\n{nav}"
        );
        assert!(
            nav.contains("&quot;q&quot;"),
            "nav should escape '\"q\"' to '&quot;q&quot;'; got:\n{nav}"
        );
        // The href path is escaped consistently with the on-disk filename
        // (`<group path>.html`, which keeps spaces verbatim), so escaping `&<>"`
        // but not spaces keeps the link pointing at the actual file.
        assert!(
            nav.contains("href=\"Root.A &amp; B &lt;x&gt; &quot;q&quot;.html\""),
            "nav href should be attribute-escaped to match the page filename; got:\n{nav}"
        );
    }

    // (a) Group page contains <table and the channel data.
    #[test]
    fn group_page_has_table_and_channel() {
        let files = render_html(&demo_model());
        let page = files
            .iter()
            .find(|f| f.path == "Root.Engine.html")
            .expect("Root.Engine.html missing");
        assert!(
            page.body.contains("<table"),
            "expected <table in group page; got:\n{}",
            &page.body[..page.body.len().min(500)]
        );
        assert!(
            page.body.contains("Root.Engine.Speed"),
            "expected channel name in group page; got:\n{}",
            &page.body[..page.body.len().min(500)]
        );
    }

    // (b) index.html contains a <nav> with href="Root.Engine.html".
    #[test]
    fn index_nav_has_html_link() {
        let files = render_html(&demo_model());
        let index = files
            .iter()
            .find(|f| f.path == "index.html")
            .expect("index.html missing");
        assert!(
            index.body.contains("<nav"),
            "expected <nav in index.html; got:\n{}",
            &index.body[..index.body.len().min(500)]
        );
        assert!(
            index.body.contains("href=\"Root.Engine.html\""),
            "expected href=\"Root.Engine.html\" in nav; got:\n{}",
            &index.body[..index.body.len().min(1000)]
        );
    }

    // (c) External http links are NOT rewritten.
    #[test]
    fn external_links_not_rewritten() {
        let html = r#"<a href="https://example.com/doc.md">ext</a>"#;
        let out = rewrite_md_links(html);
        assert_eq!(
            out, html,
            "external .md link must not be rewritten; got:\n{out}"
        );
    }

    // (c-extra) Relative .md links ARE rewritten.
    #[test]
    fn relative_md_links_are_rewritten() {
        let html = r#"<a href="Root.Engine.md">Engine</a>"#;
        let out = rewrite_md_links(html);
        assert!(
            out.contains("href=\"Root.Engine.html\""),
            "expected .md→.html rewrite; got:\n{out}"
        );
    }

    // (e) .md links with a fragment are rewritten; fragment is preserved.
    #[test]
    fn md_link_with_fragment_is_rewritten() {
        let html = r#"<a href="Root.Engine.md#section">Engine</a>"#;
        let out = rewrite_md_links(html);
        assert!(
            out.contains("href=\"Root.Engine.html#section\""),
            "expected .md#section→.html#section rewrite; got:\n{out}"
        );
    }

    // (f) .md links with a query string are rewritten; query is preserved.
    #[test]
    fn md_link_with_query_is_rewritten() {
        let html = r#"<a href="Root.Engine.md?v=1">Engine</a>"#;
        let out = rewrite_md_links(html);
        assert!(
            out.contains("href=\"Root.Engine.html?v=1\""),
            "expected .md?v=1→.html?v=1 rewrite; got:\n{out}"
        );
    }

    // (d) Every output path is a page (.html) or the shared search-index sidecar.
    #[test]
    fn all_output_paths_end_in_html() {
        let files = render_html(&demo_model());
        for f in &files {
            assert!(
                f.path.ends_with(".html") || f.path == SEARCH_INDEX_FILE,
                "expected .html path or the shared index, got: {}",
                f.path
            );
        }
    }

    // ---- richer fixture for #31 / #33 / #34 ----

    use crate::model::{EnumDoc, EnumMemberDoc, FunctionDoc, TableDoc};

    fn rich_model() -> DocModel {
        DocModel {
            title: "UQR-EV".into(),
            target_hardware: None,
            enums: vec![EnumDoc {
                name: "Switch".into(),
                anchor: "switch".into(),
                members: vec![
                    EnumMemberDoc {
                        name: "Off".into(),
                        value: 0,
                    },
                    EnumMemberDoc {
                        name: "On".into(),
                        value: 1,
                    },
                ],
                default: Some("Off".into()),
                open: false,
            }],
            groups: vec![
                GroupDoc {
                    path: "Root".into(),
                    references: vec![],
                    children: vec!["Root.Engine".into()],
                    ..Default::default()
                },
                GroupDoc {
                    path: "Root.Engine".into(),
                    symbols: vec![
                        SymbolDoc {
                            path: "Root.Engine.Speed".into(),
                            anchor: "root-engine-speed".into(),
                            kind: SymbolDocKind::Channel,
                            type_label: "f32".into(),
                            unit: Some("rpm".into()),
                            security: Some("Tune".into()),
                            tags: vec!["engine".into()],
                            ..Default::default()
                        },
                        SymbolDoc {
                            path: "Root.Engine.Gain".into(),
                            anchor: "root-engine-gain".into(),
                            kind: SymbolDocKind::Parameter,
                            type_label: "u16".into(),
                            security: Some("Calibration".into()),
                            tags: vec!["fuel".into()],
                            ..Default::default()
                        },
                    ],
                    functions: vec![FunctionDoc {
                        path: "Root.Engine.Update".into(),
                        anchor: "root-engine-update".into(),
                        source_text: Some("Out = In.Speed * 2; // double it\n".into()),
                        ..Default::default()
                    }],
                    tables: vec![TableDoc {
                        path: "Root.Engine.Map".into(),
                        anchor: "root-engine-map".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        }
    }

    // #31: the inline search index covers a known symbol with a resolvable
    // deep-link, and is embedded (no fetch) as application/json.
    #[test]
    fn search_index_contains_symbol_with_resolvable_anchor() {
        let json = search_index_json(&rich_model());
        // The channel is present with its group-page anchor href.
        assert!(
            json.contains("Root.Engine.Speed")
                && json.contains("Root.Engine.html#root-engine-speed"),
            "search index missing symbol/anchor; got:\n{json}"
        );
        // Functions, tables and enums are indexed too.
        assert!(json.contains("Root.Engine.Update"), "function missing");
        assert!(json.contains("Root.Engine.Map"), "table missing");
        assert!(
            json.contains("enums.html#switch"),
            "enum entry missing; got:\n{json}"
        );
    }

    // #31: the search index is a single shared sibling file, referenced (not
    // inlined) by every page, so a large project no longer pays O(pages ×
    // project) HTML. The file loads via a plain `<script src>` that works from
    // `file://`, keeping the site self-contained.
    #[test]
    fn search_index_is_shared_sibling_file_and_wired() {
        let files = render_html(&rich_model());
        // Exactly one shared index file is emitted, assigning the global the
        // behaviour script reads.
        let idx = files
            .iter()
            .filter(|f| f.path == "search-index.js")
            .collect::<Vec<_>>();
        assert_eq!(idx.len(), 1, "expected exactly one shared search-index.js");
        assert!(
            idx[0].body.contains("window.__M1_SEARCH_INDEX__="),
            "shared index must assign the global; got:\n{}",
            idx[0].body
        );
        // Every page references it via <script src> and no page inlines the
        // whole index as an application/json element any more.
        for f in files.iter().filter(|f| f.path.ends_with(".html")) {
            assert!(
                f.body.contains("<script src=\"search-index.js\">"),
                "page {} must reference the shared index",
                f.path
            );
            assert!(
                !f.body.contains("type=\"application/json\">[")
                    && !f.body.contains("id=\"search-index\""),
                "page {} must not inline the search index",
                f.path
            );
        }
        let index = files.iter().find(|f| f.path == "index.html").unwrap();
        assert!(
            index.body.contains("id=\"search-box\""),
            "search box missing from the shell"
        );
    }

    #[test]
    fn search_index_order_is_deterministic() {
        let a = search_index_json(&rich_model());
        let b = search_index_json(&rich_model());
        assert_eq!(a, b, "search index must be byte-identical across runs");
    }

    // The graph node-href map and the search index are both built from the one
    // `DocModel::anchored_entities` walk, so they cannot drift on which anchored
    // kinds carry a deep link (the historical bug: `node_hrefs` covered only
    // symbols/functions/references, silently missing any table/object/CAN node).
    // Every graph-eligible kind must now be deep-linked, with the same href the
    // search index uses; enums (not graph nodes) are excluded.
    #[test]
    fn node_hrefs_cover_every_graph_eligible_kind() {
        use crate::model::{CanMessageDoc, CanSignalDoc, ObjectDoc, ReferenceDoc};
        let mut model = rich_model();
        let eng = model
            .groups
            .iter_mut()
            .find(|g| g.path == "Root.Engine")
            .unwrap();
        eng.objects.push(ObjectDoc {
            path: "Root.Engine.Sensor".into(),
            anchor: "root-engine-sensor".into(),
            ..Default::default()
        });
        eng.references.push(ReferenceDoc {
            path: "Root.Engine.Alias".into(),
            anchor: "root-engine-alias".into(),
            ..Default::default()
        });
        eng.can_messages.push(CanMessageDoc {
            path: "Root.Engine.Frame".into(),
            anchor: "root-engine-frame".into(),
            signals: vec![CanSignalDoc {
                path: "Root.Engine.Frame.Rpm".into(),
                anchor: "root-engine-frame-rpm".into(),
                ..Default::default()
            }],
            ..Default::default()
        });

        let hrefs = node_hrefs(&model);

        // Symbols, functions, tables, objects, CAN messages + signals and
        // references all resolve to their <group>.html#<anchor> page link.
        assert_eq!(
            hrefs.get("Root.Engine.Speed").map(String::as_str),
            Some("Root.Engine.html#root-engine-speed")
        );
        assert_eq!(
            hrefs.get("Root.Engine.Update").map(String::as_str),
            Some("Root.Engine.html#root-engine-update")
        );
        assert_eq!(
            hrefs.get("Root.Engine.Map").map(String::as_str),
            Some("Root.Engine.html#root-engine-map"),
            "a table node must now deep-link (was silently missing)"
        );
        assert_eq!(
            hrefs.get("Root.Engine.Sensor").map(String::as_str),
            Some("Root.Engine.html#root-engine-sensor"),
            "an object node must now deep-link (was silently missing)"
        );
        assert_eq!(
            hrefs.get("Root.Engine.Frame").map(String::as_str),
            Some("Root.Engine.html#root-engine-frame"),
            "a CAN message node must now deep-link (was silently missing)"
        );
        assert_eq!(
            hrefs.get("Root.Engine.Frame.Rpm").map(String::as_str),
            Some("Root.Engine.html#root-engine-frame-rpm"),
            "a CAN signal node must now deep-link (was silently missing)"
        );
        assert_eq!(
            hrefs.get("Root.Engine.Alias").map(String::as_str),
            Some("Root.Engine.html#root-engine-alias")
        );

        // Enums live on the shared reference page and are not graph nodes, so the
        // node-href map (unlike the search index) omits them.
        assert!(
            !hrefs.contains_key("Switch"),
            "enums are not graph nodes and must not be in the node-href map"
        );

        // The node-href and search-index deep links agree for every shared key.
        for e in build_search_entries(&model) {
            if let Some(h) = hrefs.get(&e.path) {
                assert_eq!(
                    *h, e.href,
                    "search index and node-href map disagree on {}",
                    e.path
                );
            }
        }
    }

    // #33: the nav is a nested <ul> tree (the collapse JS toggles it), the page
    // carries the theme toggle, a TOC slot, and the M1-highlightable code class.
    #[test]
    fn nav_is_a_nested_tree() {
        let files = render_html(&rich_model());
        let index = files.iter().find(|f| f.path == "index.html").unwrap();
        // Root has Engine as a child → a nested <ul> inside the <li>.
        assert!(
            index
                .body
                .contains("<li><a href=\"Root.html\">Root</a><ul>"),
            "expected a nested nav tree; got nav around Root:\n{}",
            &index.body[..index.body.len().min(1200)]
        );
    }

    #[test]
    fn shell_has_theme_toggle_and_toc_slot() {
        let files = render_html(&rich_model());
        let page = files.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains("id=\"theme-toggle\""),
            "theme toggle missing"
        );
        assert!(page.body.contains("id=\"toc-slot\""), "TOC slot missing");
        assert!(
            page.body.contains("id=\"menu-toggle\""),
            "menu toggle missing"
        );
        // Dark mode follows prefers-color-scheme in the inline CSS.
        assert!(
            page.body.contains("prefers-color-scheme:dark"),
            "dark-mode media query missing from inline CSS"
        );
    }

    #[test]
    fn m1_source_block_is_highlightable() {
        // With an embedded ```m1 block, pulldown-cmark emits language-m1; the
        // inline highlighter keys off that class.
        use crate::markdown::{RenderOptions, render_with};
        let mut model = rich_model();
        // Force source embedding on the function.
        for g in &mut model.groups {
            for f in &mut g.functions {
                f.source_path = Some("Engine/Update.m1scr".into());
            }
        }
        let md = render_with(
            &model,
            &RenderOptions {
                source_base: None,
                include_source: true,
                graph: None,
            },
        );
        let html = render(&md, &model);
        let page = html.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains("language-m1"),
            "embedded source must carry the language-m1 class for highlighting"
        );
        assert!(
            page.body.contains("m1-kw") && page.body.contains("M1_KW"),
            "the inline highlighter script/CSS for M1 must be present"
        );
    }

    // #34: a security legend appears on the index; a filter panel with the
    // project's levels and tags appears on a group page; rows carry the filter
    // metadata.
    #[test]
    fn index_has_security_legend() {
        let files = render_html(&rich_model());
        let index = files.iter().find(|f| f.path == "index.html").unwrap();
        assert!(
            index.body.contains("Security levels"),
            "security legend missing from index"
        );
        assert!(
            index.body.contains("Tune") && index.body.contains("Calibration"),
            "legend must name each level present"
        );
    }

    #[test]
    fn group_page_has_filter_panel_and_row_metadata() {
        let files = render_html(&rich_model());
        let page = files.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains("id=\"filters\""),
            "filter panel missing from group page"
        );
        assert!(
            page.body.contains("data-sec=\"Tune\"")
                && page.body.contains("data-sec=\"Calibration\""),
            "security filter checkboxes missing"
        );
        assert!(
            page.body.contains("data-tag=\"engine\"") && page.body.contains("data-tag=\"fuel\""),
            "tag filter checkboxes missing"
        );
        // Rows carry the filter metadata the script reads.
        assert!(
            page.body.contains("data-security=\"Tune\"")
                && page.body.contains("data-tags=\"engine\""),
            "row filter metadata missing; got:\n{}",
            &page.body[..page.body.len().min(2400)]
        );
    }

    #[test]
    fn index_page_has_no_filter_panel() {
        let files = render_html(&rich_model());
        let index = files.iter().find(|f| f.path == "index.html").unwrap();
        assert!(
            !index.body.contains("id=\"filters\""),
            "the landing page has no filterable rows; it must not carry the panel"
        );
    }

    // Self-containment: no external asset URLs anywhere in any page (#33). We
    // only forbid asset-bearing schemes; an issue/source link in body text is
    // fine, but the shell (CSS/JS/index) must not reach the network.
    #[test]
    fn shell_has_no_external_asset_references() {
        let files = render_html(&rich_model());
        for f in &files {
            for needle in [
                "src=\"http",
                "href=\"http",
                "@import",
                "url(http",
                "cdn.",
                "googleapis",
                "unpkg",
                "jsdelivr",
            ] {
                assert!(
                    !f.body.contains(needle),
                    "page {} reaches an external asset ({needle})",
                    f.path
                );
            }
        }
    }

    // Determinism: rendering twice yields byte-identical pages (#33 guardrail).
    #[test]
    fn html_output_is_deterministic() {
        let a = render_html(&rich_model());
        let b = render_html(&rich_model());
        assert_eq!(a.len(), b.len());
        for (x, y) in a.iter().zip(b.iter()) {
            assert_eq!(x.path, y.path);
            assert_eq!(x.body, y.body, "page {} differs across runs", x.path);
        }
    }

    #[test]
    fn permalink_and_filter_css_are_inline() {
        let files = render_html(&rich_model());
        let page = files.iter().find(|f| f.path == "Root.Engine.html").unwrap();
        assert!(
            page.body.contains(".permalink") && page.body.contains("tr.filtered"),
            "permalink / filter CSS must be inline in the shell"
        );
    }

    // The page title is user-controlled (the `--title` flag, or the project's
    // parent directory name). HTML metacharacters in it must be escaped before
    // they reach the raw `<title>...</title>` in the head, the same way every
    // other text node in the shell is escaped — otherwise a title like
    // `A <b> & "C"` produces malformed/unescaped HTML on every generated page.
    #[test]
    fn title_with_html_metacharacters_is_escaped_in_head() {
        let mut model = demo_model();
        model.title = "A <b> & \"C\"".into();
        let files = render_html(&model);
        let page = files
            .iter()
            .find(|f| f.path == "Root.Engine.html")
            .expect("Root.Engine.html missing");
        assert!(
            page.body.contains("<title>A &lt;b&gt; &amp;"),
            "title metacharacters must be escaped in <title>; got head:\n{}",
            &page.body[..page.body.len().min(400)]
        );
        // The raw, unescaped tag must NOT leak into the head.
        assert!(
            !page.body.contains("<title>A <b>"),
            "raw unescaped <b> leaked into the page <title>"
        );
    }

    // The two public escapers share one implementation (a single `attr` flag),
    // so they can never drift apart on the common `& < >` set the way two
    // hand-rolled bodies could. This test pins that contract.
    #[test]
    fn text_and_attr_escapers_agree_except_on_double_quote() {
        let sample = "a & b <c> \"d\" e";

        // Both contexts always escape the markup-significant `& < >`.
        for esc in [html_escape(sample), attr_escape(sample)] {
            assert!(esc.contains("&amp;"), "'&' not escaped in: {esc}");
            assert!(esc.contains("&lt;c&gt;"), "'<c>' not escaped in: {esc}");
            assert!(!esc.contains(" & "), "raw '&' survived in: {esc}");
        }

        // The *only* difference is the attribute-delimiting double quote:
        // text context leaves it verbatim, attribute context escapes it.
        assert!(
            html_escape(sample).contains('"'),
            "text escaper must leave '\"' verbatim"
        );
        assert!(
            !html_escape(sample).contains("&quot;"),
            "text escaper must not escape '\"'"
        );
        assert!(
            attr_escape(sample).contains("&quot;"),
            "attribute escaper must escape '\"' to '&quot;'"
        );
        assert!(
            !attr_escape(sample).contains('"'),
            "attribute escaper must leave no raw '\"'"
        );

        // Apart from the `"` handling the two outputs are identical — proving
        // the single shared body. Replacing the escaped quote in the attr form
        // with a raw quote reconstructs the text form exactly.
        assert_eq!(
            attr_escape(sample).replace("&quot;", "\""),
            html_escape(sample),
            "the escapers must differ only by '\"'-handling"
        );
    }

    // Load-bearing: `attr_escape` must leave spaces verbatim so an href matches
    // the on-disk page filename (`<group path>.html` keeps spaces literal).
    #[test]
    fn attr_escape_leaves_spaces_verbatim() {
        assert_eq!(attr_escape("Root.A B"), "Root.A B");
    }

    // The shared lower-level routine appends to a caller-supplied buffer and
    // selects the attribute hardening via the `attr` flag.
    #[test]
    fn html_escape_into_appends_and_honours_attr_flag() {
        let mut out = String::from("pre:");
        html_escape_into(&mut out, "x \"y\" <z>", false);
        assert_eq!(out, "pre:x \"y\" &lt;z&gt;");

        let mut out = String::new();
        html_escape_into(&mut out, "x \"y\" <z>", true);
        assert_eq!(out, "x &quot;y&quot; &lt;z&gt;");
    }
}
