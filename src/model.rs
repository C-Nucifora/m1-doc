//! The in-memory documentation model — the single source of truth that the
//! renderers read. No m1-core / m1-typecheck types leak past this boundary.

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
    pub inputs: Vec<(String, String)>,
    pub return_type: Option<String>,
    pub annotations: Vec<AnnotationDoc>,
    /// Execution rate in Hz, derived from the script's `SelectedTrigger`
    /// (e.g. a `100 Hz` event). `None` when the function has no rate-bearing
    /// trigger (`On Startup`, untriggered) — rendered as `—`, never faked.
    pub call_rate_hz: Option<f64>,
}

/// One documented symbol (channel / parameter / constant).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct SymbolDoc {
    pub path: String,
    pub kind: SymbolDocKind,
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

/// One top-level group page (e.g. `Root.Engine`) and the symbols under it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct GroupDoc {
    pub path: String,
    pub symbols: Vec<SymbolDoc>,
    /// Functions and methods declared in this group, sorted by path.
    pub functions: Vec<FunctionDoc>,
}

/// The whole project's documentation, groups sorted by path.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct DocModel {
    pub title: String,
    pub groups: Vec<GroupDoc>,
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
    fn annotation_doc_stores_kind_and_args() {
        let a = AnnotationDoc {
            kind: "requires-finite".into(),
            args: vec!["min=0".into()],
        };
        assert_eq!(a.kind, "requires-finite");
        assert_eq!(a.args, vec!["min=0"]);
    }
}
