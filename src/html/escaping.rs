//! HTML escaping for the hand-built markup (nav, figures, filter panel). Text
//! and attribute contexts differ only by whether `"` is escaped, so both go
//! through a single [`html_escape_into`] and can never drift apart on the common
//! `& < >` set.

pub(super) use crate::escape::html_escape_into;

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
