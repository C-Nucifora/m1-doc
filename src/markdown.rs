//! Renders a [`DocModel`] to Markdown: one file per group plus an `index.md`.
//! This is the canonical output; the HTML renderer (P3) consumes these files.

use crate::model::{AnnotationDoc, DocModel, FunctionDoc, GroupDoc, SymbolDoc, SymbolDocKind};
use std::fmt::Write as _;

/// A rendered file: a project-relative path and its Markdown body.
pub struct RenderedFile {
    /// Project-relative path, e.g. `index.md` or `Root.Engine.md`.
    pub path: String,
    /// Full Markdown content ready to write to disk.
    pub body: String,
}

/// `Root.Engine` -> `Root.Engine.md` (a flat, link-safe filename).
fn group_filename(group_path: &str) -> String {
    format!("{group_path}.md")
}

/// Format one annotation as `@m1:<kind>(<args>)`, omitting the parens when
/// there are no args.
fn format_annotation(ann: &AnnotationDoc) -> String {
    if ann.args.is_empty() {
        format!("@m1:{}", ann.kind)
    } else {
        format!("@m1:{}({})", ann.kind, ann.args.join(", "))
    }
}

/// Render one function entry as a `### <path>` subsection with its input list
/// and, when present, an `**Annotations:**` block listing each `@m1:` annotation.
fn render_function(f: &FunctionDoc) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "### {}\n", f.path);
    if f.inputs.is_empty() {
        let _ = writeln!(out, "(no inputs)\n");
    } else {
        for (name, ty) in &f.inputs {
            let _ = writeln!(out, "- {name}: {ty}");
        }
        out.push('\n');
    }
    if !f.annotations.is_empty() {
        let _ = writeln!(out, "**Annotations:**\n");
        for ann in &f.annotations {
            let _ = writeln!(out, "- {}", format_annotation(ann));
        }
        out.push('\n');
    }
    out
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
                s.type_label,
                s.unit.as_deref().unwrap_or("—"),
                s.security.as_deref().unwrap_or("—"),
            );
        }
        out.push('\n');
    }
    if !group.functions.is_empty() {
        let _ = writeln!(out, "## Functions\n");
        for f in &group.functions {
            out.push_str(&render_function(f));
        }
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
    use crate::model::{DocModel, FunctionDoc, GroupDoc, SymbolDoc, SymbolDocKind};

    fn sample() -> DocModel {
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

    fn sample_with_functions() -> DocModel {
        DocModel {
            title: "Demo".into(),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![],
                functions: vec![
                    FunctionDoc {
                        path: "Root.Engine.Reset".into(),
                        inputs: vec![],
                        annotations: vec![],
                    },
                    FunctionDoc {
                        path: "Root.Engine.Update".into(),
                        inputs: vec![
                            ("Timeout".to_string(), "float".to_string()),
                            ("Enable".to_string(), "bool".to_string()),
                        ],
                        annotations: vec![],
                    },
                ],
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
            page.body
                .contains("| `Root.Engine.Speed` | f32 | rpm | — |"),
            "got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_with_no_functions_omits_functions_section() {
        let files = render(&sample());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            !page.body.contains("## Functions"),
            "must not emit Functions section when there are none; got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_renders_functions_section() {
        let files = render(&sample_with_functions());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("## Functions"),
            "missing Functions section; got:\n{}",
            page.body
        );
        // Function with no inputs shows "(no inputs)".
        assert!(
            page.body.contains("### Root.Engine.Reset"),
            "missing Reset heading; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("(no inputs)"),
            "missing (no inputs) for Reset; got:\n{}",
            page.body
        );
        // Function with inputs lists each param as "- name: type".
        assert!(
            page.body.contains("### Root.Engine.Update"),
            "missing Update heading; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- Timeout: float"),
            "missing Timeout param; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- Enable: bool"),
            "missing Enable param; got:\n{}",
            page.body
        );
    }

    #[test]
    fn function_with_annotations_renders_annotation_list() {
        use crate::model::AnnotationDoc;
        let model = DocModel {
            title: "Demo".into(),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![],
                functions: vec![FunctionDoc {
                    path: "Root.Engine.Update".into(),
                    inputs: vec![],
                    annotations: vec![
                        AnnotationDoc {
                            kind: "requires-finite".into(),
                            args: vec![],
                        },
                        AnnotationDoc {
                            kind: "allow".into(),
                            args: vec!["L010".into()],
                        },
                    ],
                }],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("**Annotations:**"),
            "missing Annotations label; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- @m1:requires-finite"),
            "missing requires-finite annotation; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- @m1:allow(L010)"),
            "missing allow(L010) annotation; got:\n{}",
            page.body
        );
    }

    #[test]
    fn function_without_annotations_omits_annotation_section() {
        let files = render(&sample_with_functions());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            !page.body.contains("**Annotations:**"),
            "must not emit Annotations when there are none; got:\n{}",
            page.body
        );
    }
}
