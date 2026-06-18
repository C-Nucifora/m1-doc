//! The in-memory documentation model — the single source of truth that the
//! renderers read. No m1-core / m1-typecheck types leak past this boundary.

/// Derive a URL-safe anchor slug from a symbol/function path: lowercase, with
/// every run of non-alphanumeric characters collapsed to a single `-`
/// (e.g. `Root.Engine.On 100Hz` → `root-engine-on-100hz`). The single shared
/// derivation used by every renderer so Markdown and HTML never drift.
///
/// This is the *base* slug; per-page collision resolution (a `-2` suffix on the
/// rare clash) is applied where the page is assembled (the loader), so the
/// final [`SymbolDoc::anchor`] / [`FunctionDoc::anchor`] are unique within a
/// page. Returns `"symbol"` for an input with no alphanumeric characters.
pub fn anchor_slug(path: &str) -> String {
    let mut out = String::new();
    let mut pending_dash = false;
    for ch in path.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !out.is_empty() {
                out.push('-');
            }
            pending_dash = false;
            out.push(ch.to_ascii_lowercase());
        } else {
            pending_dash = true;
        }
    }
    if out.is_empty() {
        out.push_str("symbol");
    }
    out
}

/// One `@m1:` annotation attached to a function's script.
///
/// Each argument is rendered to a `String`: a positional becomes its value; a
/// named becomes `key=value`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct AnnotationDoc {
    /// The bare kind name without the `@m1:` marker (e.g. `"requires-finite"`).
    pub kind: String,
    /// Each argument rendered as a string.
    pub args: Vec<String>,
}

/// One documented function or method.
///
/// `inputs` holds the declared input parameters in declaration order, each as
/// `(name, type_label)` where `type_label` is the human-readable type string
/// (e.g. `"float"`, `"bool"`). Empty when the component declares no signature.
/// `annotations` holds every `@m1:` annotation found in the function's script,
/// in source order. Empty when none are found or no script is available.
/// `return_type` is the inferred or declared return type label (e.g. `"float"`,
/// `"bool"`), or `None` when the type could not be determined.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct FunctionDoc {
    pub path: String,
    /// Stable, page-unique anchor id (see [`anchor_slug`]); assembled by the
    /// loader so both renderers point at the same target.
    pub anchor: String,
    pub inputs: Vec<(String, String)>,
    pub return_type: Option<String>,
    pub annotations: Vec<AnnotationDoc>,
    /// Execution rate in Hz, derived from the script's `SelectedTrigger`
    /// (e.g. a `100 Hz` event). `None` when the function has no rate-bearing
    /// trigger (`On Startup`, untriggered) — rendered as `—`, never faked.
    pub call_rate_hz: Option<f64>,
    /// Project-relative path of the `.m1scr` that implements this function
    /// (e.g. `Engine/Update.m1scr`), forward-slashed. `None` when the function
    /// declares no `Filename=` or the script could not be located (#30).
    pub source_path: Option<String>,
    /// The script body, retained by the loader so `--include-source` can embed
    /// it. `None` when no script was read. Carried on the model (rather than
    /// re-read at render time) because the renderers have no filesystem access.
    pub source_text: Option<String>,
}

/// One documented symbol (channel / parameter / constant).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SymbolDoc {
    pub path: String,
    /// Stable, page-unique anchor id (see [`anchor_slug`]); assembled by the
    /// loader so both renderers point at the same target.
    pub anchor: String,
    pub kind: SymbolDocKind,
    /// Tags inherited from the symbol and every ancestor group (#34), in the
    /// order m1-typecheck unions them (own tags first, then inherited). Empty
    /// when neither the symbol nor an ancestor declares a tag. Surfaced so a
    /// tag-organised project gets tag-based browsing and filtering.
    pub tags: Vec<String>,
    /// The storage type to show: `declared_type` verbatim when present, else the
    /// resolved value type's display string. Always present — every symbol has at
    /// least a resolved `ValueType`.
    pub type_label: String,
    /// The physical quantity / dimension key (e.g. `rad/s`, `Pa`, `K`), from the
    /// component's `Qty`. `None` when the symbol declares no quantity.
    pub quantity: Option<String>,
    /// The human-visible **display** unit (e.g. `rpm`, `kPa`) from
    /// `<Locale><Default Unit="…">` — what MoTeC Build and the dash show.
    pub unit: Option<String>,
    /// The stored **base** unit derived from `Qty` (e.g. `rad/s`). Shown
    /// alongside [`Self::unit`] only when the two differ (calibration vs logging
    /// see different units); collapsed when identical or absent.
    pub base_unit: Option<String>,
    /// Default logging rate in Hz (`DefaultLogRate`). `None` when unset.
    pub log_rate_hz: Option<f64>,
    pub security: Option<String>,
    /// When this symbol is enum-typed, the name of its [`EnumDoc`] so the type
    /// cell can link to the Enums reference. `None` for non-enum symbols.
    pub enum_ref: Option<String>,
    /// The component's raw package `Classname` (e.g. `BuiltIn.Channel`,
    /// `MoTeC Input.Sensor`, `BuiltIn.CAN.Signal`). Surfaced (#28) so readers can
    /// tell a plain channel from a generated IO method or a sensor input. `None`
    /// for symbols not sourced from a project/DBC `<Component>`.
    pub classname: Option<String>,
    /// 0-based line of this symbol's `<Component>` declaration in the project
    /// file ([`DocModel::m1prj_path`]), so a reader can jump to where it is
    /// declared (#57). `None` for symbols not sourced from the `.m1prj`
    /// (e.g. CAN signals from a `.m1dbc`).
    pub def_line: Option<u32>,
}

/// One package-class object component — a `SymbolKind::Object`, e.g. a
/// `MoTeC Input.Sensor` or a CAN DBC root. Carries its class and the paths of
/// its immediate members so the reader sees what the object contains (#28).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ObjectDoc {
    pub path: String,
    /// Stable, page-unique anchor id (see [`anchor_slug`]).
    pub anchor: String,
    /// The component's package class (e.g. `MoTeC Input.Sensor`), when known.
    pub class: Option<String>,
    /// Full paths of the object's immediate members, sorted. Empty for a leaf.
    pub members: Vec<String>,
    /// 0-based line of this object's `<Component>` declaration in the project
    /// file ([`DocModel::m1prj_path`]), for jump-to-declaration (#57). `None`
    /// when not sourced from the `.m1prj`.
    pub def_line: Option<u32>,
}

/// One CAN signal within a message frame (a `BuiltIn.CAN.Signal` channel): its
/// bit layout, linear scaling, physical range, and engineering unit, all from
/// the `.m1dbc`. Every field is optional and rendered as `—` when absent (#28).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CanSignalDoc {
    pub path: String,
    /// Stable, page-unique anchor id (see [`anchor_slug`]).
    pub anchor: String,
    pub start_bit: Option<u32>,
    pub length: Option<u32>,
    /// Scale factor (`physical = raw * multiplier + offset`).
    pub multiplier: Option<f64>,
    pub offset: Option<f64>,
    /// Physical `(min, max)` range, when the signal is integer-typed.
    pub range: Option<(f64, f64)>,
    pub unit: Option<String>,
}

/// One CAN message frame: a `BuiltIn.CAN.Message` object with its `can_id`/`dlc`
/// and the signals packed into it, grouped under the message (#28).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CanMessageDoc {
    pub path: String,
    /// Stable, page-unique anchor id (see [`anchor_slug`]).
    pub anchor: String,
    pub can_id: Option<u32>,
    pub dlc: Option<u32>,
    /// Signals in this frame, in bit order (`start_bit`, then path).
    pub signals: Vec<CanSignalDoc>,
    /// 0-based line of this message's `<Component>` declaration in the project
    /// file ([`DocModel::m1prj_path`]), for jump-to-declaration (#57). A CAN
    /// message defined only in a `.m1dbc` has no `.m1prj` line, so this is
    /// `None` and no link is shown.
    pub def_line: Option<u32>,
}

/// One documented enum type used in the project: its name, enumerators (member
/// name + numeric value, in container order), default value, and whether it is
/// `open` (firmware-supplied — the listed members may not be exhaustive).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnumDoc {
    pub name: String,
    /// Stable, page-unique anchor id within the Enums reference page.
    pub anchor: String,
    pub members: Vec<EnumMemberDoc>,
    pub default: Option<String>,
    pub open: bool,
}

/// One enumerator of an [`EnumDoc`]: its name and the underlying numeric value.
///
/// The M1 Development Manual (Data Types > Enumeration) defines an enum as a
/// value→name mapping (e.g. `-1 = Error`, `0 = Stopped`, `1 = Cranking`, …) —
/// the `value` is what is stored on the wire / in logs and what scripts compare
/// against, so it is definitional, not decoration. For project-local enums it
/// is the `ContainerOrder`; for builtin/intrinsic enums it is the enumerator's
/// `value`. m1-typecheck treats both as the enumerator value.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EnumMemberDoc {
    pub name: String,
    pub value: i64,
}

/// One input axis of a calibration table: its breakpoint count and (when the
/// `.m1cfg` declares one) engineering unit.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableAxisDoc {
    pub size: u32,
    pub unit: Option<String>,
}

/// One documented `BuiltIn.Table` calibration map. `axes` is in X, Y, Z order
/// (`axes.len()` is the table's dimension) and is empty when no `.m1cfg` was
/// loaded — the table is still listed by name, its shape just unknown.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TableDoc {
    pub path: String,
    /// Stable, page-unique anchor id (see [`anchor_slug`]).
    pub anchor: String,
    pub axes: Vec<TableAxisDoc>,
    /// Unit of the interpolated output value (the table body), when known.
    pub output_unit: Option<String>,
    /// 0-based line of this table's `<Component>` declaration in the project
    /// file ([`DocModel::m1prj_path`]), for jump-to-declaration (#57). `None`
    /// when not sourced from the `.m1prj`.
    pub def_line: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymbolDocKind {
    #[default]
    Channel,
    Parameter,
    Constant,
}

impl SymbolDocKind {
    /// Section heading for a group page.
    pub fn plural(self) -> &'static str {
        match self {
            SymbolDocKind::Channel => "Channels",
            SymbolDocKind::Parameter => "Parameters",
            SymbolDocKind::Constant => "Constants",
        }
    }
}

/// One `BuiltIn.Reference` component — an alias that points at another symbol
/// or path via its `.m1prj` `<Props Target="…">` (#29). The target is captured
/// verbatim by m1-typecheck and never resolved there; the loader resolves the
/// `This`/`Parent`/`Root`-relative and absolute forms to a canonical symbol
/// path *only* when that path is a documented symbol — otherwise the raw string
/// is shown as-is (degrade, never invent a dangling link).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ReferenceDoc {
    pub path: String,
    /// Stable, page-unique anchor id (see [`anchor_slug`]).
    pub anchor: String,
    /// The `<Props Target>` string verbatim (e.g. `This.Value`, `Root.Engine.Speed`).
    /// Always shown so the reader sees exactly what the project declares.
    pub target_raw: String,
    /// The target resolved to a canonical symbol path, set **only** when it
    /// resolves to a symbol present in this model (so the renderer can link it
    /// and build the inverse "used by"). `None` for unresolvable or off-model
    /// targets — the raw string is shown instead.
    pub target_resolved: Option<String>,
    /// 0-based line of this reference's `<Component>` declaration in the project
    /// file ([`DocModel::m1prj_path`]), for jump-to-declaration (#57). `None`
    /// when not sourced from the `.m1prj`.
    pub def_line: Option<u32>,
}

/// One node in the group tree — a group at any depth (`Root`, `Root.Engine`,
/// `Root.Engine.Fuel.Pump`), the symbols/functions declared **directly** under
/// it, and the full paths of its **immediate** child groups. Each node gets its
/// own page; descendants live on their own pages, reachable via [`Self::children`].
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GroupDoc {
    pub path: String,
    pub symbols: Vec<SymbolDoc>,
    /// Functions and methods declared directly in this group, sorted by path.
    pub functions: Vec<FunctionDoc>,
    /// Calibration tables declared directly in this group, sorted by path.
    pub tables: Vec<TableDoc>,
    /// Package-class objects (sensors, class instances, CAN DBC roots) declared
    /// directly in this group, sorted by path (#28).
    pub objects: Vec<ObjectDoc>,
    /// CAN message frames declared directly in this group, sorted by path (#28).
    pub can_messages: Vec<CanMessageDoc>,
    /// `BuiltIn.Reference` aliases declared directly in this group, sorted by
    /// path (#29).
    pub references: Vec<ReferenceDoc>,
    /// Full paths of the immediate child groups, sorted. Empty for a leaf group.
    pub children: Vec<String>,
}

/// The kind of a [`GraphEdge`] in the project relationship graph (#37). `Call`
/// is a function/method invoking another; `Read`/`Write` are a function reading
/// or writing a channel/parameter/constant; `Reference` is a `BuiltIn.Reference`
/// alias pointing at its target (from #29).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeKind {
    Call,
    Read,
    Write,
    Reference,
}

impl EdgeKind {
    /// Stable lowercase name used in JSON and diagram labels.
    pub fn as_str(self) -> &'static str {
        match self {
            EdgeKind::Call => "call",
            EdgeKind::Read => "read",
            EdgeKind::Write => "write",
            EdgeKind::Reference => "reference",
        }
    }
}

/// One typed edge in the relationship graph: `from` and `to` are canonical
/// symbol paths (a function, channel, parameter, constant, or reference), and
/// `kind` says how they relate. Only edges whose endpoints resolve to documented
/// symbols are recorded — dynamic/unresolved targets are dropped (#37).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct GraphEdge {
    pub from: String,
    pub to: String,
    pub kind: EdgeKind,
}

/// The project relationship graph (#37): every call/read/write/reference edge,
/// sorted and deduped so the output is deterministic. Nodes are the symbol paths
/// already documented in the model — the graph carries only the edges.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ProjectGraph {
    pub edges: Vec<GraphEdge>,
}

/// The kind of an [`AnchoredEntity`] — every documented thing that has its own
/// `<page>#<anchor>` deep link. Used by [`DocModel::anchored_entities`] so the
/// two link-building consumers (the search index and the graph node-href map)
/// share one traversal and can each filter to the kinds they care about, rather
/// than hand-walking the model in parallel and drifting (the historical bug:
/// the search index and the graph href map covered different subsets).
///
/// Symbols are split into channel / parameter / constant so the search index can
/// label them without re-inspecting the symbol; the graph map ignores the
/// distinction (it keeps everything except [`AnchoredKind::Enum`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnchoredKind {
    Channel,
    Parameter,
    Constant,
    Function,
    Table,
    Object,
    CanMessage,
    CanSignal,
    Reference,
    Enum,
}

impl AnchoredKind {
    /// The kind for a [`SymbolDocKind`] symbol.
    pub fn for_symbol(kind: SymbolDocKind) -> Self {
        match kind {
            SymbolDocKind::Channel => AnchoredKind::Channel,
            SymbolDocKind::Parameter => AnchoredKind::Parameter,
            SymbolDocKind::Constant => AnchoredKind::Constant,
        }
    }

    /// A stable, human-readable label for the kind — the text the search index
    /// shows (and the single place that text is defined).
    pub fn label(self) -> &'static str {
        match self {
            AnchoredKind::Channel => "channel",
            AnchoredKind::Parameter => "parameter",
            AnchoredKind::Constant => "constant",
            AnchoredKind::Function => "function",
            AnchoredKind::Table => "table",
            AnchoredKind::Object => "object",
            AnchoredKind::CanMessage => "CAN message",
            AnchoredKind::CanSignal => "CAN signal",
            AnchoredKind::Reference => "reference",
            AnchoredKind::Enum => "enum",
        }
    }
}

/// One documented entity that carries a stable `<page>#<anchor>` deep link, as
/// yielded by [`DocModel::anchored_entities`]. `path` is the entity's full path
/// (the enum's name for [`AnchoredKind::Enum`]); `page` is the HTML file it is
/// documented on (`<group>.html`, or `enums.html` for an enum); `anchor` is its
/// page-unique anchor. The canonical deep link is `format!("{page}#{anchor}")`,
/// defined once via [`Self::href`] so no consumer reconstructs it by hand.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnchoredEntity<'a> {
    pub kind: AnchoredKind,
    pub path: &'a str,
    pub page: String,
    pub anchor: &'a str,
    /// The owning group's path (the entity's home group), or `"Enums"` for an
    /// enum, which lives on the shared reference page rather than a group page.
    pub group: &'a str,
    /// A short descriptive hint for the entity — its display/quantity unit
    /// (symbol, CAN signal), return type (function), output unit (table), or
    /// class (object); empty when the kind carries none. Surfaced here so the
    /// search index does not re-walk the model to recover it.
    pub hint: &'a str,
}

impl AnchoredEntity<'_> {
    /// The canonical `<page>#<anchor>` deep link for this entity. The single
    /// place the link shape is built, so the search index and the graph
    /// node-href map can never disagree on it again.
    pub fn href(&self) -> String {
        format!("{}#{}", self.page, self.anchor)
    }
}

/// The whole project's documentation: the group tree plus the project-wide
/// Enums reference (sorted by name, deduped) and the relationship graph (#37).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DocModel {
    pub title: String,
    /// Target hardware (`<Project TargetHardware="…">`), when known. The
    /// m1-typecheck `Project` API does not currently expose it, so the loader
    /// leaves this `None` and the landing page degrades to a note rather than
    /// inventing a value (#32). Tracked upstream — see the loader's `load` doc.
    pub target_hardware: Option<String>,
    pub groups: Vec<GroupDoc>,
    /// Every enum type used in the project, by name, sorted and deduped.
    pub enums: Vec<EnumDoc>,
    /// The relationship graph: call/read/write/reference edges (#37).
    pub graph: ProjectGraph,
    /// Project-relative path of the `Project.m1prj` every project-sourced symbol
    /// is declared in (e.g. `Project.m1prj`), forward-slashed. Stored once on
    /// the model because the path is the same for every symbol; only the
    /// per-entity `def_line` varies. Combined with a `--source-base` and an
    /// entity's `def_line`, the renderers build a jump-to-declaration link
    /// (#57). `None` when the path could not be determined.
    pub m1prj_path: Option<String>,
}

/// Project-wide counts by kind, plus structural metrics, for the overview
/// landing page (#32). Every field is computed from the model — never
/// hardcoded — so the numbers stay in lock-step with the rendered pages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProjectStats {
    pub channels: usize,
    pub parameters: usize,
    pub constants: usize,
    pub functions: usize,
    pub tables: usize,
    pub objects: usize,
    pub can_messages: usize,
    pub can_signals: usize,
    pub enums: usize,
    /// Total documented group nodes (every node in the tree).
    pub groups: usize,
    /// Number of forest-root groups (top-level entries on the landing page).
    pub top_level_groups: usize,
    /// Deepest group path measured in dot-segments (`Root.A.B` → 3). Zero when
    /// the project has no groups.
    pub max_depth: usize,
}

impl ProjectStats {
    /// Total documented components across every kind — the headline number on
    /// the landing page.
    pub fn total_components(self) -> usize {
        self.channels
            + self.parameters
            + self.constants
            + self.functions
            + self.tables
            + self.objects
            + self.can_messages
            + self.can_signals
    }
}

/// One forest-root group on the landing tree, paired with component counts. The
/// leaf segment is the display label; the full path keys its page (#32).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TreeNode {
    pub path: String,
    /// Documented components declared **directly** on this node (not its
    /// descendants) — channels, parameters, constants, functions, tables,
    /// objects and CAN messages.
    pub direct_count: usize,
    /// Documented components across this node and its whole subtree — the
    /// figure shown beside a top-level group so its size reads at a glance
    /// (the issue's illustrative `Root.Engine (412)`).
    pub subtree_count: usize,
    /// Whether this node has any child groups (so the renderer can mark it as
    /// expandable without re-walking the tree).
    pub has_children: bool,
}

impl DocModel {
    /// Components declared directly on a group (the figure shown beside it in
    /// the landing tree and the nav).
    fn direct_count(g: &GroupDoc) -> usize {
        g.symbols.len()
            + g.functions.len()
            + g.tables.len()
            + g.objects.len()
            + g.can_messages.len()
    }

    /// Compute the project stats from the model. Counts every documented
    /// component, the group structure, and the enum reference. Deterministic:
    /// it only reads already-sorted model data (#32).
    pub fn stats(&self) -> ProjectStats {
        let mut s = ProjectStats {
            enums: self.enums.len(),
            groups: self.groups.len(),
            ..ProjectStats::default()
        };
        let present: std::collections::HashSet<&str> =
            self.groups.iter().map(|g| g.path.as_str()).collect();
        for g in &self.groups {
            for sym in &g.symbols {
                match sym.kind {
                    SymbolDocKind::Channel => s.channels += 1,
                    SymbolDocKind::Parameter => s.parameters += 1,
                    SymbolDocKind::Constant => s.constants += 1,
                }
            }
            s.functions += g.functions.len();
            s.tables += g.tables.len();
            s.objects += g.objects.len();
            s.can_messages += g.can_messages.len();
            s.can_signals += g
                .can_messages
                .iter()
                .map(|m| m.signals.len())
                .sum::<usize>();
            // A forest root is a node whose parent is not itself a documented
            // group — the same definition the index and nav use.
            let depth = g.path.split('.').count();
            s.max_depth = s.max_depth.max(depth);
            let parent = match g.path.rfind('.') {
                Some(i) => &g.path[..i],
                None => "",
            };
            if parent.is_empty() || !present.contains(parent) {
                s.top_level_groups += 1;
            }
        }
        s
    }

    /// The forest-root groups for the landing tree, in model (sorted) order,
    /// each with its direct and subtree component counts and whether it has
    /// children (#32).
    pub fn top_level_tree(&self) -> Vec<TreeNode> {
        let by_path: std::collections::BTreeMap<&str, &GroupDoc> =
            self.groups.iter().map(|g| (g.path.as_str(), g)).collect();
        self.groups
            .iter()
            .filter(|g| {
                let parent = match g.path.rfind('.') {
                    Some(i) => &g.path[..i],
                    None => "",
                };
                parent.is_empty() || !by_path.contains_key(parent)
            })
            .map(|g| TreeNode {
                path: g.path.clone(),
                direct_count: Self::direct_count(g),
                subtree_count: Self::subtree_count(g, &by_path),
                has_children: !g.children.is_empty(),
            })
            .collect()
    }

    /// Sum of [`Self::direct_count`] over a node and every descendant group,
    /// following the `children` links (the tree is acyclic, so this terminates).
    fn subtree_count(g: &GroupDoc, by_path: &std::collections::BTreeMap<&str, &GroupDoc>) -> usize {
        let mut total = Self::direct_count(g);
        for child in &g.children {
            if let Some(cg) = by_path.get(child.as_str()) {
                total += Self::subtree_count(cg, by_path);
            }
        }
        total
    }

    /// Every distinct security level the project declares, sorted, deduped.
    /// Drives the legend and the security filter (#34). Empty when no symbol
    /// declares a security level.
    pub fn security_levels(&self) -> Vec<String> {
        let mut set: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for g in &self.groups {
            for sym in &g.symbols {
                if let Some(sec) = &sym.security {
                    set.insert(sec.as_str());
                }
            }
        }
        set.into_iter().map(str::to_string).collect()
    }

    /// Every distinct tag any symbol carries, sorted, deduped. Drives the tag
    /// facet/index and the tag filter (#34). Empty when the project is
    /// untagged.
    pub fn tags(&self) -> Vec<String> {
        let mut set: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
        for g in &self.groups {
            for sym in &g.symbols {
                for tag in &sym.tags {
                    set.insert(tag.as_str());
                }
            }
        }
        set.into_iter().map(str::to_string).collect()
    }

    /// Scope the model in place to only the symbols matching `security` (any of
    /// the given access levels) and/or `tag` — the `--only-security` /
    /// `--only-tag` scoped-generation filter (#34). When both are given a symbol
    /// must satisfy both (intersection); `None` for either leaves that axis
    /// unconstrained. A scoped view is symbol-centric, so functions, tables,
    /// objects and CAN messages are dropped; groups with no surviving symbol are
    /// pruned, but every ancestor of a surviving group is kept so the tree stays
    /// navigable; child links and the enum reference list are rebuilt to match.
    /// Deterministic: ordering is preserved and pruning is set-membership only.
    pub fn retain_scoped(&mut self, security: Option<&[String]>, tag: Option<&str>) {
        use std::collections::BTreeSet;
        let keep_sym = |s: &SymbolDoc| -> bool {
            let sec_ok = match security {
                Some(levels) => s
                    .security
                    .as_deref()
                    .is_some_and(|sec| levels.iter().any(|l| l == sec)),
                None => true,
            };
            let tag_ok = match tag {
                Some(t) => s.tags.iter().any(|x| x == t),
                None => true,
            };
            sec_ok && tag_ok
        };
        // 1. Keep only matching symbols; a scoped view drops non-symbol entities.
        for g in &mut self.groups {
            g.symbols.retain(&keep_sym);
            g.functions.clear();
            g.tables.clear();
            g.objects.clear();
            g.can_messages.clear();
        }
        // 2. A group is kept if it has a surviving symbol, or is an ancestor of
        //    one (so the path to it stays navigable).
        let mut kept: BTreeSet<String> = self
            .groups
            .iter()
            .filter(|g| !g.symbols.is_empty())
            .map(|g| g.path.clone())
            .collect();
        for path in kept.iter().cloned().collect::<Vec<_>>() {
            let mut cur = path.as_str();
            while let Some(i) = cur.rfind('.') {
                cur = &cur[..i];
                kept.insert(cur.to_string());
            }
        }
        // 3. Drop pruned groups and rebuild child links + the enum reference.
        self.groups.retain(|g| kept.contains(&g.path));
        for g in &mut self.groups {
            g.children.retain(|c| kept.contains(c));
        }
        let used: BTreeSet<&str> = self
            .groups
            .iter()
            .flat_map(|g| g.symbols.iter())
            .filter_map(|s| s.enum_ref.as_deref())
            .collect();
        self.enums.retain(|e| used.contains(e.name.as_str()));
    }

    /// Every documented entity that carries a `<page>#<anchor>` deep link, in
    /// deterministic order: groups in model (sorted) order, members in their
    /// sorted model order within each group, CAN signals immediately after their
    /// message, then the project-wide enums on the shared `enums.html` page.
    ///
    /// This is the single traversal that defines the project's deep-link set.
    /// Both deep-link consumers in the HTML renderer — the search index and the
    /// relationship-graph node-href map — build on it and filter by
    /// [`AnchoredEntity::kind`], so they can never again diverge on *which*
    /// kinds carry a link or *how* that link is shaped (the link shape lives in
    /// [`AnchoredEntity::href`]). It covers all eight anchored kinds; each
    /// caller keeps only the kinds it wants.
    pub fn anchored_entities(&self) -> Vec<AnchoredEntity<'_>> {
        let mut out = Vec::new();
        for g in &self.groups {
            let page = format!("{}.html", g.path);
            for s in &g.symbols {
                // Prefer the display unit, fall back to the quantity, else empty
                // — the same precedence the search index has always shown.
                let hint = s
                    .unit
                    .as_deref()
                    .or(s.quantity.as_deref())
                    .unwrap_or_default();
                out.push(AnchoredEntity {
                    kind: AnchoredKind::for_symbol(s.kind),
                    path: &s.path,
                    page: page.clone(),
                    anchor: &s.anchor,
                    group: &g.path,
                    hint,
                });
            }
            for f in &g.functions {
                out.push(AnchoredEntity {
                    kind: AnchoredKind::Function,
                    path: &f.path,
                    page: page.clone(),
                    anchor: &f.anchor,
                    group: &g.path,
                    hint: f.return_type.as_deref().unwrap_or_default(),
                });
            }
            for t in &g.tables {
                out.push(AnchoredEntity {
                    kind: AnchoredKind::Table,
                    path: &t.path,
                    page: page.clone(),
                    anchor: &t.anchor,
                    group: &g.path,
                    hint: t.output_unit.as_deref().unwrap_or_default(),
                });
            }
            for o in &g.objects {
                out.push(AnchoredEntity {
                    kind: AnchoredKind::Object,
                    path: &o.path,
                    page: page.clone(),
                    anchor: &o.anchor,
                    group: &g.path,
                    hint: o.class.as_deref().unwrap_or_default(),
                });
            }
            for m in &g.can_messages {
                out.push(AnchoredEntity {
                    kind: AnchoredKind::CanMessage,
                    path: &m.path,
                    page: page.clone(),
                    anchor: &m.anchor,
                    group: &g.path,
                    hint: "",
                });
                for sig in &m.signals {
                    out.push(AnchoredEntity {
                        kind: AnchoredKind::CanSignal,
                        path: &sig.path,
                        page: page.clone(),
                        anchor: &sig.anchor,
                        group: &g.path,
                        hint: sig.unit.as_deref().unwrap_or_default(),
                    });
                }
            }
            for r in &g.references {
                out.push(AnchoredEntity {
                    kind: AnchoredKind::Reference,
                    path: &r.path,
                    page: page.clone(),
                    anchor: &r.anchor,
                    group: &g.path,
                    hint: "",
                });
            }
        }
        // Enums live on the shared reference page, not a group page.
        for e in &self.enums {
            out.push(AnchoredEntity {
                kind: AnchoredKind::Enum,
                path: &e.name,
                page: "enums.html".to_string(),
                anchor: &e.anchor,
                group: "Enums",
                hint: "",
            });
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_plurals_are_section_headings() {
        assert_eq!(SymbolDocKind::Channel.plural(), "Channels");
        assert_eq!(SymbolDocKind::Parameter.plural(), "Parameters");
        assert_eq!(SymbolDocKind::Constant.plural(), "Constants");
    }

    #[test]
    fn annotation_doc_defaults_to_empty() {
        let a = AnnotationDoc::default();
        assert!(a.kind.is_empty());
        assert!(a.args.is_empty());
    }

    #[test]
    fn function_doc_defaults_to_no_annotations() {
        let f = FunctionDoc::default();
        assert!(f.annotations.is_empty());
    }

    #[test]
    fn anchor_slug_is_lowercase_hyphenated_and_collapsed() {
        assert_eq!(anchor_slug("Root.Engine.Speed"), "root-engine-speed");
        assert_eq!(anchor_slug("Root.Engine.On 100Hz"), "root-engine-on-100hz");
        // Leading/trailing/repeated separators collapse; no edge hyphens.
        assert_eq!(anchor_slug("Root..Engine -- X!"), "root-engine-x");
        // No alphanumerics at all → a stable fallback, never an empty id.
        assert_eq!(anchor_slug(""), "symbol");
        assert_eq!(anchor_slug("...!!!"), "symbol");
    }

    #[test]
    fn annotation_doc_stores_kind_and_args() {
        let a = AnnotationDoc {
            kind: "requires-finite".into(),
            args: vec!["min=0".into()],
        };
        assert_eq!(a.kind, "requires-finite");
        assert_eq!(a.args, vec!["min=0"]);
    }

    // ---- #32 / #34: landing stats, tree, security/tag facets ----

    /// A two-level model with mixed kinds: stats must count every component and
    /// the structural metrics from the model alone (#32).
    fn stats_model() -> DocModel {
        DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![EnumDoc {
                name: "Switch".into(),
                anchor: "switch".into(),
                ..Default::default()
            }],
            groups: vec![
                GroupDoc {
                    path: "Root".into(),
                    children: vec!["Root.Engine".into()],
                    ..Default::default()
                },
                GroupDoc {
                    path: "Root.Engine".into(),
                    symbols: vec![
                        SymbolDoc {
                            path: "Root.Engine.Speed".into(),
                            kind: SymbolDocKind::Channel,
                            security: Some("Tune".into()),
                            tags: vec!["engine".into()],
                            ..Default::default()
                        },
                        SymbolDoc {
                            path: "Root.Engine.Gain".into(),
                            kind: SymbolDocKind::Parameter,
                            security: Some("Calibration".into()),
                            tags: vec!["engine".into(), "fuel".into()],
                            ..Default::default()
                        },
                    ],
                    functions: vec![FunctionDoc {
                        path: "Root.Engine.Update".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                },
            ],
            graph: crate::model::ProjectGraph::default(),
            m1prj_path: None,
        }
    }

    #[test]
    fn stats_count_every_kind_and_structure() {
        let s = stats_model().stats();
        assert_eq!(s.channels, 1);
        assert_eq!(s.parameters, 1);
        assert_eq!(s.functions, 1);
        assert_eq!(s.enums, 1);
        assert_eq!(s.groups, 2);
        assert_eq!(s.top_level_groups, 1, "only Root is a forest root");
        assert_eq!(s.max_depth, 2, "Root.Engine is two segments deep");
        assert_eq!(s.total_components(), 3, "1 channel + 1 param + 1 function");
    }

    #[test]
    fn top_level_tree_lists_forest_roots_with_direct_counts() {
        let tree = stats_model().top_level_tree();
        assert_eq!(tree.len(), 1, "only Root is top-level");
        assert_eq!(tree[0].path, "Root");
        // Root itself carries no direct members; its children hold them.
        assert_eq!(tree[0].direct_count, 0);
        // The subtree rolls up the 3 components under Root.Engine.
        assert_eq!(tree[0].subtree_count, 3);
        assert!(tree[0].has_children, "Root has the Engine child group");
    }

    #[test]
    fn security_levels_are_sorted_deduped() {
        let levels = stats_model().security_levels();
        assert_eq!(levels, vec!["Calibration".to_string(), "Tune".to_string()]);
    }

    #[test]
    fn tags_are_sorted_deduped_across_symbols() {
        let tags = stats_model().tags();
        assert_eq!(tags, vec!["engine".to_string(), "fuel".to_string()]);
    }

    // ---- #34: --only-security / --only-tag scoped generation ----

    #[test]
    fn retain_scoped_by_security_keeps_only_matching_symbols() {
        let mut m = stats_model();
        m.retain_scoped(Some(&["Tune".to_string()]), None);
        // Root (ancestor) + Root.Engine (has the Tune symbol) survive.
        assert_eq!(
            m.groups.iter().map(|g| g.path.as_str()).collect::<Vec<_>>(),
            vec!["Root", "Root.Engine"]
        );
        let eng = m.groups.iter().find(|g| g.path == "Root.Engine").unwrap();
        assert_eq!(eng.symbols.len(), 1);
        assert_eq!(eng.symbols[0].path, "Root.Engine.Speed");
        // A scoped view drops non-symbol entities.
        assert!(
            eng.functions.is_empty(),
            "functions dropped in a scoped view"
        );
        // The ancestor still links its kept child.
        let root = m.groups.iter().find(|g| g.path == "Root").unwrap();
        assert_eq!(root.children, vec!["Root.Engine".to_string()]);
        // The Calibration-only enum reference set is rebuilt (Switch unused here).
        assert!(m.enums.is_empty());
    }

    #[test]
    fn retain_scoped_by_tag_keeps_only_tagged_symbols() {
        let mut m = stats_model();
        m.retain_scoped(None, Some("fuel"));
        let eng = m.groups.iter().find(|g| g.path == "Root.Engine").unwrap();
        assert_eq!(eng.symbols.len(), 1);
        assert_eq!(
            eng.symbols[0].path, "Root.Engine.Gain",
            "only the fuel-tagged symbol"
        );
    }

    #[test]
    fn retain_scoped_intersects_security_and_tag_and_prunes_empty() {
        let mut m = stats_model();
        // Tune AND fuel: Speed is Tune-but-not-fuel, Gain is fuel-but-Calibration
        // → nothing matches → every group is pruned.
        m.retain_scoped(Some(&["Tune".to_string()]), Some("fuel"));
        assert!(m.groups.is_empty(), "no symbol matches both → empty model");
    }

    #[test]
    fn retain_scoped_multiple_security_levels() {
        let mut m = stats_model();
        m.retain_scoped(Some(&["Tune".to_string(), "Calibration".to_string()]), None);
        let eng = m.groups.iter().find(|g| g.path == "Root.Engine").unwrap();
        assert_eq!(
            eng.symbols.len(),
            2,
            "both Tune and Calibration symbols kept"
        );
    }

    #[test]
    fn empty_model_has_zeroed_stats_and_no_facets() {
        let m = DocModel::default();
        let s = m.stats();
        assert_eq!(s.total_components(), 0);
        assert_eq!(s.max_depth, 0);
        assert!(m.security_levels().is_empty());
        assert!(m.tags().is_empty());
        assert!(m.top_level_tree().is_empty());
    }

    // ---- the single anchored-entity traversal the deep-link consumers share ----

    /// A model holding one of every anchored kind, so a single walk can assert
    /// the shared iterator covers them all (the historical drift: the search
    /// index and the graph node-href map walked different subsets by hand).
    fn one_of_every_anchored_kind() -> DocModel {
        DocModel {
            title: "Demo".into(),
            target_hardware: None,
            enums: vec![EnumDoc {
                name: "Switch".into(),
                anchor: "switch".into(),
                ..Default::default()
            }],
            groups: vec![GroupDoc {
                path: "Root.Engine".into(),
                symbols: vec![SymbolDoc {
                    path: "Root.Engine.Speed".into(),
                    anchor: "root-engine-speed".into(),
                    ..Default::default()
                }],
                functions: vec![FunctionDoc {
                    path: "Root.Engine.Update".into(),
                    anchor: "root-engine-update".into(),
                    ..Default::default()
                }],
                tables: vec![TableDoc {
                    path: "Root.Engine.Map".into(),
                    anchor: "root-engine-map".into(),
                    ..Default::default()
                }],
                objects: vec![ObjectDoc {
                    path: "Root.Engine.Sensor".into(),
                    anchor: "root-engine-sensor".into(),
                    ..Default::default()
                }],
                can_messages: vec![CanMessageDoc {
                    path: "Root.Engine.Frame".into(),
                    anchor: "root-engine-frame".into(),
                    signals: vec![CanSignalDoc {
                        path: "Root.Engine.Frame.Rpm".into(),
                        anchor: "root-engine-frame-rpm".into(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                references: vec![ReferenceDoc {
                    path: "Root.Engine.Alias".into(),
                    anchor: "root-engine-alias".into(),
                    ..Default::default()
                }],
                ..Default::default()
            }],
            graph: ProjectGraph::default(),
            m1prj_path: None,
        }
    }

    #[test]
    fn anchored_entities_cover_all_eight_kinds_with_canonical_hrefs() {
        let m = one_of_every_anchored_kind();
        let ents = m.anchored_entities();

        // Every anchored kind is present exactly once — the search index and the
        // graph node-href map both build on this, so none can be silently missed.
        // The lone symbol defaults to Channel.
        use std::collections::BTreeSet;
        let kinds: BTreeSet<_> = ents.iter().map(|e| e.kind.label().to_string()).collect();
        assert_eq!(
            kinds,
            BTreeSet::from([
                "channel".to_string(),
                "function".to_string(),
                "table".to_string(),
                "object".to_string(),
                "CAN message".to_string(),
                "CAN signal".to_string(),
                "reference".to_string(),
                "enum".to_string(),
            ]),
            "anchored_entities must cover all eight anchored kinds"
        );

        // Group-page entities deep-link to <group>.html#<anchor>; the enum
        // deep-links to the shared enums.html page. href() is the one place the
        // link shape is built, so both consumers agree by construction.
        let by_path: std::collections::HashMap<&str, String> =
            ents.iter().map(|e| (e.path, e.href())).collect();
        assert_eq!(
            by_path["Root.Engine.Speed"],
            "Root.Engine.html#root-engine-speed"
        );
        assert_eq!(
            by_path["Root.Engine.Frame.Rpm"],
            "Root.Engine.html#root-engine-frame-rpm"
        );
        assert_eq!(by_path["Switch"], "enums.html#switch");

        // The hint travels with the entity so the search index need not re-walk
        // the model to recover it (here every fixture entity leaves it empty).
        assert!(ents.iter().all(|e| e.hint.is_empty()));
    }

    #[test]
    fn anchored_entities_signal_follows_its_message() {
        let m = one_of_every_anchored_kind();
        let ents = m.anchored_entities();
        let msg = ents
            .iter()
            .position(|e| e.kind == AnchoredKind::CanMessage)
            .unwrap();
        let sig = ents
            .iter()
            .position(|e| e.kind == AnchoredKind::CanSignal)
            .unwrap();
        assert_eq!(sig, msg + 1, "a signal is yielded right after its message");
    }
}
