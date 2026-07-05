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
        for_each_top_path(script.cst.root(), |node, access| {
            let Resolution::Symbol(sym) = resolve(node.text(), &scope) else {
                return; // local / In / Out / library / unresolved → no edge
            };
            if sym.path == from {
                return; // no self-edge
            }
            let mut push = |kind| {
                edges.insert(GraphEdge {
                    from: from.clone(),
                    to: sym.path.clone(),
                    kind,
                });
            };
            match sym.kind {
                SymbolKind::Function | SymbolKind::Method => push(EdgeKind::Call),
                SymbolKind::Channel | SymbolKind::Parameter | SymbolKind::Constant => {
                    match access {
                        Access::Read => push(EdgeKind::Read),
                        Access::Write => push(EdgeKind::Write),
                        // A read-modify-write (`+=`, `|=`, …) both reads and writes
                        // its target, so it contributes an edge of each kind.
                        Access::ReadWrite => {
                            push(EdgeKind::Read);
                            push(EdgeKind::Write);
                        }
                    }
                }
                _ => {} // groups, objects, tables — not a graph edge here
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

/// How a resolved path is accessed at a use site — the edge kind(s) it seeds.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Access {
    /// Read only (the default: a value position, an assignment's right-hand
    /// side, a call argument).
    Read,
    /// Written only (the target of a plain `=` assignment, a `local` name).
    Write,
    /// Both read and written — the target of a compound assignment (`+=`, `|=`,
    /// `>>=`, …), which is a read-modify-write.
    ReadWrite,
}

/// Visit every outermost dotted-path node (an `identifier`/`member_expression`
/// not itself the property half of a member expression, and not inside a type
/// annotation), with how it is accessed. A pre-order walk over an explicit stack
/// so a deep script can't overflow the call stack. Mirrors m1-lsp's
/// `for_each_top_path` over the public CST API.
fn for_each_top_path<'a>(root: Node<'a>, mut f: impl FnMut(Node<'a>, Access)) {
    let mut stack: Vec<(Node<'a>, Option<Node<'a>>, bool)> = vec![(root, None, false)];
    while let Some((node, parent, in_ta)) = stack.pop() {
        let is_path = matches!(node.kind(), Kind::Identifier | Kind::MemberExpression);
        let parent_is_member = parent
            .map(|p| p.kind() == Kind::MemberExpression)
            .unwrap_or(false);
        if is_path && !parent_is_member && !in_ta {
            f(node, access_of(node, parent));
        }
        let child_in_ta = in_ta || node.kind() == Kind::TypeAnnotation;
        for child in node.children().into_iter().rev() {
            stack.push((child, Some(node), child_in_ta));
        }
    }
}

/// How `node` is accessed given its parent from the walk. The write cases are the
/// target of an assignment or the name of a `local` declaration; a *compound*
/// assignment target is a read-modify-write. Everything else is a read. O(1).
fn access_of(node: Node, parent: Option<Node>) -> Access {
    match parent {
        Some(p) if p.kind() == Kind::AssignmentStatement => {
            let is_target = p
                .child_by_field(Field::Target)
                .map(|t| t.byte_range() == node.byte_range())
                .unwrap_or(false);
            if !is_target {
                return Access::Read; // the right-hand side value
            }
            // A compound operator (anything but plain `=`) reads before it writes.
            let compound = p
                .child_by_field(Field::Operator)
                .map(|o| o.text() != "=")
                .unwrap_or(false);
            if compound {
                Access::ReadWrite
            } else {
                Access::Write
            }
        }
        Some(p) if p.kind() == Kind::LocalDeclaration => {
            let is_name = p
                .child_by_field(Field::Name)
                .map(|n| n.byte_range() == node.byte_range())
                .unwrap_or(false);
            if is_name { Access::Write } else { Access::Read }
        }
        _ => Access::Read,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Collect `(path text, access)` for every top-level path in a snippet.
    fn accesses(src: &str) -> Vec<(String, Access)> {
        let cst = m1_core::parse(src);
        let mut out = Vec::new();
        for_each_top_path(cst.root(), |node, access| {
            out.push((node.text().to_string(), access));
        });
        out
    }

    /// #5: a compound assignment (`+=`) is a read-modify-write, so its target is
    /// classified `ReadWrite` (seeding *both* a read and a write edge), while a
    /// plain `=` target is `Write` and every right-hand-side path is `Read`.
    #[test]
    fn compound_assignment_target_is_read_and_write() {
        let a = accesses("Root.Engine.Count += Root.Engine.Delta;\n");
        assert_eq!(
            a.iter()
                .find(|(p, _)| p == "Root.Engine.Count")
                .map(|(_, ac)| *ac),
            Some(Access::ReadWrite),
            "a `+=` target must be read-and-written; got {a:?}"
        );
        assert_eq!(
            a.iter()
                .find(|(p, _)| p == "Root.Engine.Delta")
                .map(|(_, ac)| *ac),
            Some(Access::Read),
            "the right-hand side is a read; got {a:?}"
        );

        // A plain `=` target stays a pure write (no phantom read edge).
        let b = accesses("Root.Engine.Count = Root.Engine.Delta;\n");
        assert_eq!(
            b.iter()
                .find(|(p, _)| p == "Root.Engine.Count")
                .map(|(_, ac)| *ac),
            Some(Access::Write),
            "a plain `=` target must be write-only; got {b:?}"
        );

        // Every compound operator variant is a read-modify-write.
        for op in ["-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<=", ">>="] {
            let src = format!("Root.X {op} Root.Y;\n");
            let acc = accesses(&src);
            assert_eq!(
                acc.iter().find(|(p, _)| p == "Root.X").map(|(_, ac)| *ac),
                Some(Access::ReadWrite),
                "`{op}` target must be ReadWrite; got {acc:?}"
            );
        }
    }
}
