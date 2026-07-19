//! Renders a [`DocModel`] to Markdown: one file per group plus an `index.md`.
//! This is the canonical output; the HTML renderer (P3) consumes these files.
//!
//! The renderer is split into section modules behind small interfaces:
//! [`helpers`] (leaf format/path helpers and the `SourceLinker`), [`entries`]
//! (the per-entity section renderers for a group's members), [`group`] (the
//! group page plus its cross-reference and relationship machinery), and
//! [`pages`] (the index, per-tag, Enums, and subsystem-graph pages). This
//! module owns the public API types and composes those sections into the final
//! set of files.

mod entries;
mod group;
mod helpers;
mod pages;

use crate::model::DocModel;
use std::collections::HashMap;

use group::{build_cross_refs, render_group};
use helpers::{ENUMS_FILE, SourceLinker, graph_page_filename, group_filename, tag_filename};
use pages::{render_enums, render_graph_page, render_index, render_tag_index};

/// A rendered file: a project-relative path and its Markdown body.
pub struct RenderedFile {
    /// Project-relative path, e.g. `index.md` or `Root.Engine.md`.
    pub path: String,
    /// Full Markdown content ready to write to disk.
    pub body: String,
}

/// Render-time options driven by CLI flags (#30). `source_base`, when set, turns
/// a function's source path into an external link (e.g. a GitHub blob URL);
/// `include_source` embeds the script body in a collapsible block.
#[derive(Debug, Clone, Default)]
pub struct RenderOptions {
    /// Base URL prepended to a function's `source_path` to build a source link
    /// (trailing slash optional). `None` → the path is shown as plain text.
    pub source_base: Option<String>,
    /// When `true`, embed each function's script body in a `<details>` block.
    pub include_source: bool,
    /// `--graph <group>`: emit a focused subsystem-graph page for this group at
    /// [`GraphSpec::depth`] (#37). `None` → no extra page.
    pub graph: Option<GraphSpec>,
}

/// A `--graph <group>` request: the group path to focus and how many hops to
/// expand across its boundary.
#[derive(Debug, Clone)]
pub struct GraphSpec {
    pub group: String,
    pub depth: usize,
}

/// Render the whole model with default options (no source links, no embedded
/// source). Convenience wrapper over [`render_with`] — the binary always passes
/// explicit options, so this is used by the tests and the HTML test harness.
#[cfg(test)]
pub fn render(model: &DocModel) -> Vec<RenderedFile> {
    render_with(model, &RenderOptions::default())
}

/// Render the whole model. Always emits `index.md` first, then one file per
/// group in model order (already sorted by the loader), then the Enums
/// reference page when the project uses any enums. `opts` controls function
/// source links and embedding (#30).
pub fn render_with(model: &DocModel, opts: &RenderOptions) -> Vec<RenderedFile> {
    // name -> anchor for linking enum-typed symbols to the reference.
    let enum_anchors: HashMap<&str, &str> = model
        .enums
        .iter()
        .map(|e| (e.name.as_str(), e.anchor.as_str()))
        .collect();
    // Cross-reference link tables, built once from the whole model (#29).
    let xrefs = build_cross_refs(model);
    // Jump-to-declaration linker, built once: the project path is constant for
    // every symbol, only `def_line` varies (#57).
    let links = SourceLinker {
        base: opts
            .source_base
            .as_deref()
            .map(|b| b.trim_end_matches('/').to_string()),
        m1prj_path: model.m1prj_path.as_deref(),
    };
    let mut files = vec![RenderedFile {
        path: "index.md".to_string(),
        body: render_index(model, opts),
    }];
    for g in &model.groups {
        files.push(RenderedFile {
            path: group_filename(&g.path),
            body: render_group(g, &enum_anchors, &xrefs, &model.graph, opts, &links),
        });
    }
    // Focused `--graph <group>` subsystem page (#37), when requested.
    if let Some(spec) = &opts.graph {
        files.push(RenderedFile {
            path: graph_page_filename(&spec.group),
            body: render_graph_page(model, spec),
        });
    }
    // One index page per tag (#34), in sorted tag order so the output is
    // deterministic. Each lists every symbol carrying the tag, deep-linked.
    for tag in model.tags() {
        files.push(RenderedFile {
            path: tag_filename(&tag),
            body: render_tag_index(model, &tag),
        });
    }
    if !model.enums.is_empty() {
        files.push(RenderedFile {
            path: ENUMS_FILE.to_string(),
            body: render_enums(&model.enums),
        });
    }
    files
}

#[cfg(test)]
mod tests {
    use super::entries::render_function;
    use super::helpers::format_rate;
    use super::*;
    use crate::model::{
        DocModel, EdgeKind, EnumDoc, EnumMemberDoc, FunctionDoc, GraphEdge, GroupDoc, ProjectGraph,
        SymbolDoc, SymbolDocKind, TableAxisDoc, TableDoc,
    };

    fn sample() -> DocModel {
        DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
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

    fn sample_with_functions() -> DocModel {
        DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
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

    // ---- #32: overview landing page ----

    fn landing_model() -> DocModel {
        DocModel {
            title: "UQR-EV".into(),
            target_hardware: None,
            enums: vec![EnumDoc {
                name: "Switch".into(),
                anchor: "switch".into(),
                ..Default::default()
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
                            kind: SymbolDocKind::Channel,
                            type_label: "f32".into(),
                            security: Some("Tune".into()),
                            ..Default::default()
                        },
                        SymbolDoc {
                            path: "Root.Engine.Gain".into(),
                            kind: SymbolDocKind::Parameter,
                            type_label: "u16".into(),
                            security: Some("Calibration".into()),
                            tags: vec!["fuel".into()],
                            ..Default::default()
                        },
                    ],
                    ..Default::default()
                },
            ],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        }
    }

    #[test]
    fn index_shows_summary_stats_line() {
        let files = render(&landing_model());
        let index = &files[0];
        // Headline title + a stats line with the headline counts.
        assert!(index.body.contains("# UQR-EV"), "got:\n{}", index.body);
        assert!(
            index.body.contains("2 components")
                && index.body.contains("1 channel")
                && index.body.contains("1 parameter")
                && index.body.contains("1 top-level group"),
            "stats line missing counts; got:\n{}",
            index.body
        );
    }

    #[test]
    fn index_notes_unknown_target_hardware() {
        let files = render(&landing_model());
        let index = &files[0];
        // Degrade-never-fake: target hardware is not exposed by the project API.
        assert!(
            index.body.contains("Target hardware")
                && index.body.contains("not exposed by the project"),
            "target-hardware degrade note missing; got:\n{}",
            index.body
        );
    }

    #[test]
    fn index_renders_group_tree_with_per_group_counts() {
        let files = render(&landing_model());
        let index = &files[0];
        // Nested tree: the forest root links to its page and shows a count.
        assert!(
            index.body.contains("## Structure"),
            "structure section missing; got:\n{}",
            index.body
        );
        assert!(
            index.body.contains("[Root](Root.md)"),
            "tree must link the forest root; got:\n{}",
            index.body
        );
        // Root's subtree rolls up the two members under Root.Engine, and the
        // node is marked expandable.
        assert!(
            index.body.contains("[Root](Root.md) (2) ▸"),
            "per-group subtree count / expandable marker missing; got:\n{}",
            index.body
        );
    }

    // ---- #34: security legend + tag column + tag index pages ----

    #[test]
    fn index_renders_security_legend_for_levels_present() {
        let files = render(&landing_model());
        let index = &files[0];
        assert!(
            index.body.contains("## Security levels"),
            "legend heading missing; got:\n{}",
            index.body
        );
        assert!(
            index.body.contains("Calibration") && index.body.contains("Tune"),
            "legend must list each level present; got:\n{}",
            index.body
        );
    }

    #[test]
    fn tag_index_page_links_tagged_symbols() {
        let files = render(&landing_model());
        // A per-tag index page is emitted for the `fuel` tag.
        let page = files
            .iter()
            .find(|f| f.path == "tag.fuel.md")
            .expect("expected a tag.fuel.md index page");
        assert!(
            page.body.contains("Root.Engine.Gain") && page.body.contains("Root.Engine.md#"),
            "tag page must deep-link its tagged symbols; got:\n{}",
            page.body
        );
        // The index links the tag facet.
        let index = &files[0];
        assert!(
            index.body.contains("[fuel](tag.fuel.md)"),
            "index must link the tag facet; got:\n{}",
            index.body
        );
    }

    #[test]
    fn group_table_shows_tags_column_only_when_tagged() {
        let files = render(&landing_model());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        // The Parameters section has a tagged row → a Tags column appears there.
        assert!(
            page.body.contains("| Tags |"),
            "Tags column header missing when a row is tagged; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("`fuel`"),
            "tag value missing from row; got:\n{}",
            page.body
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
            target_hardware: None,
            enums: vec![],
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
            target_hardware: None,
            enums: vec![],
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
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                references: vec![],
                children: vec![],
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
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
            target_hardware: None,
            enums: vec![],
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
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                references: vec![],
                children: vec![],
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
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
            target_hardware: None,
            enums: vec![],
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
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                references: vec![],
                children: vec![],
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
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
    fn enums_reference_lists_closed_enum_with_members_and_default(/* #27 */) {
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            groups: vec![],
            enums: vec![EnumDoc {
                name: "MoTeC Types.Switch".into(),
                anchor: "motec-types-switch".into(),
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
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files
            .iter()
            .find(|f| f.path == "enums.md")
            .expect("enums.md should be emitted");
        assert!(
            page.body.contains("## MoTeC Types.Switch (default: Off)"),
            "closed enum heading wrong; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("<a id=\"motec-types-switch\"></a>")
                // Each enumerator is rendered `value = name`, with the default
                // marked — the numeric value is part of what the enum is.
                && page.body.contains("- 0 = Off (default)")
                && page.body.contains("- 1 = On"),
            "members/anchor/values missing; got:\n{}",
            page.body
        );
        // The index links the reference.
        let index = &files[0];
        assert!(
            index.body.contains("[Enums](enums.md)"),
            "index should link the enums reference; got:\n{}",
            index.body
        );
    }

    /// The manual defines an enum as a value→name mapping (its canonical
    /// example is `-1 = Error`, `0 = Stopped (default)`, `1 = Cranking`, …), so
    /// the Enums reference must render the numeric value of every enumerator —
    /// including negative ones — and mark the default.
    #[test]
    fn enums_reference_shows_numeric_value_of_each_enumerator() {
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            groups: vec![],
            enums: vec![EnumDoc {
                name: "Engine State".into(),
                anchor: "engine-state".into(),
                members: vec![
                    EnumMemberDoc {
                        name: "Error".into(),
                        value: -1,
                    },
                    EnumMemberDoc {
                        name: "Stopped".into(),
                        value: 0,
                    },
                    EnumMemberDoc {
                        name: "Cranking".into(),
                        value: 1,
                    },
                ],
                default: Some("Stopped".into()),
                open: false,
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "enums.md").unwrap();
        assert!(
            page.body.contains("- -1 = Error")
                && page.body.contains("- 0 = Stopped (default)")
                && page.body.contains("- 1 = Cranking"),
            "each enumerator must render `value = name` (default marked); got:\n{}",
            page.body
        );
    }

    #[test]
    fn open_enum_is_labelled_partial(/* #27 */) {
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            groups: vec![],
            enums: vec![EnumDoc {
                name: "Gear State".into(),
                anchor: "gear-state".into(),
                members: vec![EnumMemberDoc {
                    name: "Neutral".into(),
                    value: 0,
                }],
                default: None,
                open: true,
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "enums.md").unwrap();
        assert!(
            page.body
                .contains("open — firmware-supplied, members may be partial"),
            "open enum must be labelled partial; got:\n{}",
            page.body
        );
    }

    #[test]
    fn enum_typed_symbol_links_to_its_reference_entry(/* #27 */) {
        let model = DocModel {
            title: "Demo".into(),
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
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Mode".into(),
                    anchor: "root-engine-mode".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "Switch".into(),
                    enum_ref: Some("Switch".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("[Switch](enums.md#switch)"),
            "enum-typed symbol must link to its reference; got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_renders_tables_section_with_dimensionality(/* #26 */) {
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                tables: vec![TableDoc {
                    path: "Root.Engine.IgnitionMap".into(),
                    anchor: "root-engine-ignitionmap".into(),
                    axes: vec![
                        TableAxisDoc {
                            size: 16,
                            unit: Some("rpm".into()),
                        },
                        TableAxisDoc {
                            size: 12,
                            unit: Some("kPa".into()),
                        },
                    ],
                    output_unit: Some("deg".into()),
                    def_line: None,
                }],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(page.body.contains("## Tables"), "got:\n{}", page.body);
        assert!(
            page.body.contains("### Root.Engine.IgnitionMap"),
            "got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("2-D table — 16 (rpm) × 12 (kPa) → deg"),
            "dimensionality line wrong; got:\n{}",
            page.body
        );
        // Tables are anchored like every other entity (#24).
        assert!(
            page.body.contains("<a id=\"root-engine-ignitionmap\"></a>"),
            "table anchor missing; got:\n{}",
            page.body
        );
    }

    #[test]
    fn table_without_cfg_metadata_is_still_listed(/* #26 */) {
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                tables: vec![TableDoc {
                    path: "Root.Engine.FuelMap".into(),
                    anchor: "root-engine-fuelmap".into(),
                    axes: vec![],
                    output_unit: None,
                    def_line: None,
                }],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body.contains("### Root.Engine.FuelMap")
                && page.body.contains("shape requires a loaded `.m1cfg`"),
            "unshaped table must still be listed; got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_has_breadcrumb_and_subgroup_links(/* #23 */) {
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine.Fuel".into(),
                references: vec![],
                children: vec!["Root.Engine.Fuel.Pump".into()],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files
            .iter()
            .find(|f| f.path == "Root.Engine.Fuel.md")
            .unwrap();
        // Breadcrumb: ancestors are links, the current segment is plain.
        assert!(
            page.body
                .contains("[Root](Root.md) › [Engine](Root.Engine.md) › Fuel"),
            "breadcrumb wrong; got:\n{}",
            page.body
        );
        // Sub-groups section links each child by its leaf label.
        assert!(
            page.body.contains("## Sub-groups")
                && page.body.contains("[Pump](Root.Engine.Fuel.Pump.md)"),
            "sub-groups missing; got:\n{}",
            page.body
        );
    }

    #[test]
    fn index_links_only_forest_roots_not_every_node(/* #23 */) {
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![
                GroupDoc {
                    path: "Root".into(),
                    references: vec![],
                    children: vec!["Root.Engine".into()],
                    ..Default::default()
                },
                GroupDoc {
                    path: "Root.Engine".into(),
                    ..Default::default()
                },
            ],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let index = &files[0];
        assert!(
            index.body.contains("[Root](Root.md)"),
            "got:\n{}",
            index.body
        );
        // Root.Engine is reachable by descending, not listed at the index top level.
        assert!(
            !index.body.contains("[Root.Engine](Root.Engine.md)"),
            "index must not flat-list child groups; got:\n{}",
            index.body
        );
    }

    #[test]
    fn rows_and_functions_emit_their_stable_anchor(/* #24 */) {
        let model = DocModel {
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
                    ..Default::default()
                }],
                functions: vec![FunctionDoc {
                    path: "Root.Engine.Update".into(),
                    anchor: "root-engine-update".into(),
                    ..Default::default()
                }],
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                references: vec![],
                children: vec![],
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        // Symbol row carries a leading inline anchor → `Root.Engine.md#root-engine-speed`.
        // The anchor also carries the filter class (#34).
        assert!(
            page.body.contains(
                "| <a id=\"root-engine-speed\" class=\"m1-row-anchor\"></a>`Root.Engine.Speed`"
            ),
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
            target_hardware: None,
            enums: vec![],
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
                tables: vec![],
                objects: vec![],
                can_messages: vec![],
                references: vec![],
                children: vec![],
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
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

    // ---- #28: objects, CAN, classname column ----

    #[test]
    fn group_page_renders_objects_with_class_and_members() {
        use crate::model::ObjectDoc;
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Inputs".into(),
                objects: vec![ObjectDoc {
                    path: "Root.Inputs.OilP".into(),
                    anchor: "root-inputs-oilp".into(),
                    class: Some("MoTeC Input.Sensor".into()),
                    members: vec![
                        "Root.Inputs.OilP.Calibration".into(),
                        "Root.Inputs.OilP.Resource".into(),
                    ],
                    def_line: None,
                }],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Inputs.md").unwrap();
        assert!(page.body.contains("## Objects"), "got:\n{}", page.body);
        assert!(
            page.body.contains("### Root.Inputs.OilP")
                && page.body.contains("**Class:** MoTeC Input.Sensor"),
            "object class missing; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("- `Root.Inputs.OilP.Resource`"),
            "object members missing; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("<a id=\"root-inputs-oilp\"></a>"),
            "object anchor missing; got:\n{}",
            page.body
        );
    }

    #[test]
    fn group_page_renders_can_message_and_signal_layout() {
        use crate::model::{CanMessageDoc, CanSignalDoc};
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Bus".into(),
                can_messages: vec![CanMessageDoc {
                    path: "Bus.EngineData".into(),
                    anchor: "bus-enginedata".into(),
                    can_id: Some(160),
                    dlc: Some(8),
                    signals: vec![CanSignalDoc {
                        path: "Bus.EngineData.EngineSpeed".into(),
                        anchor: "bus-enginedata-enginespeed".into(),
                        start_bit: Some(24),
                        length: Some(16),
                        multiplier: Some(0.5),
                        offset: Some(0.0),
                        range: Some((0.0, 8000.0)),
                        unit: Some("rpm".into()),
                    }],
                    def_line: None,
                }],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Bus.md").unwrap();
        assert!(page.body.contains("## CAN"), "got:\n{}", page.body);
        // Frame id is shown in hex with the dlc.
        assert!(
            page.body.contains("### Bus.EngineData (id 0xA0, dlc 8)"),
            "message frame line wrong; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("| Signal | Bits | Scale | Range | Unit |"),
            "signal table header missing; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("`EngineSpeed` | @24, 16 | ×0.5 +0 | 0 .. 8000 | rpm |"),
            "signal row wrong; got:\n{}",
            page.body
        );
    }

    #[test]
    fn can_message_with_no_signals_is_still_listed() {
        use crate::model::CanMessageDoc;
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Bus".into(),
                can_messages: vec![CanMessageDoc {
                    path: "Bus.Empty".into(),
                    anchor: "bus-empty".into(),
                    can_id: None,
                    dlc: None,
                    signals: vec![],
                    def_line: None,
                }],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Bus.md").unwrap();
        assert!(
            page.body.contains("### Bus.Empty (id —, dlc —)") && page.body.contains("(no signals)"),
            "empty message must degrade, not drop; got:\n{}",
            page.body
        );
    }

    #[test]
    fn class_column_appears_only_for_non_plain_classnames() {
        // One plain BuiltIn.Channel and one sensor-resource channel
        // (MoTeC Input.Sensor.Resource) — the section must show a Class column.
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Inputs".into(),
                symbols: vec![
                    SymbolDoc {
                        path: "Root.Inputs.Plain".into(),
                        kind: SymbolDocKind::Channel,
                        type_label: "f32".into(),
                        classname: Some("BuiltIn.Channel".into()),
                        ..Default::default()
                    },
                    SymbolDoc {
                        path: "Root.Inputs.Sensed".into(),
                        kind: SymbolDocKind::Channel,
                        type_label: "f32".into(),
                        classname: Some("BuiltIn.ChannelCalibratable".into()),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Inputs.md").unwrap();
        assert!(
            page.body
                .contains("| Name | Type | Quantity | Unit | Base | Log rate | Security | Class |"),
            "Class column header missing; got:\n{}",
            page.body
        );
        assert!(
            page.body.contains("BuiltIn.ChannelCalibratable |"),
            "non-plain class must be shown; got:\n{}",
            page.body
        );
    }

    // ---- #30: function source links + embedding ----

    fn fn_with_source() -> FunctionDoc {
        FunctionDoc {
            path: "Root.Engine.Update".into(),
            anchor: "root-engine-update".into(),
            call_rate_hz: Some(100.0),
            source_path: Some("Engine/Update.m1scr".into()),
            source_text: Some("Out = In.Speed * 2;\n".into()),
            ..Default::default()
        }
    }

    #[test]
    fn function_source_path_shown_plain_without_base() {
        let out = render_function(&fn_with_source(), &RenderOptions::default());
        assert!(
            out.contains("**Source:** `Engine/Update.m1scr`"),
            "plain source path missing; got:\n{out}"
        );
    }

    #[test]
    fn function_source_path_becomes_link_with_base() {
        let opts = RenderOptions {
            source_base: Some("https://github.com/UQRacing/EV-M1/blob/main/".into()),
            include_source: false,
            graph: None,
        };
        let out = render_function(&fn_with_source(), &opts);
        assert!(
            out.contains(
                "**Source:** [Engine/Update.m1scr](https://github.com/UQRacing/EV-M1/blob/main/Engine/Update.m1scr)"
            ),
            "source link (trailing slash trimmed) wrong; got:\n{out}"
        );
    }

    #[test]
    fn function_source_link_url_encodes_spaces_keeps_text_readable() {
        // M1 object names may contain spaces (Development Manual, Naming
        // Objects). The on-disk source path therefore can too — the URL target
        // must be percent-encoded so the link is valid, while the visible link
        // text stays the human-readable raw path.
        let mut f = fn_with_source();
        f.source_path = Some("Control.Power Limit.Reset Integral Error.m1scr".into());
        let opts = RenderOptions {
            source_base: Some("https://example/blob/main".into()),
            ..Default::default()
        };
        let out = render_function(&f, &opts);
        // URL target: spaces encoded, '/' separators preserved.
        assert!(
            out.contains(
                "(https://example/blob/main/Control.Power%20Limit.Reset%20Integral%20Error.m1scr)"
            ),
            "source link URL must percent-encode spaces; got:\n{out}"
        );
        // Visible link text stays the raw, readable path.
        assert!(
            out.contains("[Control.Power Limit.Reset Integral Error.m1scr]"),
            "link text must stay human-readable; got:\n{out}"
        );
    }

    #[test]
    fn include_source_embeds_collapsible_body() {
        let opts = RenderOptions {
            source_base: None,
            include_source: true,
            graph: None,
        };
        let out = render_function(&fn_with_source(), &opts);
        assert!(
            out.contains("<details><summary>Source</summary>")
                && out.contains("```m1\nOut = In.Speed * 2;\n```")
                && out.contains("</details>"),
            "include_source must embed a collapsible code block; got:\n{out}"
        );
    }

    #[test]
    fn include_source_escalates_fence_past_embedded_backticks() {
        // A script body whose own line is exactly three backticks (e.g. inside a
        // block comment or a string literal) must NOT terminate the code fence
        // early — the outer fence has to be longer than any run inside the body.
        let mut f = fn_with_source();
        f.source_text = Some("Out = 1;\n```\nstill source;\n".into());
        let opts = RenderOptions {
            source_base: None,
            include_source: true,
            graph: None,
        };
        let out = render_function(&f, &opts);
        // The opening/closing fence must be 4+ backticks so the embedded ``` is
        // treated as code, not as a fence terminator.
        assert!(
            out.contains("````m1\n"),
            "fence must escalate past embedded triple-backtick; got:\n{out}"
        );
        // The whole body (including the line after the embedded fence) stays
        // inside the code block, and </details> remains a separate block.
        assert!(
            out.contains("still source;"),
            "body after embedded fence must be preserved; got:\n{out}"
        );
        assert!(
            out.contains("````\n"),
            "closing fence must match the escalated length; got:\n{out}"
        );
        assert!(
            out.contains("</details>"),
            "</details> must still be emitted; got:\n{out}"
        );
    }

    #[test]
    fn source_off_by_default_no_details() {
        let out = render_function(&fn_with_source(), &RenderOptions::default());
        assert!(
            !out.contains("<details>"),
            "source body must not embed unless --include-source; got:\n{out}"
        );
    }

    #[test]
    fn function_without_source_path_omits_source_line() {
        let f = FunctionDoc {
            path: "Root.Engine.NoFile".into(),
            anchor: "root-engine-nofile".into(),
            ..Default::default()
        };
        let out = render_function(&f, &RenderOptions::default());
        assert!(
            !out.contains("**Source:**"),
            "no source path → no Source line; got:\n{out}"
        );
    }

    #[test]
    fn class_column_absent_when_all_plain() {
        // Every channel is a plain BuiltIn.Channel → no Class column (no clutter).
        let model = DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    classname: Some("BuiltIn.Channel".into()),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body
                .contains("| Name | Type | Quantity | Unit | Base | Log rate | Security |\n"),
            "default header must be unchanged; got:\n{}",
            page.body
        );
        assert!(
            !page.body.contains("| Class |"),
            "Class column must be absent when all plain; got:\n{}",
            page.body
        );
    }

    /// #29: a group's `BuiltIn.Reference` aliases render a `## References` table
    /// (resolved targets deep-linked, unresolved shown raw) and the inverse
    /// `## Used by` table on the referenced symbol's page.
    #[test]
    fn references_and_used_by_render_with_links() {
        use crate::model::ReferenceDoc;
        let model = DocModel {
            title: "T".into(),
            target_hardware: None,
            enums: vec![],
            groups: vec![GroupDoc {
                path: "Root.Sensors".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Sensors.OilP".into(),
                    anchor: "root-sensors-oilp".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    ..Default::default()
                }],
                references: vec![
                    ReferenceDoc {
                        path: "Root.Sensors.Alias".into(),
                        anchor: "root-sensors-alias".into(),
                        target_raw: "This.OilP".into(),
                        target_resolved: Some("Root.Sensors.OilP".into()),
                        def_line: None,
                    },
                    ReferenceDoc {
                        path: "Root.Sensors.Dangling".into(),
                        anchor: "root-sensors-dangling".into(),
                        target_raw: "Nowhere.X".into(),
                        target_resolved: None,
                        def_line: None,
                    },
                ],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        };
        let files = render(&model);
        let page = files
            .iter()
            .find(|f| f.path == "Root.Sensors.md")
            .expect("Root.Sensors.md");

        // Forward: the References section deep-links a resolved target.
        assert!(page.body.contains("## References"), "got:\n{}", page.body);
        assert!(
            page.body
                .contains("[`Root.Sensors.OilP`](Root.Sensors.md#root-sensors-oilp)"),
            "resolved target must deep-link the symbol; got:\n{}",
            page.body
        );
        // An unresolved target is shown verbatim, never linked.
        assert!(
            page.body.contains("`Nowhere.X`"),
            "raw target must be shown verbatim; got:\n{}",
            page.body
        );

        // Inverse: the symbol's page lists who references it, deep-linked.
        assert!(page.body.contains("## Used by"), "got:\n{}", page.body);
        assert!(
            page.body
                .contains("[`Root.Sensors.Alias`](Root.Sensors.md#root-sensors-alias)"),
            "used-by must link the referencing alias; got:\n{}",
            page.body
        );
    }

    // ---- #37 relationship graph ----

    /// A model whose `Root.Engine.Update` function reads `Speed` and writes
    /// `Torque` — a group with documented relationships.
    fn graph_model() -> DocModel {
        let mut m = sample(); // Root.Engine with the Speed channel
        m.groups[0].symbols.push(SymbolDoc {
            path: "Root.Engine.Torque".into(),
            kind: SymbolDocKind::Channel,
            type_label: "f32".into(),
            ..Default::default()
        });
        m.groups[0].functions.push(FunctionDoc {
            path: "Root.Engine.Update".into(),
            anchor: "root-engine-update".into(),
            ..Default::default()
        });
        m.graph = ProjectGraph {
            edges: vec![
                GraphEdge {
                    from: "Root.Engine.Update".into(),
                    to: "Root.Engine.Speed".into(),
                    kind: EdgeKind::Read,
                },
                GraphEdge {
                    from: "Root.Engine.Update".into(),
                    to: "Root.Engine.Torque".into(),
                    kind: EdgeKind::Write,
                },
            ],
        };
        m
    }

    #[test]
    fn group_with_relationships_emits_graph_block_with_mermaid_fallback() {
        let files = render(&graph_model());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(page.body.contains("## Relationships"));
        assert!(
            page.body.contains("<!--m1-graph:group:1:Root.Engine-->"),
            "sentinel for the HTML widget missing; got:\n{}",
            page.body
        );
        // The Markdown carries a Mermaid fallback for GitHub viewers.
        assert!(page.body.contains("```mermaid"));
        assert!(page.body.contains("-. reads .->"));
        assert!(page.body.contains("-- writes -->"));
    }

    #[test]
    fn group_without_relationships_has_no_graph_block() {
        // sample() has no graph edges → no Relationships section.
        let files = render(&sample());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(!page.body.contains("## Relationships"));
        assert!(!page.body.contains("m1-graph"));
    }

    #[test]
    fn graph_flag_emits_focused_subsystem_page_linked_from_index() {
        let opts = RenderOptions {
            source_base: None,
            include_source: false,
            graph: Some(GraphSpec {
                group: "Root.Engine".into(),
                depth: 2,
            }),
        };
        let files = render_with(&graph_model(), &opts);
        let page = files
            .iter()
            .find(|f| f.path == "graph.root-engine.md")
            .expect("--graph must emit a subsystem page");
        assert!(page.body.contains("# Subsystem: Root.Engine"));
        assert!(page.body.contains("<!--m1-graph:subtree:2:Root.Engine-->"));
        let index = files.iter().find(|f| f.path == "index.md").unwrap();
        assert!(
            index
                .body
                .contains("[Subsystem: Root.Engine](graph.root-engine.md)"),
            "index must link the subsystem graph; got:\n{}",
            index.body
        );
    }

    // ---- #57: source/definition links for symbols, not just functions ----

    /// A model with a project file path and a channel/parameter/constant, a
    /// table, an object and a reference — each carrying a `def_line` — so the
    /// renderer can build a jump-to-declaration link to the `.m1prj`.
    fn source_link_model() -> DocModel {
        use crate::model::{ObjectDoc, ReferenceDoc, TableDoc};
        DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![],
            m1prj_path: Some("Project.m1prj".into()),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    anchor: "root-engine-speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    def_line: Some(41),
                    ..Default::default()
                }],
                tables: vec![TableDoc {
                    path: "Root.Engine.IgnitionMap".into(),
                    anchor: "root-engine-ignitionmap".into(),
                    axes: vec![],
                    output_unit: None,
                    def_line: Some(50),
                }],
                objects: vec![ObjectDoc {
                    path: "Root.Engine.OilP".into(),
                    anchor: "root-engine-oilp".into(),
                    class: Some("MoTeC Input.Sensor".into()),
                    members: vec![],
                    def_line: Some(60),
                }],
                references: vec![ReferenceDoc {
                    path: "Root.Engine.Alias".into(),
                    anchor: "root-engine-alias".into(),
                    target_raw: "This.Speed".into(),
                    target_resolved: None,
                    def_line: Some(70),
                }],
                ..Default::default()
            }],
            graph: crate::model::ProjectGraph::default(),
        }
    }

    #[test]
    fn symbol_row_links_to_its_declaration_when_source_base_set() {
        let opts = RenderOptions {
            source_base: Some("https://example/blob/main".into()),
            ..Default::default()
        };
        let files = render_with(&source_link_model(), &opts);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        // def_line is 0-based (41) → 1-based GitHub anchor L42.
        assert!(
            page.body
                .contains("[src](https://example/blob/main/Project.m1prj#L42)"),
            "symbol row must deep-link its declaration; got:\n{}",
            page.body
        );
    }

    #[test]
    fn declaration_src_link_url_encodes_spaces_in_m1prj_path() {
        // A project under a directory with a space yields an m1prj path with a
        // space; the `[src]` URL must percent-encode it (the link has no
        // visible text to keep readable — it's a fixed `src` label).
        let mut model = source_link_model();
        model.m1prj_path = Some("Vehicle Configs/Project.m1prj".into());
        let opts = RenderOptions {
            source_base: Some("https://example/blob/main".into()),
            ..Default::default()
        };
        let files = render_with(&model, &opts);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            page.body
                .contains("[src](https://example/blob/main/Vehicle%20Configs/Project.m1prj#L42)"),
            "declaration src link must percent-encode spaces; got:\n{}",
            page.body
        );
    }

    #[test]
    fn table_object_and_reference_link_to_their_declaration() {
        let opts = RenderOptions {
            source_base: Some("https://example/blob/main".into()),
            ..Default::default()
        };
        let files = render_with(&source_link_model(), &opts);
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        // Table (line 50 → L51), object (60 → L61), reference (70 → L71).
        assert!(
            page.body
                .contains("https://example/blob/main/Project.m1prj#L51"),
            "table must deep-link its declaration; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("https://example/blob/main/Project.m1prj#L61"),
            "object must deep-link its declaration; got:\n{}",
            page.body
        );
        assert!(
            page.body
                .contains("https://example/blob/main/Project.m1prj#L71"),
            "reference must deep-link its declaration; got:\n{}",
            page.body
        );
    }

    #[test]
    fn symbol_source_link_is_absent_without_a_source_base() {
        // No `source_base` → no invented link; the common-case row is unchanged.
        let files = render(&source_link_model());
        let page = files.iter().find(|f| f.path == "Root.Engine.md").unwrap();
        assert!(
            !page.body.contains("[src]("),
            "must not emit a source link without a source_base; got:\n{}",
            page.body
        );
        // The plain channel row is intact (no trailing link in the Name cell).
        assert!(
            page.body.contains("`Root.Engine.Speed` | f32 |"),
            "row layout must be unchanged without a source link; got:\n{}",
            page.body
        );
    }
}
