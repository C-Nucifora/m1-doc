//! Serialises a [`DocModel`] to a single machine-readable JSON document — the
//! same data the Markdown and HTML renderers show, but as structured data a
//! programmatic consumer (editor tooling, dashboards, doc-diffing, CI checks)
//! can depend on (#35).
//!
//! The serializer is hand-rolled rather than serde-derived for two reasons: it
//! keeps this feature additive and file-disjoint (no `#[derive(Serialize)]` on
//! `model.rs`, which a sibling PR owns), and it pins down byte-for-byte the key
//! order and number/string formatting — the guardrail the whole tool lives by
//! is *deterministic, byte-identical* output. Object keys are emitted in the
//! fixed order written here; arrays preserve the model's order (already sorted
//! by the loader). Missing data is `null`, never invented (degrade, never fake).

use crate::model::{
    AnnotationDoc, CanMessageDoc, CanSignalDoc, DocModel, EnumDoc, FunctionDoc, GraphEdge,
    GroupDoc, ObjectDoc, ReferenceDoc, SymbolDoc, SymbolDocKind, TableDoc,
};
use std::fmt::Write as _;

/// The JSON schema version. Bump on any breaking change to the shape below so
/// consumers can gate on it. Additive fields do not require a bump.
pub const SCHEMA_VERSION: u32 = 1;

/// Serialise the whole model to a pretty-printed JSON string with a trailing
/// newline. Two calls with an equal `DocModel` return byte-identical strings.
pub fn render(model: &DocModel) -> String {
    let mut w = Writer::new();
    w.object(|w| {
        w.field_u32("schema_version", SCHEMA_VERSION);
        w.field_str("title", &model.title);
        w.field_opt_str("target_hardware", model.target_hardware.as_deref());
        w.field("groups", |w| w.array(&model.groups, write_group));
        w.field("enums", |w| w.array(&model.enums, write_enum));
        // The relationship graph (#37): nodes are the symbol paths above; this
        // carries the typed edges between them.
        w.field("graph", |w| {
            w.object(|w| {
                w.field("edges", |w| w.array(&model.graph.edges, write_edge));
            });
        });
    });
    w.finish()
}

fn write_edge(w: &mut Writer, e: &GraphEdge) {
    w.object(|w| {
        w.field_str("from", &e.from);
        w.field_str("to", &e.to);
        w.field_str("kind", e.kind.as_str());
    });
}

/// The documented kind of a symbol, as a stable lowercase string.
fn kind_str(kind: SymbolDocKind) -> &'static str {
    match kind {
        SymbolDocKind::Channel => "channel",
        SymbolDocKind::Parameter => "parameter",
        SymbolDocKind::Constant => "constant",
    }
}

fn write_group(w: &mut Writer, g: &GroupDoc) {
    w.object(|w| {
        w.field_str("path", &g.path);
        w.field("symbols", |w| w.array(&g.symbols, write_symbol));
        w.field("functions", |w| w.array(&g.functions, write_function));
        w.field("tables", |w| w.array(&g.tables, write_table));
        w.field("objects", |w| w.array(&g.objects, write_object));
        w.field("can_messages", |w| {
            w.array(&g.can_messages, write_can_message)
        });
        w.field("references", |w| w.array(&g.references, write_reference));
        w.field("children", |w| w.string_array(&g.children));
    });
}

fn write_reference(w: &mut Writer, r: &ReferenceDoc) {
    w.object(|w| {
        w.field_str("path", &r.path);
        w.field_str("anchor", &r.anchor);
        w.field_str("target_raw", &r.target_raw);
        w.field_opt_str("target_resolved", r.target_resolved.as_deref());
    });
}

fn write_symbol(w: &mut Writer, s: &SymbolDoc) {
    w.object(|w| {
        w.field_str("path", &s.path);
        w.field_str("anchor", &s.anchor);
        w.field_str("kind", kind_str(s.kind));
        w.field_str("type_label", &s.type_label);
        w.field_opt_str("quantity", s.quantity.as_deref());
        w.field_opt_str("unit", s.unit.as_deref());
        w.field_opt_str("base_unit", s.base_unit.as_deref());
        w.field_opt_f64("log_rate_hz", s.log_rate_hz);
        w.field_opt_str("security", s.security.as_deref());
        w.field_opt_str("enum_ref", s.enum_ref.as_deref());
        w.field_opt_str("classname", s.classname.as_deref());
        w.field("tags", |w| w.string_array(&s.tags));
    });
}

fn write_function(w: &mut Writer, f: &FunctionDoc) {
    w.object(|w| {
        w.field_str("path", &f.path);
        w.field_str("anchor", &f.anchor);
        // Inputs as `[{"name","type"}]` so the pairing survives JSON (an array
        // of 2-tuples would lose the field names a consumer wants).
        w.field("inputs", |w| {
            w.array(&f.inputs, |w, (name, ty)| {
                w.object(|w| {
                    w.field_str("name", name);
                    w.field_str("type", ty);
                });
            })
        });
        w.field_opt_str("return_type", f.return_type.as_deref());
        w.field("annotations", |w| w.array(&f.annotations, write_annotation));
        w.field_opt_f64("call_rate_hz", f.call_rate_hz);
        w.field_opt_str("source_path", f.source_path.as_deref());
    });
}

fn write_annotation(w: &mut Writer, a: &AnnotationDoc) {
    w.object(|w| {
        w.field_str("kind", &a.kind);
        w.field("args", |w| w.string_array(&a.args));
    });
}

fn write_table(w: &mut Writer, t: &TableDoc) {
    w.object(|w| {
        w.field_str("path", &t.path);
        w.field_str("anchor", &t.anchor);
        w.field("axes", |w| {
            w.array(&t.axes, |w, axis| {
                w.object(|w| {
                    w.field_u32("size", axis.size);
                    w.field_opt_str("unit", axis.unit.as_deref());
                });
            })
        });
        w.field_opt_str("output_unit", t.output_unit.as_deref());
    });
}

fn write_object(w: &mut Writer, o: &ObjectDoc) {
    w.object(|w| {
        w.field_str("path", &o.path);
        w.field_str("anchor", &o.anchor);
        w.field_opt_str("class", o.class.as_deref());
        w.field("members", |w| w.string_array(&o.members));
    });
}

fn write_can_message(w: &mut Writer, m: &CanMessageDoc) {
    w.object(|w| {
        w.field_str("path", &m.path);
        w.field_str("anchor", &m.anchor);
        w.field_opt_u32("id", m.can_id);
        w.field_opt_u32("dlc", m.dlc);
        w.field("signals", |w| w.array(&m.signals, write_can_signal));
    });
}

fn write_can_signal(w: &mut Writer, s: &CanSignalDoc) {
    w.object(|w| {
        w.field_str("path", &s.path);
        w.field_str("anchor", &s.anchor);
        w.field_opt_u32("start_bit", s.start_bit);
        w.field_opt_u32("length", s.length);
        w.field_opt_f64("multiplier", s.multiplier);
        w.field_opt_f64("offset", s.offset);
        // Range is a `[min, max]` pair or null — degrade, never fake a default.
        match s.range {
            Some((lo, hi)) => w.field("range", |w| {
                w.raw_array_of_2(fmt_f64(lo), fmt_f64(hi));
            }),
            None => w.field_null("range"),
        }
        w.field_opt_str("unit", s.unit.as_deref());
    });
}

fn write_enum(w: &mut Writer, e: &EnumDoc) {
    w.object(|w| {
        w.field_str("name", &e.name);
        w.field_str("anchor", &e.anchor);
        w.field("members", |w| w.string_array(&e.members));
        w.field_opt_str("default", e.default.as_deref());
        w.field_bool("open", e.open);
    });
}

/// Format an `f64` deterministically. Non-finite values (`NaN`, `±inf`) have no
/// JSON number form, so they serialise as `null` — degrade, never emit invalid
/// JSON. `{:?}` gives a round-trippable shortest representation (`0.5`, `2.0`).
fn fmt_f64(v: f64) -> String {
    if v.is_finite() {
        format!("{v:?}")
    } else {
        "null".to_string()
    }
}

/// A minimal pretty-printer for the fixed JSON shape above. Tracks indentation
/// and emits a trailing comma only between siblings, so the output parses and is
/// stable. Not a general JSON library — just enough for this document.
struct Writer {
    buf: String,
    indent: usize,
    /// Whether the current container already has a child (controls the comma).
    has_child: Vec<bool>,
}

impl Writer {
    fn new() -> Self {
        Writer {
            buf: String::new(),
            indent: 0,
            has_child: Vec::new(),
        }
    }

    fn finish(mut self) -> String {
        self.buf.push('\n');
        self.buf
    }

    /// Write the indentation for the current depth.
    fn pad(&mut self) {
        for _ in 0..self.indent {
            self.buf.push_str("  ");
        }
    }

    /// Emit the separating comma + newline before a sibling, or just a newline
    /// before the first child of a container.
    fn sep(&mut self) {
        let first = self.has_child.last().copied().unwrap_or(true);
        if first {
            self.buf.push('\n');
            if let Some(last) = self.has_child.last_mut() {
                *last = false;
            }
        } else {
            self.buf.push_str(",\n");
        }
    }

    /// Open an object, run `body` to emit its fields, then close it. An empty
    /// object renders as `{}`.
    fn object(&mut self, body: impl FnOnce(&mut Self)) {
        self.buf.push('{');
        self.has_child.push(true);
        self.indent += 1;
        body(self);
        let had = !self.has_child.pop().unwrap_or(true);
        self.indent -= 1;
        if had {
            self.buf.push('\n');
            self.pad();
        }
        self.buf.push('}');
    }

    /// A named field whose value is produced by `value`.
    fn field(&mut self, key: &str, value: impl FnOnce(&mut Self)) {
        self.sep();
        self.pad();
        write_json_string(&mut self.buf, key);
        self.buf.push_str(": ");
        value(self);
    }

    fn field_str(&mut self, key: &str, value: &str) {
        self.field(key, |w| write_json_string(&mut w.buf, value));
    }

    fn field_opt_str(&mut self, key: &str, value: Option<&str>) {
        match value {
            Some(v) => self.field_str(key, v),
            None => self.field_null(key),
        }
    }

    fn field_u32(&mut self, key: &str, value: u32) {
        self.field(key, |w| {
            let _ = write!(w.buf, "{value}");
        });
    }

    fn field_opt_u32(&mut self, key: &str, value: Option<u32>) {
        match value {
            Some(v) => self.field_u32(key, v),
            None => self.field_null(key),
        }
    }

    fn field_opt_f64(&mut self, key: &str, value: Option<f64>) {
        match value {
            Some(v) => self.field(key, |w| w.buf.push_str(&fmt_f64(v))),
            None => self.field_null(key),
        }
    }

    fn field_bool(&mut self, key: &str, value: bool) {
        self.field(key, |w| {
            w.buf.push_str(if value { "true" } else { "false" })
        });
    }

    fn field_null(&mut self, key: &str) {
        self.field(key, |w| w.buf.push_str("null"));
    }

    /// Render `items` as an array, calling `each` to emit every element. An
    /// empty array renders as `[]`.
    fn array<T>(&mut self, items: &[T], each: impl Fn(&mut Self, &T)) {
        self.buf.push('[');
        self.has_child.push(true);
        self.indent += 1;
        for item in items {
            self.sep();
            self.pad();
            each(self, item);
        }
        let had = !self.has_child.pop().unwrap_or(true);
        self.indent -= 1;
        if had {
            self.buf.push('\n');
            self.pad();
        }
        self.buf.push(']');
    }

    /// Render a slice of strings as a JSON string array.
    fn string_array(&mut self, items: &[String]) {
        self.array(items, |w, s| write_json_string(&mut w.buf, s));
    }

    /// Render a 2-element array of already-formatted numeric tokens (used for a
    /// `[min, max]` range) on a single line.
    fn raw_array_of_2(&mut self, a: String, b: String) {
        let _ = write!(self.buf, "[{a}, {b}]");
    }
}

/// Append `s` to `out` as a correctly-escaped JSON string (with surrounding
/// quotes). Escapes the JSON control set plus `"`/`\\`, and emits `\uXXXX` for
/// the remaining C0 control characters so the output is always valid JSON.
fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{
        AnnotationDoc, CanMessageDoc, CanSignalDoc, DocModel, EnumDoc, FunctionDoc, GroupDoc,
        ObjectDoc, SymbolDoc, SymbolDocKind, TableAxisDoc, TableDoc,
    };

    /// A model exercising every entity kind and every optional field (both the
    /// present and the absent arm).
    fn rich_model() -> DocModel {
        DocModel {
            title: "Demo \"Project\"".into(),
            target_hardware: Some("ecu120".into()),
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    anchor: "root-engine-speed".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    quantity: Some("rad/s".into()),
                    unit: Some("rpm".into()),
                    base_unit: Some("rad/s".into()),
                    log_rate_hz: Some(200.0),
                    security: Some("Engineer".into()),
                    enum_ref: None,
                    classname: Some("BuiltIn.Channel".into()),
                    tags: vec!["engine".into(), "fuel".into()],
                }],
                functions: vec![FunctionDoc {
                    path: "Root.Engine.Update".into(),
                    anchor: "root-engine-update".into(),
                    inputs: vec![("Timeout".into(), "float".into())],
                    return_type: Some("float".into()),
                    annotations: vec![AnnotationDoc {
                        kind: "requires-finite".into(),
                        args: vec!["min=0".into()],
                    }],
                    call_rate_hz: Some(100.0),
                    source_path: Some("Engine/Update.m1scr".into()),
                    source_text: Some("Out = In;\n".into()),
                }],
                tables: vec![TableDoc {
                    path: "Root.Engine.IgnitionMap".into(),
                    anchor: "root-engine-ignitionmap".into(),
                    axes: vec![TableAxisDoc {
                        size: 16,
                        unit: Some("rpm".into()),
                    }],
                    output_unit: Some("deg".into()),
                }],
                objects: vec![ObjectDoc {
                    path: "Root.Engine.OilP".into(),
                    anchor: "root-engine-oilp".into(),
                    class: Some("MoTeC Input.Sensor".into()),
                    members: vec!["Root.Engine.OilP.Resource".into()],
                }],
                can_messages: vec![CanMessageDoc {
                    path: "Root.Engine.Frame".into(),
                    anchor: "root-engine-frame".into(),
                    can_id: Some(160),
                    dlc: Some(8),
                    signals: vec![CanSignalDoc {
                        path: "Root.Engine.Frame.Rpm".into(),
                        anchor: "root-engine-frame-rpm".into(),
                        start_bit: Some(24),
                        length: Some(16),
                        multiplier: Some(0.5),
                        offset: Some(0.0),
                        range: Some((0.0, 8000.0)),
                        unit: Some("rpm".into()),
                    }],
                }],
                references: vec![],
                children: vec!["Root.Engine.Fuel".into()],
            }],
            enums: vec![EnumDoc {
                name: "Switch".into(),
                anchor: "switch".into(),
                members: vec!["Off".into(), "On".into()],
                default: Some("Off".into()),
                open: false,
            }],
            graph: crate::model::ProjectGraph::default(),
        }
    }

    #[test]
    fn output_is_deterministic() {
        let model = rich_model();
        assert_eq!(
            render(&model),
            render(&model),
            "two renders of the same model must be byte-identical"
        );
    }

    #[test]
    fn has_versioned_top_level_schema() {
        let json = render(&rich_model());
        assert!(
            json.contains("\"schema_version\": 1"),
            "missing schema_version; got:\n{json}"
        );
    }

    #[test]
    fn covers_every_entity_and_field() {
        let json = render(&rich_model());
        for needle in [
            "\"title\": \"Demo \\\"Project\\\"\"",
            "\"path\": \"Root.Engine.Speed\"",
            "\"kind\": \"channel\"",
            "\"type_label\": \"f32\"",
            "\"quantity\": \"rad/s\"",
            "\"base_unit\": \"rad/s\"",
            "\"log_rate_hz\": 200.0",
            "\"security\": \"Engineer\"",
            "\"classname\": \"BuiltIn.Channel\"",
            "\"return_type\": \"float\"",
            "\"call_rate_hz\": 100.0",
            "\"source_path\": \"Engine/Update.m1scr\"",
            "\"kind\": \"requires-finite\"",
            "\"output_unit\": \"deg\"",
            "\"class\": \"MoTeC Input.Sensor\"",
            "\"id\": 160",
            "\"dlc\": 8",
            "\"range\": [0.0, 8000.0]",
            "\"name\": \"Switch\"",
            "\"open\": false",
        ] {
            assert!(json.contains(needle), "missing {needle:?}; got:\n{json}");
        }
    }

    #[test]
    fn absent_optionals_serialise_as_null() {
        // A symbol with every optional unset → each renders as JSON null, never
        // an invented value (degrade, never fake).
        let model = DocModel {
            title: "T".into(),
            target_hardware: None,
            groups: vec![GroupDoc {
                path: "Root".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.X".into(),
                    anchor: "root-x".into(),
                    kind: SymbolDocKind::Constant,
                    type_label: "u8".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            enums: vec![],
            graph: crate::model::ProjectGraph::default(),
        };
        let json = render(&model);
        assert!(json.contains("\"quantity\": null"), "got:\n{json}");
        assert!(json.contains("\"unit\": null"), "got:\n{json}");
        assert!(json.contains("\"log_rate_hz\": null"), "got:\n{json}");
        assert!(json.contains("\"enum_ref\": null"), "got:\n{json}");
        assert!(json.contains("\"kind\": \"constant\""), "got:\n{json}");
    }

    #[test]
    fn empty_model_still_has_schema_and_containers() {
        let json = render(&DocModel::default());
        assert!(json.contains("\"schema_version\": 1"), "got:\n{json}");
        assert!(json.contains("\"groups\": []"), "got:\n{json}");
        assert!(json.contains("\"enums\": []"), "got:\n{json}");
        assert!(
            json.ends_with("}\n"),
            "must end with a newline; got:\n{json}"
        );
    }

    #[test]
    fn strings_with_control_chars_are_escaped() {
        let model = DocModel {
            title: "tab\there\nnew\\back".into(),
            ..Default::default()
        };
        let json = render(&model);
        assert!(
            json.contains("\"title\": \"tab\\there\\nnew\\\\back\""),
            "control chars must be JSON-escaped; got:\n{json}"
        );
    }

    #[test]
    fn non_finite_floats_degrade_to_null() {
        // A NaN log rate has no JSON number form → null, keeping output valid.
        let model = DocModel {
            title: "T".into(),
            target_hardware: None,
            groups: vec![GroupDoc {
                path: "Root".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.X".into(),
                    anchor: "root-x".into(),
                    kind: SymbolDocKind::Channel,
                    type_label: "f32".into(),
                    log_rate_hz: Some(f64::NAN),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            enums: vec![],
            graph: crate::model::ProjectGraph::default(),
        };
        let json = render(&model);
        assert!(json.contains("\"log_rate_hz\": null"), "got:\n{json}");
    }

    #[test]
    fn references_serialize_with_raw_and_resolved_targets() {
        use crate::model::ReferenceDoc;
        let model = DocModel {
            title: "T".into(),
            groups: vec![GroupDoc {
                path: "Root".into(),
                references: vec![
                    ReferenceDoc {
                        path: "Root.Alias".into(),
                        anchor: "root-alias".into(),
                        target_raw: "This.Value".into(),
                        target_resolved: Some("Root.Value".into()),
                    },
                    ReferenceDoc {
                        path: "Root.Dangling".into(),
                        anchor: "root-dangling".into(),
                        target_raw: "Off.Model".into(),
                        target_resolved: None,
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };
        let json = render(&model);
        assert!(json.contains("\"references\""), "got:\n{json}");
        assert!(
            json.contains("\"target_raw\": \"This.Value\""),
            "got:\n{json}"
        );
        assert!(
            json.contains("\"target_resolved\": \"Root.Value\""),
            "resolved target must serialize; got:\n{json}"
        );
        // An unresolved target serialises as null — degrade, never invent.
        assert!(
            json.contains("\"target_raw\": \"Off.Model\"")
                && json.contains("\"target_resolved\": null"),
            "unresolved target must be raw + null; got:\n{json}"
        );
    }

    #[test]
    fn graph_edges_serialize_with_typed_kinds() {
        use crate::model::{EdgeKind, GraphEdge, ProjectGraph};
        let model = DocModel {
            title: "T".into(),
            graph: ProjectGraph {
                edges: vec![
                    GraphEdge {
                        from: "Root.A.Update".into(),
                        to: "Root.A.Helper".into(),
                        kind: EdgeKind::Call,
                    },
                    GraphEdge {
                        from: "Root.A.Update".into(),
                        to: "Root.A.Speed".into(),
                        kind: EdgeKind::Read,
                    },
                ],
            },
            ..Default::default()
        };
        let json = render(&model);
        assert!(json.contains("\"graph\""), "got:\n{json}");
        assert!(json.contains("\"edges\""), "got:\n{json}");
        assert!(
            json.contains("\"from\": \"Root.A.Update\"")
                && json.contains("\"to\": \"Root.A.Helper\"")
                && json.contains("\"kind\": \"call\""),
            "call edge must serialize; got:\n{json}"
        );
        assert!(json.contains("\"kind\": \"read\""), "got:\n{json}");
        // An empty model still emits an empty edges array (never omitted).
        let empty = render(&DocModel::default());
        assert!(
            empty.contains("\"graph\"") && empty.contains("\"edges\": []"),
            "empty graph must still emit edges:[]; got:\n{empty}"
        );
    }
}
