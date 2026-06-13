//! The in-memory documentation model — the single source of truth that the
//! renderers read. No m1-core / m1-typecheck types leak past this boundary.

/// One documented symbol (channel / parameter / constant).
#[derive(Debug, Clone, PartialEq)]
pub struct SymbolDoc {
    pub path: String,
    pub kind: SymbolDocKind,
    /// The storage type to show: `declared_type` verbatim when present, else the
    /// resolved value type's display string. Always present — every symbol has at
    /// least a resolved `ValueType`.
    pub type_label: String,
    pub unit: Option<String>,
    pub security: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SymbolDocKind {
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
}
