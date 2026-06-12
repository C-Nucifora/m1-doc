//! Renders a [`DocModel`] to Markdown: one file per group plus an `index.md`.
//! This is the canonical output; the HTML renderer (P3) consumes these files.

use crate::model::{DocModel, GroupDoc, SymbolDoc, SymbolDocKind};
use std::fmt::Write as _;

/// A rendered file: a project-relative path and its Markdown body.
pub struct RenderedFile {
    pub path: String,
    pub body: String,
}

/// `Root.Engine` -> `Root.Engine.md` (a flat, link-safe filename).
fn group_filename(group_path: &str) -> String {
    format!("{group_path}.md")
}

fn render_group(group: &GroupDoc) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", group.path);
    for kind in [
        SymbolDocKind::Channel,
        SymbolDocKind::Parameter,
        SymbolDocKind::Constant,
    ] {
        let rows: Vec<&SymbolDoc> = group.symbols.iter().filter(|s| s.kind == kind).collect();
        if rows.is_empty() {
            continue;
        }
        let _ = writeln!(out, "## {}\n", kind.plural());
        let _ = writeln!(out, "| Name | Type | Unit | Security |");
        let _ = writeln!(out, "| --- | --- | --- | --- |");
        for s in rows {
            let _ = writeln!(
                out,
                "| `{}` | {} | {} | {} |",
                s.path,
                s.type_label.as_deref().unwrap_or("—"),
                s.unit.as_deref().unwrap_or("—"),
                s.security.as_deref().unwrap_or("—"),
            );
        }
        out.push('\n');
    }
    out
}

fn render_index(model: &DocModel) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "# {}\n", model.title);
    let _ = writeln!(out, "## Groups\n");
    for g in &model.groups {
        let _ = writeln!(out, "- [{}]({})", g.path, group_filename(&g.path));
    }
    out
}

/// Render the whole model. Always emits `index.md` first, then one file per
/// group in model order (already sorted by the loader).
pub fn render(model: &DocModel) -> Vec<RenderedFile> {
    let mut files = vec![RenderedFile {
        path: "index.md".to_string(),
        body: render_index(model),
    }];
    for g in &model.groups {
        files.push(RenderedFile {
            path: group_filename(&g.path),
            body: render_group(g),
        });
    }
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DocModel, GroupDoc, SymbolDoc, SymbolDocKind};

    fn sample() -> DocModel {
        DocModel {
            title: "Demo".into(),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: Some("f32".into()),
                    unit: Some("rpm".into()),
                    security: None,
                }],
            }],
        }
    }

    #[test]
    fn index_links_each_group() {
        let files = render(&sample());
        let index = &files[0];
        assert_eq!(index.path, "index.md");
        assert!(
            index.body.contains("[Root.Engine](Root.Engine.md)"),
            "got:\n{}",
            index.body
        );
    }

    #[test]
    fn group_page_tables_its_channels() {
        let files = render(&sample());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(page.body.contains("## Channels"), "got:\n{}", page.body);
        assert!(
            page.body.contains("| `Root.Engine.Speed` | f32 | rpm | — |"),
            "got:\n{}",
            page.body
        );
    }
}
