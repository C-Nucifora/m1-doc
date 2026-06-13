//! Renders Markdown files (produced by [`crate::markdown`]) to a self-contained
//! HTML site.  Each `.md` file becomes a `.html` file; intra-doc links are
//! rewritten from `*.md` to `*.html`.  External `http(s)://` links are left
//! untouched.  The only inputs are [`crate::markdown::RenderedFile`] slices and
//! a [`crate::model::DocModel`] (for the sidebar and page title).  No m1-core /
//! m1-typecheck types cross this module boundary.

use crate::markdown::RenderedFile;
use crate::model::DocModel;

// ---------------------------------------------------------------------------
// Internal CSS
// ---------------------------------------------------------------------------

const STYLE: &str = r#"
*,*::before,*::after{box-sizing:border-box}
body{margin:0;font-family:system-ui,sans-serif;font-size:16px;line-height:1.6;
     color:#1a1a1a;background:#fff;display:flex;min-height:100vh}
nav{width:220px;min-width:220px;background:#f4f4f4;border-right:1px solid #ddd;
    padding:1rem;position:sticky;top:0;height:100vh;overflow-y:auto}
nav h2{font-size:.85rem;text-transform:uppercase;letter-spacing:.06em;
        color:#666;margin:0 0 .5rem}
nav a{display:block;font-size:.9rem;color:#0055cc;text-decoration:none;
      padding:.15rem 0}
nav a:hover{text-decoration:underline}
main{flex:1;padding:2rem 3rem;max-width:900px}
h1{font-size:1.8rem;border-bottom:2px solid #ddd;padding-bottom:.4rem}
h2{font-size:1.3rem;margin-top:2rem}
h3{font-size:1.1rem}
table{border-collapse:collapse;width:100%;margin:1rem 0}
th,td{border:1px solid #ccc;padding:.4rem .7rem;text-align:left}
th{background:#f0f0f0;font-weight:600}
tr:nth-child(even) td{background:#fafafa}
code{background:#eef;padding:.1em .3em;border-radius:3px;font-size:.9em}
pre code{background:none;padding:0}
pre{background:#f6f6f6;padding:1rem;border-radius:4px;overflow-x:auto}
"#;

// ---------------------------------------------------------------------------
// Link rewriting
// ---------------------------------------------------------------------------

/// Rewrite relative `*.md` hrefs to `*.html`.  Operates on the raw HTML
/// string produced by pulldown-cmark.  Only touches `href="…"` attributes
/// whose values end with `.md` and do **not** start with `http://` or
/// `https://`.
fn rewrite_md_links(html: &str) -> String {
    // We scan byte-by-byte for the pattern  href="…"  to keep the
    // implementation simple and dependency-free.
    let needle = "href=\"";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(needle) {
        // Emit everything up to and including `href="`
        out.push_str(&rest[..pos + needle.len()]);
        rest = &rest[pos + needle.len()..];
        // Find the closing quote.
        if let Some(end) = rest.find('"') {
            let href = &rest[..end];
            if href.ends_with(".md")
                && !href.starts_with("http://")
                && !href.starts_with("https://")
            {
                // Replace the trailing `.md` with `.html`.
                out.push_str(&href[..href.len() - 3]);
                out.push_str(".html");
            } else {
                out.push_str(href);
            }
            out.push('"');
            rest = &rest[end + 1..];
        }
        // If no closing quote found the rest of the string is copied below.
    }
    out.push_str(rest);
    out
}

// ---------------------------------------------------------------------------
// Sidebar
// ---------------------------------------------------------------------------

fn build_nav(model: &DocModel) -> String {
    let mut nav = String::from("<nav><h2>Navigation</h2>");
    nav.push_str("<a href=\"index.html\">Index</a>");
    for g in &model.groups {
        let filename = format!("{}.html", g.path);
        nav.push_str(&format!("<a href=\"{filename}\">{path}</a>", path = g.path));
    }
    nav.push_str("</nav>");
    nav
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Convert a slice of Markdown [`RenderedFile`]s to HTML [`RenderedFile`]s.
///
/// For each input file:
/// - renders the Markdown body to an HTML fragment (tables enabled),
/// - wraps it in a minimal self-contained page with inline CSS and a sidebar,
/// - rewrites relative `*.md` hrefs to `*.html`,
/// - changes the output path from `*.md` to `*.html`.
pub fn render(markdown_files: &[RenderedFile], model: &DocModel) -> Vec<RenderedFile> {
    let nav = build_nav(model);
    markdown_files
        .iter()
        .map(|f| {
            // 1. Convert Markdown → HTML fragment (tables enabled).
            let mut fragment = String::new();
            let parser =
                pulldown_cmark::Parser::new_ext(&f.body, pulldown_cmark::Options::ENABLE_TABLES);
            pulldown_cmark::html::push_html(&mut fragment, parser);

            // 2. Rewrite intra-doc .md links → .html links.
            let fragment = rewrite_md_links(&fragment);

            // 3. Wrap in full page.
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
<main>{fragment}</main>\
</body>\
</html>",
                title = model.title,
                style = STYLE,
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
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DocModel, GroupDoc, SymbolDoc, SymbolDocKind};

    fn demo_model() -> DocModel {
        DocModel {
            title: "Demo".into(),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    unit: Some("rpm".into()),
                    security: None,
                }],
                functions: vec![],
            }],
        }
    }

    fn render_html(model: &DocModel) -> Vec<RenderedFile> {
        let md_files = crate::markdown::render(model);
        render(&md_files, model)
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

    // (d) Every output path ends in .html.
    #[test]
    fn all_output_paths_end_in_html() {
        let files = render_html(&demo_model());
        for f in &files {
            assert!(
                f.path.ends_with(".html"),
                "expected .html path, got: {}",
                f.path
            );
        }
    }
}
