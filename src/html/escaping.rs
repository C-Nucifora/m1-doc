//! HTML escaping for the hand-built markup (nav, figures, filter panel). Text
//! and attribute contexts differ only by whether `"` is escaped, so both go
//! through a single [`html_escape_into`] and can never drift apart on the common
//! `& < >` set.

/// Append `s` to `out` with HTML escaping. `& < >` are always escaped (the set
/// that can inject markup in any context); when `attr` is set the
/// attribute-delimiting `"` is additionally escaped so the result is safe inside
/// a double-quoted attribute value.
///
/// This is the single implementation behind both [`html_escape`] (text context)
/// and [`attr_escape`] (attribute context) — the two contexts differ only by the
/// `"` flag, so they can never drift apart on the common `& < >` set the way two
/// hand-rolled bodies could (mirrors the JSON escaper consolidation in
/// [`crate::escape`]).
///
/// Note: spaces are deliberately left verbatim. The attribute callers escape
/// group-path hrefs, which must keep spaces literal to match the on-disk page
/// filename (`<group path>.html`).
pub(super) fn html_escape_into(out: &mut String, s: &str, attr: bool) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
}

/// Minimal HTML-text escaping for figure titles. Escapes `& < >`, leaving `"`
/// (and spaces) verbatim — for text contexts, not attribute values.
pub(super) fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    html_escape_into(&mut out, s, false);
    out
}

/// Escape a string for use inside a double-quoted HTML attribute. Escapes the
/// markup-significant `& < >` plus the attribute-delimiting `"`; spaces are left
/// verbatim so an href matches the on-disk page filename. Thin wrapper over
/// [`html_escape_into`] with `attr = true`.
pub(super) fn attr_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    html_escape_into(&mut out, s, true);
    out
}
