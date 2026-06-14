//! Builds a [`DocModel`] from a loaded m1-typecheck project. Channels,
//! parameters, constants, and functions are placed on their *immediate* parent
//! group at whatever depth (`Root.Engine.Fuel.Pump` for
//! `Root.Engine.Fuel.Pump.Demand`), and every ancestor group becomes its own
//! node so the full tree is navigable.

use crate::model::{
    AnnotationDoc, DocModel, EnumDoc, FunctionDoc, GroupDoc, SymbolDoc, SymbolDocKind,
    TableAxisDoc, TableDoc, anchor_slug,
};
use m1_typecheck::Project;
use m1_typecheck::symbols::{Symbol, SymbolKind, SymbolTable};
use std::collections::BTreeMap;

/// The parent group of a path: everything up to (not including) the last
/// dot-segment. `Root.Engine.Speed` → `Root.Engine`; `Root.Engine` → `Root`;
/// `Root` → `""` (a forest root, no parent group).
fn parent_group(path: &str) -> &str {
    match path.rfind('.') {
        Some(i) => &path[..i],
        None => "",
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
    let return_type = sym.return_type.map(|vt| value_type_label(vt).to_string());
    FunctionDoc {
        path: sym.path.clone(),
        // Base slug; `assign_anchors` resolves any page collisions.
        anchor: anchor_slug(&sym.path),
        inputs,
        return_type,
        annotations: Vec::new(),
        call_rate_hz: sym.call_rate_hz,
    }
}

/// Build a [`TableDoc`] from a `BuiltIn.Table` symbol. Axis sizes/units and the
/// output unit come from `table_meta` (populated when a `.m1cfg` is loaded);
/// with no cfg the axes are empty and the table is still documented by name.
fn table_doc(sym: &Symbol) -> TableDoc {
    let (axes, output_unit) = match &sym.table_meta {
        Some(meta) => (
            meta.axes
                .iter()
                .map(|a| TableAxisDoc {
                    size: a.size,
                    unit: a.unit.clone(),
                })
                .collect(),
            meta.output_unit.clone(),
        ),
        None => (Vec::new(), None),
    };
    TableDoc {
        path: sym.path.clone(),
        anchor: anchor_slug(&sym.path),
        axes,
        output_unit,
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

fn symbol_doc(sym: &Symbol, kind: SymbolDocKind, table: &SymbolTable) -> SymbolDoc {
    // `display_unit` is the human-visible unit (e.g. `rpm`, `kPa`) from
    // `<Locale><Default Unit="…">`. `unit` is the stored base unit derived from
    // `Qty` (e.g. `rad/s`). We prefer `display_unit` for documentation because
    // it is what MoTeC Build and the dash display to the user.
    let unit = sym.display_unit.clone().or_else(|| sym.unit.clone());
    // Resolve an enum-typed symbol to its enum's name (#27), so the type shows
    // `MoTeC Types.Switch` and links to the Enums reference instead of `enum`.
    let enum_name = sym.enum_assoc.map(|id| table.enum_type(id).name.clone());
    let type_label = sym
        .declared_type
        .clone()
        .or_else(|| enum_name.clone())
        .unwrap_or_else(|| value_type_label(sym.value_type).to_string());
    SymbolDoc {
        path: sym.path.clone(),
        // Base slug; `assign_anchors` resolves any page collisions.
        anchor: anchor_slug(&sym.path),
        kind,
        type_label,
        quantity: sym.qty.clone(),
        unit,
        base_unit: sym.unit.clone(),
        log_rate_hz: sym.log_rate_hz,
        security: sym.security.clone(),
        enum_ref: enum_name,
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

/// Collect every `.m1scr` file under `dir` (recursively) as `(basename, source)`
/// pairs suitable for passing to `m1_typecheck::parsed::parse_all`. The source is
/// read with lossy UTF-8 decoding to handle Windows-1252 encoded files without
/// aborting the entire collection.
fn collect_scripts(dir: &std::path::Path) -> Vec<(String, String)> {
    let mut out = Vec::new();
    collect_scripts_rec(dir, &mut out);
    out
}

fn collect_scripts_rec(dir: &std::path::Path, out: &mut Vec<(String, String)>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_scripts_rec(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("m1scr") {
            let Some(name) = path
                .file_name()
                .and_then(|n| n.to_str())
                .map(str::to_string)
            else {
                continue;
            };
            let bytes = std::fs::read(&path).unwrap_or_default();
            let source = String::from_utf8_lossy(&bytes).into_owned();
            out.push((name, source));
        }
    }
}

/// Load a project file and build its documentation model. Keeps all
/// m1-typecheck I/O inside the loader so the rest of the crate stays
/// toolchain-agnostic.
pub fn load(
    project_path: &std::path::Path,
    title: String,
) -> Result<DocModel, m1_typecheck::project::LoadError> {
    let mut project = m1_typecheck::Project::load(project_path)?;

    // Infer return types from script bodies before building the model so that
    // `function_doc` can read the populated `return_type` from each symbol.
    let project_dir = project_path
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."));
    let script_pairs = collect_scripts(project_dir);
    let parsed = m1_typecheck::parsed::parse_all(&script_pairs);
    project.infer_return_types(&parsed);

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
/// Ensure a group node exists for `path`, creating every ancestor group up the
/// chain (`Root.Engine.Fuel` also creates `Root.Engine` and `Root`) so the tree
/// is fully connected even through groups that hold no direct members.
fn ensure_group(groups: &mut BTreeMap<String, GroupDoc>, path: &str) {
    if path.is_empty() || groups.contains_key(path) {
        return;
    }
    groups.insert(
        path.to_string(),
        GroupDoc {
            path: path.to_string(),
            ..GroupDoc::default()
        },
    );
    let parent = parent_group(path).to_string();
    ensure_group(groups, &parent);
}

pub fn build_model(project: &Project, title: String) -> DocModel {
    let table = project.symbols();
    let mut groups: BTreeMap<String, GroupDoc> = BTreeMap::new();
    for sym in table.iter() {
        // A documented member lives on its *parent* group's page — the path
        // minus its own leaf segment — at whatever depth that is.
        let parent = parent_group(&sym.path).to_string();
        if parent.is_empty() {
            continue; // a bare top-level name with no enclosing group
        }
        ensure_group(&mut groups, &parent);
        let group = groups
            .get_mut(&parent)
            .expect("ensure_group just created it");
        if let Some(kind) = doc_kind(sym.kind) {
            group.symbols.push(symbol_doc(sym, kind, table));
        } else if is_function(sym.kind) {
            group.functions.push(function_doc(sym));
        } else if matches!(sym.kind, SymbolKind::Table) {
            group.tables.push(table_doc(sym));
        }
    }
    // Wire each node into its parent's `children` list.
    let paths: Vec<String> = groups.keys().cloned().collect();
    for path in &paths {
        let parent = parent_group(path);
        if !parent.is_empty()
            && let Some(p) = groups.get_mut(parent)
        {
            p.children.push(path.clone());
        }
    }
    // Deterministic order: groups by path (BTreeMap), members and children
    // sorted within each node.
    let mut groups: Vec<GroupDoc> = groups.into_values().collect();
    for g in &mut groups {
        g.symbols.sort_by(|a, b| a.path.cmp(&b.path));
        g.functions.sort_by(|a, b| a.path.cmp(&b.path));
        g.tables.sort_by(|a, b| a.path.cmp(&b.path));
        g.children.sort();
        assign_anchors(g);
    }
    let enums = collect_enums(table);
    DocModel {
        title,
        groups,
        enums,
    }
}

/// Collect every enum type referenced by a symbol's `enum_assoc` into a sorted,
/// deduped reference. Members are listed in container order (#27). Anchors are
/// kept unique within the Enums page so each entry is deep-linkable.
fn collect_enums(table: &SymbolTable) -> Vec<EnumDoc> {
    let mut by_name: BTreeMap<String, EnumDoc> = BTreeMap::new();
    for sym in table.iter() {
        let Some(id) = sym.enum_assoc else { continue };
        let et = table.enum_type(id);
        by_name.entry(et.name.clone()).or_insert_with(|| {
            let mut members = et.members.clone();
            members.sort_by(|a, b| a.1.cmp(&b.1).then_with(|| a.0.cmp(&b.0)));
            EnumDoc {
                name: et.name.clone(),
                anchor: anchor_slug(&et.name),
                members: members.into_iter().map(|(n, _)| n).collect(),
                default: et.default.clone(),
                open: et.open,
            }
        });
    }
    // Keep anchors unique within the Enums page (rare slug collisions get -2…).
    let mut enums: Vec<EnumDoc> = by_name.into_values().collect();
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    for e in &mut enums {
        let n = counts.entry(e.anchor.clone()).or_insert(0);
        *n += 1;
        if *n > 1 {
            e.anchor = format!("{}-{n}", e.anchor);
        }
    }
    enums
}

/// Assign each symbol and function on a group page a page-unique anchor id.
/// Symbols and functions share one namespace (they coexist on the same HTML
/// page, where an `id` must be unique). Base slugs come from the shared
/// [`anchor_slug`]; a rare collision gets a deterministic `-2`, `-3`, … suffix.
fn assign_anchors(group: &mut GroupDoc) {
    let mut counts: BTreeMap<String, u32> = BTreeMap::new();
    let mut unique = |path: &str| -> String {
        let base = anchor_slug(path);
        let n = counts.entry(base.clone()).or_insert(0);
        *n += 1;
        if *n == 1 { base } else { format!("{base}-{n}") }
    };
    for s in &mut group.symbols {
        s.anchor = unique(&s.path);
    }
    for f in &mut group.functions {
        f.anchor = unique(&f.path);
    }
    for t in &mut group.tables {
        t.anchor = unique(&t.path);
    }
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
    fn parent_group_strips_the_leaf_segment() {
        assert_eq!(parent_group("Root.Engine.Speed"), "Root.Engine");
        assert_eq!(parent_group("Root.Engine.Gain.Value"), "Root.Engine.Gain");
        assert_eq!(parent_group("Root.Engine"), "Root");
        assert_eq!(parent_group("Root"), "");
    }

    #[test]
    fn assign_anchors_dedupes_collisions_within_a_page() {
        // Three paths that all sanitise to `root-engine-oil-temp`. Symbols and
        // functions share one page namespace, so all three must stay distinct.
        let mut group = GroupDoc {
            path: "Root.Engine".into(),
            symbols: vec![
                SymbolDoc {
                    path: "Root.Engine.Oil Temp".into(),
                    ..Default::default()
                },
                SymbolDoc {
                    path: "Root.Engine.Oil-Temp".into(),
                    ..Default::default()
                },
            ],
            functions: vec![FunctionDoc {
                path: "Root.Engine.Oil.Temp".into(),
                ..Default::default()
            }],
            ..Default::default()
        };
        assign_anchors(&mut group);
        assert_eq!(group.symbols[0].anchor, "root-engine-oil-temp");
        assert_eq!(group.symbols[1].anchor, "root-engine-oil-temp-2");
        assert_eq!(group.functions[0].anchor, "root-engine-oil-temp-3");

        let mut all = [
            group.symbols[0].anchor.clone(),
            group.symbols[1].anchor.clone(),
            group.functions[0].anchor.clone(),
        ];
        all.sort();
        let unique = all.iter().collect::<std::collections::BTreeSet<_>>().len();
        assert_eq!(unique, 3, "anchors must be unique within a page");
    }

    #[test]
    fn build_model_assigns_a_stable_anchor_to_each_symbol() {
        let model = build_model(&Project::from_xml(PROJECT).unwrap(), "Demo".into());
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .unwrap();
        let speed = eng
            .symbols
            .iter()
            .find(|s| s.path == "Root.Engine.Speed")
            .unwrap();
        assert_eq!(speed.anchor, "root-engine-speed");
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

    // `is_function` also matches SymbolKind::Method (BuiltIn.MethodUser), so a
    // method must be collected as a FunctionDoc under its group. All other
    // loader tests use FuncUser only, leaving the Method branch uncovered — a
    // regression dropping methods from docs would otherwise go unnoticed (#21).
    const METHOD_PROJECT: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.MethodUser" Name="Root.Engine.Control">
    <Signature ReturnType="bool">
     <Params>
      <Param Name="Demand" Type="f32"/>
     </Params>
    </Signature>
   </Component>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;

    #[test]
    fn method_collected_under_group() {
        let project = Project::from_xml(METHOD_PROJECT).unwrap();
        let model = build_model(&project, "Demo".into());
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group");
        assert_eq!(
            eng.functions.len(),
            1,
            "a BuiltIn.MethodUser must be collected as a function; got {:?}",
            eng.functions
        );
        let m = &eng.functions[0];
        assert_eq!(m.path, "Root.Engine.Control");
        assert_eq!(
            m.inputs,
            vec![("Demand".to_string(), "float".to_string())],
            "unexpected method inputs: {:?}",
            m.inputs
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
    fn nests_members_under_their_immediate_parent_group() {
        let project = Project::from_xml(PROJECT).unwrap();
        let model = build_model(&project, "Demo".into());
        // The channel sits directly on Root.Engine.
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
        // The deeper parameter `Root.Engine.Gain.Value` is NOT hoisted to
        // Root.Engine — it lives on its own parent-group page Root.Engine.Gain.
        assert!(
            !eng.symbols
                .iter()
                .any(|s| s.path == "Root.Engine.Gain.Value"),
            "deeper member must not be hoisted to the 2-segment group"
        );
        let gain = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine.Gain")
            .expect("Root.Engine.Gain group should exist as its own node");
        assert!(
            gain.symbols.iter().any(
                |s| s.kind == SymbolDocKind::Parameter && s.security.as_deref() == Some("Tune")
            ),
            "parameter with security on its parent group; got {:?}",
            gain.symbols
        );
        // Root.Engine lists Root.Engine.Gain as a child group.
        assert!(
            eng.children.iter().any(|c| c == "Root.Engine.Gain"),
            "Root.Engine should link its child group; got {:?}",
            eng.children
        );
    }

    #[test]
    fn table_is_collected_and_listed_even_without_cfg_metadata() {
        // A BuiltIn.Table with no .m1cfg loaded: table_meta is None, so the
        // shape is unknown — but the table must still be documented (#26), not
        // silently dropped the way doc_kind drops it.
        const TBL: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.Table" Name="Root.Engine.IgnitionMap"/>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;
        let model = build_model(&Project::from_xml(TBL).unwrap(), "Demo".into());
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group");
        let tbl = eng
            .tables
            .iter()
            .find(|t| t.path == "Root.Engine.IgnitionMap")
            .expect("table must be collected");
        assert!(tbl.axes.is_empty(), "no cfg → no axis shape");
        assert!(!tbl.anchor.is_empty(), "table participates in anchors");
    }

    #[test]
    fn deep_nesting_produces_a_node_per_level_with_parent_child_links() {
        // A channel five segments deep: Root.A.B.C.D.Speed. Every ancestor group
        // (Root, Root.A, Root.A.B, Root.A.B.C, Root.A.B.C.D) must exist as its
        // own node, chained parent→child, with the member on the deepest one.
        const DEEP: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.Channel" Name="Root.A.B.C.D.Speed"><Props Type="f32"/></Component>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;
        let model = build_model(&Project::from_xml(DEEP).unwrap(), "Demo".into());
        let node = |p: &str| model.groups.iter().find(|g| g.path == p);

        for level in ["Root", "Root.A", "Root.A.B", "Root.A.B.C", "Root.A.B.C.D"] {
            assert!(node(level).is_some(), "missing group page for {level}");
        }
        // Parent→child links chain down the tree.
        assert_eq!(node("Root").unwrap().children, vec!["Root.A".to_string()]);
        assert_eq!(
            node("Root.A").unwrap().children,
            vec!["Root.A.B".to_string()]
        );
        assert_eq!(
            node("Root.A.B.C").unwrap().children,
            vec!["Root.A.B.C.D".to_string()]
        );
        // The deepest group is a leaf (no child groups) and owns the member.
        let leaf = node("Root.A.B.C.D").unwrap();
        assert!(leaf.children.is_empty(), "deepest node has no child groups");
        assert!(
            leaf.symbols.iter().any(|s| s.path == "Root.A.B.C.D.Speed"),
            "member lands on the deepest group; got {:?}",
            leaf.symbols
        );
        // The shallower groups hold no direct members — they're pure structure.
        assert!(node("Root.A.B").unwrap().symbols.is_empty());
    }

    /// A `BuiltIn.Constant` component must be collected as `SymbolDocKind::Constant`
    /// under its top-level group. This exercises the `doc_kind` branch and
    /// `build_model` path that were previously untested; removing the
    /// `SymbolKind::Constant => Some(SymbolDocKind::Constant)` arm in `doc_kind`
    /// would cause this test to fail.
    #[test]
    fn constant_collected_under_its_group() {
        const CONST_PROJECT: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.Constant" Name="Root.Engine.MaxRpm"><Props Type="u16"/></Component>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;
        let project = Project::from_xml(CONST_PROJECT).unwrap();
        let model = build_model(&project, "Demo".into());
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group");
        assert!(
            eng.symbols.iter().any(|s| s.path == "Root.Engine.MaxRpm"
                && s.kind == SymbolDocKind::Constant
                && s.type_label == "u16"),
            "expected Constant symbol Root.Engine.MaxRpm with type u16; got {:?}",
            eng.symbols
        );
    }

    /// Integration test: a FuncUser whose `.m1scr` body contains `Out = 1.0;`
    /// should have its return type inferred as `"float"` by the loader.
    ///
    /// The script file name uses the `<stem>.m1scr` convention so
    /// `infer_return_types` associates it with the function symbol via the
    /// explicit `Filename=` attribute match.
    #[test]
    fn return_type_inferred_from_script_out_assignment() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let prj_path = dir.path().join("Project.m1prj");
        let script_path = dir.path().join("Engine.Compute.m1scr");

        fs::write(
            &prj_path,
            r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.FuncUser" Filename="Engine.Compute.m1scr" Name="Root.Engine.Compute"/>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#,
        )
        .unwrap();

        // `Out = 1.0;` is a float literal assignment to the return-value object.
        fs::write(&script_path, "Out = 1.0;\n").unwrap();

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
            f.return_type.as_deref(),
            Some("float"),
            "expected float return type from `Out = 1.0;`; got {:?}",
            f.return_type
        );
    }

    /// A function with no `Out =` assignment should have `return_type` as `None`.
    #[test]
    fn return_type_none_when_script_has_no_out_assignment() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let prj_path = dir.path().join("Project.m1prj");
        let script_path = dir.path().join("Engine.Helper.m1scr");

        fs::write(
            &prj_path,
            r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.FuncUser" Filename="Engine.Helper.m1scr" Name="Root.Engine.Helper"/>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#,
        )
        .unwrap();

        // No `Out =` assignment: return type must not be guessed.
        fs::write(&script_path, "local x = 1;\n").unwrap();

        let model = load(&prj_path, "T".into()).unwrap();
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group");
        let f = &eng.functions[0];
        assert_eq!(
            f.return_type, None,
            "no Out = means no return type; got {:?}",
            f.return_type
        );
    }
}
