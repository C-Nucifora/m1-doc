//! The project relationship graph (#37): call / read / write / reference edges,
//! derived from the parsed `.m1scr` bodies and the model's resolved references.
//!
//! Edges are extracted by walking each script's CST for every outermost dotted
//! path and resolving it against the project with m1-typecheck's name resolver —
//! the same approach m1-lsp's call hierarchy uses. A path that resolves to a
//! function/method is a **call**; to a channel/parameter/constant a **read** or
//! **write** (by whether it is an assignment target); anything that does not
//! resolve to a documented symbol (locals, `In`/`Out`, library objects, dynamic
//! targets) is dropped — the graph records only honest, resolvable edges.

use crate::model::{DocModel, EdgeKind, GraphEdge, ProjectGraph};
use m1_core::{Field, Kind, Node};
use m1_typecheck::parsed::ParsedScript;
use m1_typecheck::project::Project;
use m1_typecheck::resolve::{Resolution, Scope, resolve};
use m1_typecheck::symbols::SymbolKind;
use m1_typecheck::types::ValueType;
use std::collections::{BTreeSet, HashMap};

/// Build the relationship graph from the parsed scripts and the model's resolved
/// references. Edges are sorted and deduped for deterministic output.
pub fn build_graph(project: &Project, scripts: &[ParsedScript], model: &DocModel) -> ProjectGraph {
    let mut edges: BTreeSet<GraphEdge> = BTreeSet::new();

    for script in scripts {
        // Attribute every edge to the function/method symbol the script backs.
        // A script with no backing function can't be the source of an edge.
        let Some(from) = project.function_symbol_for_script(&script.name) else {
            continue;
        };
        let scope = Scope {
            locals: collect_locals(script.cst.root()),
            group: project.group_for_script(&script.name),
            project: Some(project),
            fn_symbol: Some(from.clone()),
        };
        for_each_top_path(script.cst.root(), |node, is_write| {
            let Resolution::Symbol(sym) = resolve(node.text(), &scope) else {
                return; // local / In / Out / library / unresolved → no edge
            };
            let kind = match sym.kind {
                SymbolKind::Function | SymbolKind::Method => EdgeKind::Call,
                SymbolKind::Channel | SymbolKind::Parameter | SymbolKind::Constant => {
                    if is_write {
                        EdgeKind::Write
                    } else {
                        EdgeKind::Read
                    }
                }
                _ => return, // groups, objects, tables — not a graph edge here
            };
            if sym.path != from {
                edges.insert(GraphEdge {
                    from: from.clone(),
                    to: sym.path.clone(),
                    kind,
                });
            }
        });
    }

    // Reference edges come from #29's already-resolved aliases (no re-derivation).
    for group in &model.groups {
        for r in &group.references {
            if let Some(target) = &r.target_resolved {
                edges.insert(GraphEdge {
                    from: r.path.clone(),
                    to: target.clone(),
                    kind: EdgeKind::Reference,
                });
            }
        }
    }

    ProjectGraph {
        edges: edges.into_iter().collect(),
    }
}

/// Collect every `local` declaration name so the resolver classifies those names
/// as locals (not channels). The exact type is irrelevant for edge extraction —
/// only membership matters — so each is recorded as `Unknown`.
fn collect_locals(root: Node) -> HashMap<String, ValueType> {
    let mut locals = HashMap::new();
    for n in root.descendants() {
        if n.kind() == Kind::LocalDeclaration
            && let Some(name) = n
                .named_children()
                .into_iter()
                .find(|c| c.kind() == Kind::Identifier)
        {
            locals.insert(name.text().to_string(), ValueType::Unknown);
        }
    }
    locals
}

/// Visit every outermost dotted-path node (an `identifier`/`member_expression`
/// not itself the property half of a member expression, and not inside a type
/// annotation), with whether it is being written. A pre-order walk over an
/// explicit stack so a deep script can't overflow the call stack. Mirrors
/// m1-lsp's `for_each_top_path` over the public CST API.
fn for_each_top_path<'a>(root: Node<'a>, mut f: impl FnMut(Node<'a>, bool)) {
    let mut stack: Vec<(Node<'a>, Option<Node<'a>>, bool)> = vec![(root, None, false)];
    while let Some((node, parent, in_ta)) = stack.pop() {
        let is_path = matches!(node.kind(), Kind::Identifier | Kind::MemberExpression);
        let parent_is_member = parent
            .map(|p| p.kind() == Kind::MemberExpression)
            .unwrap_or(false);
        if is_path && !parent_is_member && !in_ta {
            f(node, is_write_of(node, parent));
        }
        let child_in_ta = in_ta || node.kind() == Kind::TypeAnnotation;
        for child in node.children().into_iter().rev() {
            stack.push((child, Some(node), child_in_ta));
        }
    }
}

/// True when `node` is being written: the target of an assignment or the name of
/// a `local` declaration. O(1) given the parent from the walk.
fn is_write_of(node: Node, parent: Option<Node>) -> bool {
    match parent {
        Some(p) if p.kind() == Kind::AssignmentStatement => p
            .child_by_field(Field::Target)
            .map(|t| t.byte_range() == node.byte_range())
            .unwrap_or(false),
        Some(p) if p.kind() == Kind::LocalDeclaration => p
            .child_by_field(Field::Name)
            .map(|n| n.byte_range() == node.byte_range())
            .unwrap_or(false),
        _ => false,
    }
}
