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

/// Render one function entry as a `### <path>` subsection with its call rate,
/// input list, optional return type, and, when present, an `**Annotations:**`
/// block listing each `@m1:` annotation.
fn render_function(f: &FunctionDoc) -> String {
    let mut out = String::new();
    // Explicit, deterministic anchor (our scheme — not pulldown-cmark's
    // incidental heading slug) so `<group>.md#<anchor>` is stable.
    let _ = writeln!(out, "<a id=\"{}\"></a>\n", f.anchor);
    let _ = writeln!(out, "### {}\n", f.path);
    let _ = writeln!(out, "**Call rate:** {}\n", format_rate(f.call_rate_hz));
    if f.inputs.is_empty() {
        let _ = writeln!(out, "(no inputs)\n");
    } else {
        for (name, ty) in &f.inputs {
            let _ = writeln!(out, "- {name}: {ty}");
        }
        out.push('\n');
    }
    if let Some(rt) = &f.return_type {
        let _ = writeln!(out, "**Returns:** {rt}\n");
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
        let _ = writeln!(
            out,
            "| Name | Type | Quantity | Unit | Base | Log rate | Security |"
        );
        let _ = writeln!(out, "| --- | --- | --- | --- | --- | --- | --- |");
        for s in rows {
            // Show the base unit only when it differs from the display unit —
            // collapse the redundant case (and when either is absent).
            let base = match (s.unit.as_deref(), s.base_unit.as_deref()) {
                (Some(disp), Some(base)) if disp != base => base,
                _ => "—",
            };
            // Leading inline anchor in the Name cell makes the row linkable as
            // `<group>.md#<anchor>`; it carries into the HTML table verbatim.
            let _ = writeln!(
                out,
                "| <a id=\"{}\"></a>`{}` | {} | {} | {} | {} | {} | {} |",
                s.anchor,
                s.path,
                s.type_label,
                s.quantity.as_deref().unwrap_or("—"),
                s.unit.as_deref().unwrap_or("—"),
                base,
                format_rate(s.log_rate_hz),
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
                    ..Default::default()
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
                        return_type: None,
                        annotations: vec![],
                        ..Default::default()
                    },
                    FunctionDoc {
                        path: "Root.Engine.Update".into(),
                        inputs: vec![
                            ("Timeout".to_string(), "float".to_string()),
                            ("Enable".to_string(), "bool".to_string()),
                        ],
                        return_type: None,
                        annotations: vec![],
                        ..Default::default()
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
                .contains("`Root.Engine.Speed` | f32 | — | rpm | — | — | — |"),
            "got:\n{}",
            page.body
        );
    }

    fn sample_with_constant() -> DocModel {
        DocModel {
            title: "Demo".into(),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.MaxRpm".into(),
                    kind: SymbolDocKind::Constant,
                    type_label: "u16".into(),
                    unit: None,
                    security: None,
                    ..Default::default()
                }],
                functions: vec![],
            }],
        }
    }

    /// A group containing a Constant symbol must render a `## Constants` section
    /// and include the constant's row in the table. Removing the
    /// `SymbolDocKind::Constant` branch from `render_group` would cause this test
    /// to fail.
    #[test]
    fn group_page_tables_its_constants() {
        let files = render(&sample_with_constant());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("## Constants"),
            "expected Constants section; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("`Root.Engine.MaxRpm` | u16 | — | — |"),
            "expected constant row; got:\n{}",
            page.body
        );
        // Channels and Parameters sections must be absent when there are none.
        assert!(
            !page.body.contains("## Channels"),
            "must not emit Channels when there are none; got:\n{}",
            page.body
        );
        assert!(
            !page.body.contains("## Parameters"),
            "must not emit Parameters when there are none; got:\n{}",
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
                    return_type: None,
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
                    ..Default::default()
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

    #[test]
    fn function_with_return_type_renders_returns_line() {
        let model = DocModel {
            title: "Demo".into(),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![],
                functions: vec![FunctionDoc {
                    path: "Root.Engine.Compute".into(),
                    inputs: vec![("X".to_string(), "float".to_string())],
                    return_type: Some("float".to_string()),
                    annotations: vec![],
                    ..Default::default()
                }],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("**Returns:** float"),
            "missing Returns line; got:\n{}",
            page.body
        );
    }

    #[test]
    fn function_without_return_type_omits_returns_line() {
        let files = render(&sample_with_functions());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            !page.body.contains("**Returns:**"),
            "must not emit Returns when return_type is None; got:\n{}",
            page.body
        );
    }

    // ---- #25: rate / quantity / base-vs-display-unit surfacing ----

    #[test]
    fn format_rate_trims_trailing_zeros_and_handles_none() {
        assert_eq!(format_rate(Some(200.0)), "200 Hz");
        assert_eq!(format_rate(Some(0.5)), "0.5 Hz");
        assert_eq!(format_rate(Some(12.25)), "12.25 Hz");
        assert_eq!(format_rate(None), "—");
    }

    #[test]
    fn group_table_shows_quantity_log_rate_and_base_only_when_it_differs() {
        // Display unit (rpm) differs from the stored base (rad/s) → both shown;
        // the channel carries a quantity and a log rate.
        let model = DocModel {
            title: "Demo".into(),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![
                    SymbolDoc {
                        path: "Root.Engine.Speed".into(),
                        kind: SymbolDocKind::Channel,
                        type_label: "f32".into(),
                        quantity: Some("rad/s".into()),
                        unit: Some("rpm".into()),
                        base_unit: Some("rad/s".into()),
                        log_rate_hz: Some(200.0),
                        security: None,
                        ..Default::default()
                    },
                    // Display == base → Base column collapses to "—".
                    SymbolDoc {
                        path: "Root.Engine.Load".into(),
                        kind: SymbolDocKind::Channel,
                        type_label: "f32".into(),
                        quantity: None,
                        unit: Some("%".into()),
                        base_unit: Some("%".into()),
                        log_rate_hz: None,
                        security: None,
                        ..Default::default()
                    },
                ],
                functions: vec![],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body
                .contains("`Root.Engine.Speed` | f32 | rad/s | rpm | rad/s | 200 Hz | — |"),
            "rate/quantity/base not surfaced; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("`Root.Engine.Load` | f32 | — | % | — | — | — |"),
            "base must collapse when identical to display; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("| Name | Type | Quantity | Unit | Base | Log rate | Security |"),
            "table header missing new columns; got:\n{}",
            page.body
        );
    }

    #[test]
    fn rows_and_functions_emit_their_stable_anchor(/* #24 */) {
        let model = DocModel {
            title: "Demo".into(),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    anchor: "root-engine-speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    ..Default::default()
                }],
                functions: vec![FunctionDoc {
                    path: "Root.Engine.Update".into(),
                    anchor: "root-engine-update".into(),
                    ..Default::default()
                }],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        // Symbol row carries a leading inline anchor → `Root.Engine.md#root-engine-speed`.
        assert!(
            page.body
                .contains("| <a id=\"root-engine-speed\"></a>`Root.Engine.Speed`"),
            "symbol row missing its anchor; got:\n{}",
            page.body
        );
        // Function uses our explicit anchor, not pulldown-cmark's heading slug.
        assert!(
            page.body.contains("<a id=\"root-engine-update\"></a>"),
            "function missing its anchor; got:\n{}",
            page.body
        );
    }

    #[test]
    fn function_renders_call_rate_and_dash_when_absent() {
        let model = DocModel {
            title: "Demo".into(),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![],
                functions: vec![
                    FunctionDoc {
                        path: "Root.Engine.Update".into(),
                        call_rate_hz: Some(100.0),
                        ..Default::default()
                    },
                    FunctionDoc {
                        path: "Root.Engine.Init".into(),
                        call_rate_hz: None,
                        ..Default::default()
                    },
                ],
            }],
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("### Root.Engine.Update")
                && page.body.contains("**Call rate:** 100 Hz"),
            "triggered function must show its call rate; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("**Call rate:** —"),
            "untriggered function must show — ; got:\n{}",
            page.body
        );
    }
}
