//! Builds a [`DocModel`] from a loaded m1-typecheck project. Channels,
//! parameters, and constants are grouped by their top-level group
//! (`Root.Engine` for `Root.Engine.Speed`).

use crate::model::{AnnotationDoc, DocModel, FunctionDoc, GroupDoc, SymbolDoc, SymbolDocKind};
use m1_typecheck::symbols::{Symbol, SymbolKind};
use m1_typecheck::Project;
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

/// `true` when the symbol kind should be collected as a function.
fn is_function(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::Function | SymbolKind::Method)
}

/// Build a [`FunctionDoc`] from a function/method symbol.
fn function_doc(sym: &Symbol) -> FunctionDoc {
    let inputs = sym
        .in_params
        .as_deref()
        .unwrap_or(&[])
        .iter()
        .map(|(name, vt)| (name.clone(), value_type_label(*vt).to_string()))
        .collect();
    FunctionDoc {
        path: sym.path.clone(),
        inputs,
        annotations: Vec::new(),
    }
}

/// Render a `ValueType` as a human-readable label. `ValueType` has no `Display`
/// impl; this mirrors the string representations used elsewhere in the toolchain.
/// `Enum(_)` collapses to `"enum"` because the specific enum name comes from
/// `declared_type` when present — the `ValueType` variant only carries the
/// resolved kind, not the name the user typed.
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
/// resolved value type's display string. Always returns a non-empty string —
/// every symbol has at least a resolved `ValueType`.
fn type_label(sym: &Symbol) -> String {
    sym.declared_type
        .clone()
        .unwrap_or_else(|| value_type_label(sym.value_type).to_string())
}

fn symbol_doc(sym: &Symbol, kind: SymbolDocKind) -> SymbolDoc {
    // `display_unit` is the human-visible unit (e.g. `rpm`, `kPa`) from
    // `<Locale><Default Unit="…">`. `unit` is the stored base unit derived from
    // `Qty` (e.g. `rad/s`). We prefer `display_unit` for documentation because
    // it is what MoTeC Build and the dash display to the user.
    let unit = sym.display_unit.clone().or_else(|| sym.unit.clone());
    SymbolDoc {
        path: sym.path.clone(),
        kind,
        type_label: type_label(sym),
        unit,
        security: sym.security.clone(),
    }
}

/// Convert an `m1_core::AnnotationArg` to its string representation.
fn arg_to_string(arg: &m1_core::AnnotationArg) -> String {
    match arg {
        m1_core::AnnotationArg::Positional(v) => v.clone(),
        m1_core::AnnotationArg::Named { key, value } => format!("{key}={value}"),
    }
}

/// Parse `@m1:` annotations from script source text.
fn parse_annotations(source: &str) -> Vec<AnnotationDoc> {
    let cst = m1_core::parse(source);
    let registry = m1_core::Registry::seed();
    let annotations = m1_core::annotations(&cst, &registry);
    annotations
        .all()
        .iter()
        .map(|ann| AnnotationDoc {
            kind: ann.kind.clone(),
            args: ann.args.iter().map(arg_to_string).collect(),
        })
        .collect()
}

/// Resolve the path to a script file given the project directory and the
/// function's filename (a basename like `"Foo.m1scr"`).
///
/// Strategy: try `project_dir/filename` first. If that is missing, fall back to
/// a recursive walk of `project_dir` looking for the first file whose basename
/// matches.
fn resolve_script(project_dir: &std::path::Path, filename: &str) -> Option<std::path::PathBuf> {
    let direct = project_dir.join(filename);
    if direct.is_file() {
        return Some(direct);
    }
    // Recursive fallback: walk the project directory tree.
    find_file_in_dir(project_dir, filename)
}

/// Recursively search `dir` for the first file whose base name matches `name`.
fn find_file_in_dir(dir: &std::path::Path, name: &str) -> Option<std::path::PathBuf> {
    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file_in_dir(&path, name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|n| n.to_str()) == Some(name) {
            return Some(path);
        }
    }
    None
}

/// Load a project file and build its documentation model. Keeps all
/// m1-typecheck I/O inside the loader so the rest of the crate stays
/// toolchain-agnostic.
pub fn load(
    project_path: &std::path::Path,
    title: String,
) -> Result<DocModel, m1_typecheck::project::LoadError> {
    let project = m1_typecheck::Project::load(project_path)?;
    let mut model = build_model(&project, title);

    // Build a map of function path -> script filename from the project symbols.
    // Only function/method symbols carry a filename.
    let path_to_filename: BTreeMap<String, String> = project
        .symbols()
        .iter()
        .filter(|sym| is_function(sym.kind))
        .filter_map(|sym| sym.filename.as_ref().map(|f| (sym.path.clone(), f.clone())))
        .collect();

    if path_to_filename.is_empty() {
        return Ok(model);
    }

    let project_dir = project_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));

    for group in &mut model.groups {
        for func in &mut group.functions {
            let Some(filename) = path_to_filename.get(&func.path) else {
                continue;
            };
            let Some(script_path) = resolve_script(project_dir, filename) else {
                continue;
            };
            let bytes = std::fs::read(&script_path).unwrap_or_default();
            let source = String::from_utf8_lossy(&bytes);
            func.annotations = parse_annotations(&source);
        }
    }

    Ok(model)
}

/// Build the documentation model from a project, with `title` for the index.
pub fn build_model(project: &Project, title: String) -> DocModel {
    let mut groups: BTreeMap<String, GroupDoc> = BTreeMap::new();
    for sym in project.symbols().iter() {
        let group_path = top_level_group(&sym.path);
        let group = groups
            .entry(group_path.clone())
            .or_insert_with(|| GroupDoc {
                path: group_path,
                symbols: Vec::new(),
                functions: Vec::new(),
            });
        if let Some(kind) = doc_kind(sym.kind) {
            group.symbols.push(symbol_doc(sym, kind));
        } else if is_function(sym.kind) {
            group.functions.push(function_doc(sym));
        }
    }
    // Deterministic order: groups by path (BTreeMap), symbols and functions by
    // path within.
    let mut groups: Vec<GroupDoc> = groups.into_values().collect();
    for g in &mut groups {
        g.symbols.sort_by(|a, b| a.path.cmp(&b.path));
        g.functions.sort_by(|a, b| a.path.cmp(&b.path));
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
    fn top_level_group_single_segment() {
        assert_eq!(top_level_group("Root"), "Root");
    }

    #[test]
    fn top_level_group_two_segments() {
        assert_eq!(top_level_group("Root.Engine"), "Root.Engine");
    }

    #[test]
    fn top_level_group_deep_path() {
        assert_eq!(top_level_group("Root.Engine.Gain.Value"), "Root.Engine");
    }

    // A FuncUser with a <Signature><Params> produces in_params on the symbol;
    // the loader must collect it under the group's `functions` list.
    const FUNC_PROJECT: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.FuncUser" Name="Root.Engine.Update">
    <Signature ReturnType="bool">
     <Params>
      <Param Name="Timeout" Type="f32"/>
      <Param Name="Enable" Type="bool"/>
     </Params>
    </Signature>
   </Component>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;

    #[test]
    fn function_with_inputs_collected_under_group() {
        let project = Project::from_xml(FUNC_PROJECT).unwrap();
        let model = build_model(&project, "Demo".into());
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group");
        assert_eq!(
            eng.functions.len(),
            1,
            "expected one function; got {:?}",
            eng.functions
        );
        let f = &eng.functions[0];
        assert_eq!(f.path, "Root.Engine.Update");
        assert_eq!(
            f.inputs,
            vec![
                ("Timeout".to_string(), "float".to_string()),
                ("Enable".to_string(), "bool".to_string()),
            ],
            "unexpected inputs: {:?}",
            f.inputs
        );
    }

    #[test]
    fn function_without_signature_has_empty_inputs() {
        // A FuncUser with no <Signature> produces in_params = None; the
        // FunctionDoc should have an empty inputs list.
        const NO_SIG: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.FuncUser" Filename="Engine.Update.m1scr" Name="Root.Engine.Update"/>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;
        let project = Project::from_xml(NO_SIG).unwrap();
        let model = build_model(&project, "Demo".into());
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group");
        assert_eq!(
            eng.functions.len(),
            1,
            "expected one function; got {:?}",
            eng.functions
        );
        assert!(
            eng.functions[0].inputs.is_empty(),
            "no-signature function must have empty inputs"
        );
    }

    /// Integration test: writes a real tempdir with a Project.m1prj that
    /// declares a FuncUser with Filename= and a .m1scr containing a @m1:
    /// annotation, then calls loader::load and asserts the annotation is surfaced.
    #[test]
    fn annotations_surfaced_from_script_file() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let prj_path = dir.path().join("Project.m1prj");
        let script_path = dir.path().join("Foo.m1scr");

        fs::write(
            &prj_path,
            r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.FuncUser" Filename="Foo.m1scr" Name="Root.Engine.Update"/>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#,
        )
        .unwrap();

        fs::write(&script_path, "// @m1:requires-finite\nx = a / b;\n").unwrap();

        let model = load(&prj_path, "T".into()).unwrap();
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group");
        assert_eq!(
            eng.functions.len(),
            1,
            "expected one function; got {:?}",
            eng.functions
        );
        let f = &eng.functions[0];
        assert_eq!(
            f.annotations.len(),
            1,
            "expected one annotation; got {:?}",
            f.annotations
        );
        assert_eq!(
            f.annotations[0].kind, "requires-finite",
            "unexpected kind: {:?}",
            f.annotations[0]
        );
        assert!(
            f.annotations[0].args.is_empty(),
            "requires-finite has no args; got {:?}",
            f.annotations[0].args
        );
    }

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
            eng.symbols.iter().any(
                |s| s.kind == SymbolDocKind::Parameter && s.security.as_deref() == Some("Tune")
            ),
            "parameter with security; got {:?}",
            eng.symbols
        );
    }
}
