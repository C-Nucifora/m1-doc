//! The client-side search index (#31): a flat JSON array over every documented
//! entity, emitted once per project as a shared `search-index.js` sidecar that
//! every page loads via `<script src>`. Built from the model's single
//! [`DocModel::anchored_entities`] walk so it can never cover a different subset
//! of entities than the relationship-graph deep links (see [`super::graph`]).

use crate::model::DocModel;

/// Filename of the shared search-index sidecar (#31). One per project, loaded by
/// every page via a plain `<script src>` — which works from `file://`, keeping
/// the self-contained guarantee — instead of inlining the whole index into each
/// page (that was O(pages × project): 274 MB on EV-M1).
pub(super) const SEARCH_INDEX_FILE: &str = "search-index.js";

/// One search record. `p` = full path, `k` = kind label, `g` = owning group,
/// `u` = unit/quantity hint (may be empty), `h` = `<group>.html#<anchor>` href.
/// Short field names keep the inline JSON compact on large projects.
pub(super) struct SearchEntry {
    pub(super) path: String,
    kind: &'static str,
    group: String,
    hint: String,
    pub(super) href: String,
}

/// Minimal JSON-string escaping for the inline index: returns the *unquoted*
/// inner content (callers supply the surrounding `"`). Uses the shared
/// script-safe escaper so `<`/`>`/`/` can never form a literal `</script>` that
/// would close the embedding element early. See
/// [`crate::escape::escape_json_into`].
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    crate::escape::escape_json_into(&mut out, s, true);
    out
}

/// Collect every documented entity into search records, in deterministic order.
///
/// Built from the model's single [`DocModel::anchored_entities`] walk — the same
/// traversal the relationship-graph node-href map uses — so the search index and
/// the graph deep links can never again cover different subsets of the anchored
/// kinds. The search index keeps every kind (it indexes enums too); the graph
/// map drops enums. See `node_hrefs`.
pub(super) fn build_search_entries(model: &DocModel) -> Vec<SearchEntry> {
    model
        .anchored_entities()
        .into_iter()
        .map(|e| SearchEntry {
            path: e.path.to_string(),
            kind: e.kind.label(),
            group: e.group.to_string(),
            hint: e.hint.to_string(),
            href: e.href(),
        })
        .collect()
}

/// Render the search index as a compact inline JSON array. Deterministic order.
pub(super) fn search_index_json(model: &DocModel) -> String {
    let mut json = String::from("[");
    for (i, e) in build_search_entries(model).iter().enumerate() {
        if i > 0 {
            json.push(',');
        }
        json.push_str(&format!(
            "{{\"p\":\"{}\",\"k\":\"{}\",\"g\":\"{}\",\"u\":\"{}\",\"h\":\"{}\"}}",
            json_escape(&e.path),
            json_escape(e.kind),
            json_escape(&e.group),
            json_escape(&e.hint),
            json_escape(&e.href),
        ));
    }
    json.push(']');
    json
}

/// The `<script src>` reference every page uses to load the shared index. It
/// must precede the behaviour script, which reads `window.__M1_SEARCH_INDEX__`.
pub(super) fn search_index_ref() -> String {
    format!("<script src=\"{SEARCH_INDEX_FILE}\"></script>")
}

/// The body of the shared [`SEARCH_INDEX_FILE`]: assigns the index array to a
/// global the behaviour script reads. Emitted once for the whole project.
pub(super) fn build_search_index_file(model: &DocModel) -> String {
    format!("window.__M1_SEARCH_INDEX__={};\n", search_index_json(model))
}
