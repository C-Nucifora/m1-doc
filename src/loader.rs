//! Builds a [`DocModel`] from a loaded m1-typecheck project. Channels,
//! parameters, and constants are grouped by their top-level group
//! (`Root.Engine` for `Root.Engine.Speed`).

use crate::model::{DocModel, GroupDoc, SymbolDoc, SymbolDocKind};
use m1_typecheck::Project;
use m1_typecheck::symbols::{Symbol, SymbolKind};
use std::collections::BTreeMap;

/// The top-level group a symbol's docs belong on: the first two dot-segments
/// (`Root.Engine`), or the whole path when it has fewer than two.
fn top_level_group(path: &str) -> String {
    let mut it = path.split('.');
    match (it.next(), it.next()) {
        (Some(a), Some(b)) => format!("{a}.{b}"),
        _ => path.to_string(),
    }
}

/// Map an m1-typecheck symbol kind to the documented kinds. Returns `None` for
/// kinds P1 does not document (functions, tables, groups, objects, …).
fn doc_kind(kind: SymbolKind) -> Option<SymbolDocKind> {
    match kind {
        SymbolKind::Channel => Some(SymbolDocKind::Channel),
        SymbolKind::Parameter => Some(SymbolDocKind::Parameter),
        SymbolKind::Constant => Some(SymbolDocKind::Constant),
        _ => None,
    }
}

/// Render a `ValueType` as a human-readable label. `ValueType` has no `Display`
/// impl; this mirrors the string representations used elsewhere in the toolchain.
fn value_type_label(vt: m1_typecheck::ValueType) -> &'static str {
    use m1_typecheck::ValueType;
    match vt {
        ValueType::Boolean => "bool",
        ValueType::Integer => "integer",
        ValueType::Unsigned => "unsigned",
        ValueType::Float => "float",
        ValueType::Enum(_) => "enum",
        ValueType::String => "string",
        ValueType::Unknown => "unknown",
    }
}

/// The storage type label: the declared type verbatim when present, else the
/// resolved value type's display string.
fn type_label(sym: &Symbol) -> Option<String> {
    sym.declared_type
        .clone()
        .or_else(|| Some(value_type_label(sym.value_type).to_string()))
}

fn symbol_doc(sym: &Symbol, kind: SymbolDocKind) -> SymbolDoc {
    // `display_unit` is the human-visible unit (e.g. `rpm`, `kPa`) from
    // `<Locale><Default Unit="…">`. `unit` is the stored base unit derived from
    // `Qty` (e.g. `rad/s`). We prefer `display_unit` for documentation because
    // it is what MoTeC Build and the dash display to the user.
    let unit = sym
        .display_unit
        .clone()
        .or_else(|| sym.unit.clone());
    SymbolDoc {
        path: sym.path.clone(),
        kind,
        type_label: type_label(sym),
        unit,
        security: sym.security.clone(),
    }
}

/// Build the documentation model from a project, with `title` for the index.
pub fn build_model(project: &Project, title: String) -> DocModel {
    let mut groups: BTreeMap<String, GroupDoc> = BTreeMap::new();
    for sym in project.symbols().iter() {
        let Some(kind) = doc_kind(sym.kind) else {
            continue;
        };
        let group_path = top_level_group(&sym.path);
        let group = groups.entry(group_path.clone()).or_insert_with(|| GroupDoc {
            path: group_path,
            symbols: Vec::new(),
        });
        group.symbols.push(symbol_doc(sym, kind));
    }
    // Deterministic order: groups by path (BTreeMap), symbols by path within.
    let mut groups: Vec<GroupDoc> = groups.into_values().collect();
    for g in &mut groups {
        g.symbols.sort_by(|a, b| a.path.cmp(&b.path));
    }
    DocModel { title, groups }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROJECT: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.Channel" Name="Root.Engine.Speed"><Props Type="f32"><Locale><Default Unit="rpm"/></Locale></Props></Component>
   <Component Classname="BuiltIn.Parameter" Name="Root.Engine.Gain.Value"><Props Type="u16" Security="Tune"/></Component>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;

    #[test]
    fn groups_channels_and_params_under_their_top_level_group() {
        let project = Project::from_xml(PROJECT).unwrap();
        let model = build_model(&project, "Demo".into());
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group");
        assert!(
            eng.symbols.iter().any(|s| s.path == "Root.Engine.Speed"
                && s.kind == SymbolDocKind::Channel
                && s.unit.as_deref() == Some("rpm")),
            "channel with unit; got {:?}",
            eng.symbols
        );
        assert!(
            eng.symbols.iter().any(|s| s.kind == SymbolDocKind::Parameter
                && s.security.as_deref() == Some("Tune")),
            "parameter with security; got {:?}",
            eng.symbols
        );
    }
}
