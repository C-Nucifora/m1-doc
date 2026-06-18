//! Builds a [`DocModel`] from a loaded m1-typecheck project. Channels,
//! parameters, constants, and functions are placed on their *immediate* parent
//! group at whatever depth (`Root.Engine.Fuel.Pump` for
//! `Root.Engine.Fuel.Pump.Demand`), and every ancestor group becomes its own
//! node so the full tree is navigable.

use crate::model::{
    AnnotationDoc, CanMessageDoc, CanSignalDoc, DocModel, EnumDoc, EnumMemberDoc, FunctionDoc,
    GroupDoc, ObjectDoc, ReferenceDoc, SymbolDoc, SymbolDocKind, TableAxisDoc, TableDoc,
    anchor_slug,
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

/// `true` for a `BuiltIn.CAN.Signal` — a channel sourced from a `.m1dbc`. These
/// are documented inside their parent message's CAN section (#28), not as plain
/// channels, so the collector skips them in the normal symbol pass.
fn is_can_signal(sym: &Symbol) -> bool {
    sym.classname.as_deref() == Some("BuiltIn.CAN.Signal")
}

/// `true` for a `BuiltIn.CAN.Message` object — a CAN frame whose signals are
/// grouped beneath it (#28), distinct from a plain package object.
fn is_can_message(sym: &Symbol) -> bool {
    sym.classname.as_deref() == Some("BuiltIn.CAN.Message")
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
        // Filled in by the `load()` post-pass once the script is located.
        source_path: None,
        source_text: None,
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
        def_line: sym.def_line,
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
        // Inherited tags (own + ancestor-group), as m1-typecheck unions them (#34).
        tags: sym.tags.clone(),
        type_label,
        quantity: sym.qty.clone(),
        unit,
        base_unit: sym.unit.clone(),
        log_rate_hz: sym.log_rate_hz,
        security: sym.security.clone(),
        enum_ref: enum_name,
        classname: sym.classname.clone(),
        def_line: sym.def_line,
    }
}

/// Build an [`ObjectDoc`] for a package-class object: its class and the paths of
/// its immediate members (#28).
fn object_doc(sym: &Symbol, table: &SymbolTable) -> ObjectDoc {
    let mut members: Vec<String> = table
        .immediate_children(&sym.path)
        .iter()
        .map(|c| c.path.clone())
        .collect();
    members.sort();
    ObjectDoc {
        path: sym.path.clone(),
        anchor: anchor_slug(&sym.path),
        class: sym.class.clone(),
        members,
        def_line: sym.def_line,
    }
}

/// Build a [`CanSignalDoc`] from a `BuiltIn.CAN.Signal` channel (#28).
fn can_signal_doc(sym: &Symbol) -> CanSignalDoc {
    let can = sym.can.as_ref();
    CanSignalDoc {
        path: sym.path.clone(),
        anchor: anchor_slug(&sym.path),
        start_bit: can.and_then(|c| c.start_bit),
        length: can.and_then(|c| c.length),
        multiplier: can.and_then(|c| c.multiplier),
        offset: can.and_then(|c| c.offset),
        range: sym.dbc_range,
        unit: sym.unit.clone(),
    }
}

/// Build a [`CanMessageDoc`] from a `BuiltIn.CAN.Message` object, pulling its
/// frame id/dlc and packing its `BuiltIn.CAN.Signal` children in bit order (#28).
fn can_message_doc(sym: &Symbol, table: &SymbolTable) -> CanMessageDoc {
    let can = sym.can.as_ref();
    let mut signals: Vec<CanSignalDoc> = table
        .immediate_children(&sym.path)
        .iter()
        .filter(|c| is_can_signal(c))
        .map(|c| can_signal_doc(c))
        .collect();
    // Frame order: by start bit, then path for a stable tie-break.
    signals.sort_by(|a, b| {
        a.start_bit
            .cmp(&b.start_bit)
            .then_with(|| a.path.cmp(&b.path))
    });
    CanMessageDoc {
        path: sym.path.clone(),
        anchor: anchor_slug(&sym.path),
        can_id: can.and_then(|c| c.can_id),
        dlc: can.and_then(|c| c.dlc),
        signals,
        // A CAN message is sourced from a `.m1dbc`, so its `def_line` indexes
        // that DBC — not the `Project.m1prj` the model's `m1prj_path` points at.
        // Pairing the two would build a wrong link, so we leave this `None` and
        // emit no jump-to-declaration for CAN entities (#57).
        def_line: None,
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

/// The script's path relative to the project directory, forward-slashed for a
/// stable URL/link (e.g. `Engine/Update.m1scr`). Falls back to the file's base
/// name when it lies outside the project tree (#30).
fn relative_source_path(project_dir: &std::path::Path, script_path: &std::path::Path) -> String {
    let rel = script_path.strip_prefix(project_dir).unwrap_or(script_path);
    rel.components()
        .map(|c| c.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

/// A `.m1scr` file read off disk: its base name, its lossy-decoded source, and
/// the on-disk path it was read from. Collected once by [`collect_scripts`] so
/// the per-function pass can reuse both the source and the path without a second
/// `fs::read` or a recursive `find_file_in_dir` walk.
struct CollectedScript {
    name: String,
    source: String,
    path: std::path::PathBuf,
}

/// Collect every `.m1scr` file under `dir` (recursively). Files in a directory
/// are visited before its subdirectories, so a script directly under `dir`
/// precedes a same-named one nested deeper — matching the "direct first, then
/// recursive walk" preference of the old `resolve_script`. The source is read
/// with lossy UTF-8 decoding to handle Windows-1252 encoded files without
/// aborting the entire collection.
fn collect_scripts(dir: &std::path::Path) -> Vec<CollectedScript> {
    let mut out = Vec::new();
    collect_scripts_rec(dir, &mut out);
    out
}

fn collect_scripts_rec(dir: &std::path::Path, out: &mut Vec<CollectedScript>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    // Visit files before recursing so a shallower script wins a base-name
    // collision over a deeper one (see `script_by_name`).
    let mut subdirs = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
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
            out.push(CollectedScript { name, source, path });
        }
    }
    for sub in subdirs {
        collect_scripts_rec(&sub, out);
    }
}

/// Index the collected scripts by base name, first-wins. Because
/// [`collect_scripts`] visits a directory's files before its subdirectories, a
/// script directly under the project dir shadows a deeper same-named one — the
/// behaviour the old `resolve_script` (`project_dir/filename` first, then a
/// recursive first-match walk) produced. The map borrows each script so the
/// per-function pass reuses the already-read source and path with no extra I/O.
fn script_by_name(scripts: &[CollectedScript]) -> BTreeMap<&str, &CollectedScript> {
    let mut map: BTreeMap<&str, &CollectedScript> = BTreeMap::new();
    for s in scripts {
        map.entry(s.name.as_str()).or_insert(s);
    }
    map
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
    let scripts = collect_scripts(project_dir);
    // `parse_all` wants `(basename, source)` pairs; build them from the scripts
    // we already read into memory rather than reading the files a second time.
    let script_pairs: Vec<(String, String)> = scripts
        .iter()
        .map(|s| (s.name.clone(), s.source.clone()))
        .collect();
    let parsed = m1_typecheck::parsed::parse_all(&script_pairs);
    project.infer_return_types(&parsed);

    let mut model = build_model(&project, title);

    // Record the project file's location, relative to the project dir, so the
    // renderers can build a jump-to-declaration link for every project-sourced
    // symbol (its `def_line` is the line within this file) (#57). This is the
    // same constant path for every symbol, so it lives once on the model.
    model.m1prj_path = Some(relative_source_path(project_dir, project_path));

    // Relationship graph (#37): call/read/write edges from the parsed scripts,
    // plus the reference edges from the model's resolved aliases (#29). Built
    // before the early return below so a project whose functions carry no script
    // filenames still gets its reference edges.
    model.graph = crate::graph::build_graph(&project, &parsed, &model);

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

    // Reuse the scripts already read into memory (keyed by base name) instead of
    // re-walking the tree and re-reading each file per function. The source and
    // its on-disk path are both in hand from `collect_scripts`.
    let by_name = script_by_name(&scripts);

    for group in &mut model.groups {
        for func in &mut group.functions {
            let Some(filename) = path_to_filename.get(&func.path) else {
                continue;
            };
            let Some(script) = by_name.get(filename.as_str()) else {
                continue;
            };
            func.annotations = parse_annotations(&script.source);
            // Retain the source link + body (#30): the project-relative path for
            // a source link, and the text so `--include-source` can embed it.
            func.source_path = Some(relative_source_path(project_dir, &script.path));
            func.source_text = Some(script.source.clone());
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
        // CAN signals are documented inside their parent message's CAN section
        // (#28), not as plain channels — skip them here so they neither double-
        // list nor spawn a synthetic group at the message path.
        if is_can_signal(sym) {
            continue;
        }
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
        } else if matches!(sym.kind, SymbolKind::Object) {
            if is_can_message(sym) {
                group.can_messages.push(can_message_doc(sym, table));
            } else {
                group.objects.push(object_doc(sym, table));
            }
        } else if matches!(sym.kind, SymbolKind::Reference) {
            group.references.push(reference_doc(sym));
        }
    }
    // Resolve each reference's verbatim target to a documented symbol path (#29),
    // keeping the resolution only when it actually names a symbol we document —
    // otherwise the raw string is shown (degrade, never a dangling link). Done
    // once the whole symbol set is known.
    let symbol_paths: std::collections::HashSet<String> = groups
        .values()
        .flat_map(|g| g.symbols.iter().map(|s| s.path.clone()))
        .collect();
    for g in groups.values_mut() {
        for r in &mut g.references {
            r.target_resolved = resolve_reference_target(&r.path, &r.target_raw)
                .filter(|p| symbol_paths.contains(p));
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
        g.objects.sort_by(|a, b| a.path.cmp(&b.path));
        g.can_messages.sort_by(|a, b| a.path.cmp(&b.path));
        g.references.sort_by(|a, b| a.path.cmp(&b.path));
        g.children.sort();
        assign_anchors(g);
    }
    let enums = collect_enums(table);
    DocModel {
        title,
        // The m1-typecheck `Project` API exposes neither `<Project Name>` nor
        // `TargetHardware`, so we cannot fill this without re-parsing the
        // `.m1prj` (which the data contract forbids here). The landing page
        // degrades to a note rather than inventing a value (#32). Tracked
        // upstream: expose `Name`/`TargetHardware` on `Project` so the title
        // default improves and this stops being `None`.
        target_hardware: None,
        groups,
        enums,
        // The relationship graph needs the parsed scripts, which only `load`
        // has; it fills this in. `build_model` (symbols-only) leaves it empty.
        graph: crate::model::ProjectGraph::default(),
        // `build_model` works from symbols alone and has no project path; `load`
        // fills this in once it knows the on-disk `.m1prj` location (#57).
        m1prj_path: None,
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
                // Keep the numeric value (`ContainerOrder` for project-local
                // enums, the enumerator `value` for builtin ones) — it is what
                // a reader needs to interpret a logged value or a CAN payload.
                members: members
                    .into_iter()
                    .map(|(name, value)| EnumMemberDoc { name, value })
                    .collect(),
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
    for o in &mut group.objects {
        o.anchor = unique(&o.path);
    }
    for m in &mut group.can_messages {
        m.anchor = unique(&m.path);
        for s in &mut m.signals {
            s.anchor = unique(&s.path);
        }
    }
    for r in &mut group.references {
        r.anchor = unique(&r.path);
    }
}

/// Build a [`ReferenceDoc`] from a `SymbolKind::Reference` symbol. The target is
/// the `<Props Target>` string verbatim (empty when the reference declares none);
/// `target_resolved` is filled by a later pass once the full symbol set is known.
fn reference_doc(sym: &Symbol) -> ReferenceDoc {
    ReferenceDoc {
        path: sym.path.clone(),
        anchor: anchor_slug(&sym.path),
        target_raw: sym.reference_target.clone().unwrap_or_default(),
        target_resolved: None,
        def_line: sym.def_line,
    }
}

/// Resolve a reference's verbatim `<Props Target>` to a canonical symbol path
/// when it uses an M1 path keyword (#29). `This` is the reference's enclosing
/// group, `Parent` that group's parent, and `Root.…` is absolute. A bare or
/// unrecognised target returns `None` — we never guess, so the renderer shows
/// the raw string instead of a possibly-wrong link.
fn resolve_reference_target(reference_path: &str, target: &str) -> Option<String> {
    let this = parent_group(reference_path); // the reference's enclosing group
    if target == "Root" || target.starts_with("Root.") {
        Some(target.to_string())
    } else if let Some(rest) = target.strip_prefix("This.") {
        (!this.is_empty()).then(|| format!("{this}.{rest}"))
    } else if target == "This" {
        (!this.is_empty()).then(|| this.to_string())
    } else if let Some(rest) = target.strip_prefix("Parent.") {
        let parent = parent_group(this);
        (!parent.is_empty()).then(|| format!("{parent}.{rest}"))
    } else if target == "Parent" {
        let parent = parent_group(this);
        (!parent.is_empty()).then(|| parent.to_string())
    } else {
        None
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

    /// #30: a function whose `Filename=` resolves to a script on disk must carry
    /// the project-relative source path and the body (for `--include-source`).
    #[test]
    fn function_retains_source_path_and_body() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let prj_path = dir.path().join("Project.m1prj");
        let scripts = dir.path().join("Scripts");
        fs::create_dir_all(&scripts).unwrap();
        let script_path = scripts.join("Update.m1scr");

        fs::write(
            &prj_path,
            r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.FuncUser" Filename="Update.m1scr" Name="Root.Engine.Update"/>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#,
        )
        .unwrap();
        fs::write(&script_path, "Out = In.Speed * 2;\n").unwrap();

        let model = load(&prj_path, "T".into()).unwrap();
        let f = &model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group")
            .functions[0];
        // Path is project-relative and forward-slashed (the recursive walk found
        // it under Scripts/).
        assert_eq!(
            f.source_path.as_deref(),
            Some("Scripts/Update.m1scr"),
            "source path wrong; got {:?}",
            f.source_path
        );
        assert_eq!(
            f.source_text.as_deref(),
            Some("Out = In.Speed * 2;\n"),
            "source body not retained; got {:?}",
            f.source_text
        );
    }

    /// `script_by_name` indexes collected scripts first-wins, and
    /// `collect_scripts` visits a directory's files before recursing — so a
    /// script directly under the project dir shadows a same-named one nested
    /// deeper. This preserves the old `resolve_script` precedence
    /// (`project_dir/filename` first, then a recursive first-match walk) now
    /// that the per-function pass resolves filenames through the in-memory map
    /// instead of re-walking the tree. Without files-before-subdirs ordering the
    /// nested copy could win and the body/source-link would point at the wrong
    /// file.
    #[test]
    fn collected_script_lookup_prefers_a_shallower_same_named_file() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let nested = dir.path().join("Nested");
        fs::create_dir_all(&nested).unwrap();
        // Same base name in the project root and a subdirectory.
        fs::write(dir.path().join("Dup.m1scr"), "// root\n").unwrap();
        fs::write(nested.join("Dup.m1scr"), "// nested\n").unwrap();

        let scripts = collect_scripts(dir.path());
        let by_name = script_by_name(&scripts);
        let chosen = by_name.get("Dup.m1scr").expect("Dup.m1scr collected");
        assert_eq!(
            chosen.source, "// root\n",
            "the shallower (project-root) file must win the base-name collision; got {:?}",
            chosen.source
        );
        assert_eq!(
            chosen.path,
            dir.path().join("Dup.m1scr"),
            "the winning script must carry the root path, not the nested one"
        );
    }

    /// End-to-end guard for the refactor: a function whose `.m1scr` lives in a
    /// subdirectory must still get its annotations, source path and body — proving
    /// the in-memory `script_by_name` lookup resolves a nested filename the way
    /// the removed recursive `resolve_script` walk used to, with no second read.
    #[test]
    fn nested_script_resolved_via_collected_map() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let prj_path = dir.path().join("Project.m1prj");
        let scripts = dir.path().join("Scripts").join("Engine");
        fs::create_dir_all(&scripts).unwrap();
        fs::write(
            &prj_path,
            r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.FuncUser" Filename="Update.m1scr" Name="Root.Engine.Update"/>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#,
        )
        .unwrap();
        fs::write(
            scripts.join("Update.m1scr"),
            "// @m1:requires-finite\nOut = In.Speed * 2;\n",
        )
        .unwrap();

        let model = load(&prj_path, "T".into()).unwrap();
        let f = &model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group")
            .functions[0];
        assert_eq!(
            f.source_path.as_deref(),
            Some("Scripts/Engine/Update.m1scr"),
            "nested source path must be found via the in-memory map; got {:?}",
            f.source_path
        );
        assert_eq!(
            f.source_text.as_deref(),
            Some("// @m1:requires-finite\nOut = In.Speed * 2;\n"),
            "nested source body must be retained; got {:?}",
            f.source_text
        );
        assert_eq!(
            f.annotations.len(),
            1,
            "annotation from the nested script must be surfaced; got {:?}",
            f.annotations
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

    /// #28: a `MoTeC Input.Sensor` is a `SymbolKind::Object`; the loader must
    /// document it with its class and the paths of its immediate members, on its
    /// parent group page.
    #[test]
    fn object_documented_with_class_and_members() {
        const OBJ: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Inputs"/>
   <Component Classname="MoTeC Input.Sensor" Name="Root.Inputs.OilP"/>
   <Component Classname="BuiltIn.Channel" Name="Root.Inputs.OilP.Resource"><Props Type="f32"/></Component>
   <Component Classname="BuiltIn.Parameter" Name="Root.Inputs.OilP.Calibration"><Props Type="f32"/></Component>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;
        let model = build_model(&Project::from_xml(OBJ).unwrap(), "Demo".into());
        let inputs = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Inputs")
            .expect("Root.Inputs group");
        let obj = inputs
            .objects
            .iter()
            .find(|o| o.path == "Root.Inputs.OilP")
            .expect("sensor object must be documented");
        assert_eq!(obj.class.as_deref(), Some("MoTeC Input.Sensor"));
        assert!(
            obj.members
                .contains(&"Root.Inputs.OilP.Resource".to_string())
                && obj
                    .members
                    .contains(&"Root.Inputs.OilP.Calibration".to_string()),
            "object must list its immediate members; got {:?}",
            obj.members
        );
        assert!(!obj.anchor.is_empty(), "object participates in anchors");
    }

    /// #28: a `.m1dbc` contributes a CAN message object and signal channels. The
    /// loader must surface the message's id/dlc and pack its signals (with bit
    /// layout, scale, range, unit) under it — and NOT also list the signals as
    /// plain channels.
    #[test]
    fn can_message_and_signals_documented_from_dbc() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let prj_path = dir.path().join("Project.m1prj");
        let dbc_path = dir.path().join("Bus.m1dbc");
        fs::write(
            &prj_path,
            r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List/></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#,
        )
        .unwrap();
        fs::write(
            &dbc_path,
            r#"<?xml version="1.0"?>
<DBC>
 <ComponentStream><List>
   <Component Classname="BuiltIn.CAN.DBC" Name="Bus"/>
   <Component Classname="BuiltIn.CAN.Message" Name="Bus.EngineData">
    <Props CANId="160" DLC="8"/>
   </Component>
   <Component Classname="BuiltIn.CAN.Signal" Name="Bus.EngineData.EngineSpeed">
    <Props Type="u16" Qty="rpm" StartBit="24" Length="16" Multiplier="0.5" Offset="0"/>
   </Component>
 </List></ComponentStream>
</DBC>"#,
        )
        .unwrap();

        let project = Project::from_xml(&fs::read_to_string(&prj_path).unwrap())
            .unwrap()
            .with_dbc(&dbc_path, "Bus.m1dbc")
            .unwrap();
        let model = build_model(&project, "Demo".into());

        // The message lands on its parent group page (the DBC bus node).
        let bus = model
            .groups
            .iter()
            .find(|g| g.path == "Bus")
            .expect("Bus group");
        let msg = bus
            .can_messages
            .iter()
            .find(|m| m.path == "Bus.EngineData")
            .expect("CAN message must be documented");
        assert_eq!(msg.can_id, Some(160));
        assert_eq!(msg.dlc, Some(8));
        let sig = msg
            .signals
            .iter()
            .find(|s| s.path == "Bus.EngineData.EngineSpeed")
            .expect("signal must be packed under its message");
        assert_eq!(sig.start_bit, Some(24));
        assert_eq!(sig.length, Some(16));
        assert_eq!(sig.multiplier, Some(0.5));
        assert_eq!(sig.unit.as_deref(), Some("rpm"));
        assert!(sig.range.is_some(), "u16 signal has a bounded range");

        // The signal must NOT also appear as a plain channel anywhere, and no
        // synthetic group is created at the message path.
        assert!(
            model.groups.iter().all(|g| g
                .symbols
                .iter()
                .all(|s| s.path != "Bus.EngineData.EngineSpeed")),
            "CAN signal must not double-list as a plain channel"
        );
        assert!(
            model.groups.iter().all(|g| g.path != "Bus.EngineData"),
            "no synthetic group should be created at the message path"
        );
    }

    /// #34: a symbol's tags (own + inherited from its ancestor groups) must be
    /// carried onto the `SymbolDoc` so the renderer can build the tag facet and
    /// filter. A group-level `Tags=` is inherited by its members.
    #[test]
    fn symbol_tags_are_surfaced_including_inherited() {
        const TAGGED: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"><Props SelectedTags="engine"/></Component>
   <Component Classname="BuiltIn.Channel" Name="Root.Engine.Speed"><Props Type="f32" SelectedTags="speed"/></Component>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;
        let model = build_model(&Project::from_xml(TAGGED).unwrap(), "Demo".into());
        let eng = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group");
        let speed = eng
            .symbols
            .iter()
            .find(|s| s.path == "Root.Engine.Speed")
            .expect("speed channel");
        assert!(
            speed.tags.contains(&"speed".to_string()) && speed.tags.contains(&"engine".to_string()),
            "own + inherited group tags must be surfaced; got {:?}",
            speed.tags
        );
        // The facet collapses to the sorted, deduped union.
        assert_eq!(
            model.tags(),
            vec!["engine".to_string(), "speed".to_string()]
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

    /// #29: a `BuiltIn.Reference` is documented with its `<Props Target>` and,
    /// where the target uses a path keyword and names a symbol we document, a
    /// resolved canonical path; an off-model / bare target stays raw (no
    /// invented link).
    #[test]
    fn references_documented_with_resolved_and_raw_targets() {
        const REFS: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Sensors"/>
   <Component Classname="BuiltIn.Channel" Name="Root.Sensors.OilP"><Props Type="f32"/></Component>
   <Component Classname="BuiltIn.Reference" Name="Root.Sensors.AliasThis"><Props Target="This.OilP"/></Component>
   <Component Classname="BuiltIn.Reference" Name="Root.Sensors.AliasAbs"><Props Target="Root.Sensors.OilP"/></Component>
   <Component Classname="BuiltIn.Reference" Name="Root.Sensors.Dangling"><Props Target="Nowhere.X"/></Component>
   <Component Classname="BuiltIn.Reference" Name="Root.Sensors.NoTarget"><Props/></Component>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#;
        let model = build_model(&Project::from_xml(REFS).unwrap(), "Demo".into());
        let g = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Sensors")
            .expect("Root.Sensors group");
        let by_path = |p: &str| g.references.iter().find(|r| r.path == p).unwrap();

        // `This.OilP` from Root.Sensors.* resolves to the documented sibling.
        let this = by_path("Root.Sensors.AliasThis");
        assert_eq!(this.target_raw, "This.OilP");
        assert_eq!(
            this.target_resolved.as_deref(),
            Some("Root.Sensors.OilP"),
            "This-relative target must resolve to the documented symbol"
        );
        // An absolute `Root.…` target that names a documented symbol resolves too.
        assert_eq!(
            by_path("Root.Sensors.AliasAbs").target_resolved.as_deref(),
            Some("Root.Sensors.OilP")
        );
        // A target that does not name any documented symbol stays raw (no link).
        let dangling = by_path("Root.Sensors.Dangling");
        assert_eq!(dangling.target_raw, "Nowhere.X");
        assert_eq!(
            dangling.target_resolved, None,
            "an off-model target must not be resolved into a dangling link"
        );
        // A reference with no Target declares an empty raw target, never invented.
        let none = by_path("Root.Sensors.NoTarget");
        assert_eq!(none.target_raw, "");
        assert_eq!(none.target_resolved, None);
        assert!(!none.anchor.is_empty(), "references participate in anchors");
    }

    /// #57: a project-sourced symbol must carry the `def_line` of its
    /// `<Component>` declaration, and the model must record the project file
    /// path once — together these let the renderers build a jump-to-declaration
    /// link. The line is 0-based (the `Speed` channel is on the 5th line of the
    /// XML below → `def_line == 4`).
    #[test]
    fn symbol_carries_def_line_and_model_records_m1prj_path() {
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let prj_path = dir.path().join("Project.m1prj");
        fs::write(
            &prj_path,
            "<?xml version=\"1.0\"?>\n\
<MoTeCM1BuildSession>\n\
 <Project Name=\"Demo\" TargetHardware=\"ecu120\">\n\
  <ComponentStream><List>\n\
   <Component Classname=\"BuiltIn.Channel\" Name=\"Root.Engine.Speed\"><Props Type=\"f32\"/></Component>\n\
  </List></ComponentStream>\n\
 </Project>\n\
</MoTeCM1BuildSession>",
        )
        .unwrap();

        let model = load(&prj_path, "Demo".into()).unwrap();
        assert_eq!(
            model.m1prj_path.as_deref(),
            Some("Project.m1prj"),
            "the model must record the project-relative .m1prj path; got {:?}",
            model.m1prj_path
        );
        let speed = model
            .groups
            .iter()
            .find(|g| g.path == "Root.Engine")
            .expect("Root.Engine group")
            .symbols
            .iter()
            .find(|s| s.path == "Root.Engine.Speed")
            .expect("Speed channel");
        assert_eq!(
            speed.def_line,
            Some(4),
            "the channel must carry the 0-based line of its declaration; got {:?}",
            speed.def_line
        );
    }

    /// #37: the relationship graph extracts a call edge (one function invoking
    /// another), a read edge (a function reading a channel), a write edge (a
    /// function writing a channel), and a reference edge (#29's alias) — each
    /// resolved to a documented symbol; locals and `Out` produce no edge.
    #[test]
    fn graph_extracts_call_read_write_and_reference_edges() {
        use crate::model::EdgeKind;
        use std::fs;
        use tempfile::TempDir;

        let dir = TempDir::new().unwrap();
        let prj = dir.path().join("Project.m1prj");
        fs::write(
            &prj,
            r#"<?xml version="1.0"?>
<MoTeCM1BuildSession>
 <Project Name="Demo" TargetHardware="ecu120">
  <ComponentStream><List>
   <Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
   <Component Classname="BuiltIn.Channel" Name="Root.Engine.Speed"><Props Type="f32"/></Component>
   <Component Classname="BuiltIn.Channel" Name="Root.Engine.Output"><Props Type="f32"/></Component>
   <Component Classname="BuiltIn.FuncUser" Filename="Helper.m1scr" Name="Root.Engine.Helper"/>
   <Component Classname="BuiltIn.FuncUser" Filename="Update.m1scr" Name="Root.Engine.Update"/>
   <Component Classname="BuiltIn.Reference" Name="Root.Engine.Alias"><Props Target="This.Speed"/></Component>
  </List></ComponentStream>
 </Project>
</MoTeCM1BuildSession>"#,
        )
        .unwrap();
        // Update calls Helper, reads Speed, writes Output, and uses a local +
        // `Out` (neither of which must produce an edge).
        fs::write(
            dir.path().join("Update.m1scr"),
            "local tmp = Root.Engine.Speed;\nRoot.Engine.Output = Root.Engine.Helper(tmp);\nOut = tmp;\n",
        )
        .unwrap();
        fs::write(dir.path().join("Helper.m1scr"), "Out = In.X;\n").unwrap();

        let model = load(&prj, "Demo".into()).unwrap();
        let has = |from: &str, to: &str, kind: EdgeKind| {
            model
                .graph
                .edges
                .iter()
                .any(|e| e.from == from && e.to == to && e.kind == kind)
        };

        assert!(
            has("Root.Engine.Update", "Root.Engine.Helper", EdgeKind::Call),
            "missing call edge; got {:?}",
            model.graph.edges
        );
        assert!(
            has("Root.Engine.Update", "Root.Engine.Speed", EdgeKind::Read),
            "missing read edge; got {:?}",
            model.graph.edges
        );
        assert!(
            has("Root.Engine.Update", "Root.Engine.Output", EdgeKind::Write),
            "missing write edge; got {:?}",
            model.graph.edges
        );
        assert!(
            has(
                "Root.Engine.Alias",
                "Root.Engine.Speed",
                EdgeKind::Reference
            ),
            "missing reference edge; got {:?}",
            model.graph.edges
        );
        // `Out` and the local `tmp` must not appear as edge targets — they don't
        // resolve to documented symbols (degrade, never fake).
        assert!(
            !model
                .graph
                .edges
                .iter()
                .any(|e| e.to == "Out" || e.to == "tmp"),
            "Out / local must not produce edges; got {:?}",
            model.graph.edges
        );
        // Edges are sorted+deduped: no duplicate (from,to,kind) triple.
        let mut seen = std::collections::HashSet::new();
        for e in &model.graph.edges {
            assert!(
                seen.insert((&e.from, &e.to, e.kind)),
                "duplicate edge {e:?}"
            );
        }
    }
}
