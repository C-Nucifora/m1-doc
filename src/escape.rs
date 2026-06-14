//! A single JSON-string escaper shared by every emitter in the crate.
//!
//! Three near-identical hand-rolled escapers used to live in `diagram.rs`,
//! `html.rs` and `json.rs`. They all escaped the same JSON control set; the two
//! that emit JSON inside an inline `<script>` additionally hardened `<`/`>`/`/`
//! so the payload can never close the embedding element early. Keeping three
//! copies meant a hardening fix to one could silently drift from the others, so
//! they are consolidated here.
//!
//! [`escape_json_into`] is the one implementation. `script_safe` selects whether
//! the script-close hardening is applied:
//!
//! * `script_safe == false` — a spec-conformant JSON string literal (used by
//!   the standalone `index.json` writer).
//! * `script_safe == true` — additionally escapes `<` `>` `/` so the result is
//!   safe to embed verbatim inside an inline `<script>` block.

use std::fmt::Write as _;

/// Append `s` to `out` as a JSON string literal *without* the surrounding
/// quotes — callers that need the quotes push them themselves (see
/// [`escape_json_quoted`]).
///
/// Always escapes the JSON control set (`"` `\` `\n` `\r` `\t` `\b` `\f`) and
/// emits `\uXXXX` for the remaining C0 control characters, so the output is
/// always valid JSON.
///
/// When `script_safe` is set, `<` `>` `/` are additionally escaped (as
/// `<` `>` `\/`) so the string can never contain a literal
/// `</script>` that would close an embedding `<script>` element early.
pub(crate) fn escape_json_into(out: &mut String, s: &str, script_safe: bool) {
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            '<' if script_safe => out.push_str("\\u003c"),
            '>' if script_safe => out.push_str("\\u003e"),
            '/' if script_safe => out.push_str("\\/"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
}

/// Append `s` to `out` as a quoted JSON string literal (with the surrounding
/// `"`). See [`escape_json_into`] for the `script_safe` flag.
pub(crate) fn escape_json_quoted(out: &mut String, s: &str, script_safe: bool) {
    out.push('"');
    escape_json_into(out, s, script_safe);
    out.push('"');
}

/// Convenience wrapper returning a freshly-allocated quoted JSON string literal.
pub(crate) fn escape_json_string(s: &str, script_safe: bool) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    escape_json_quoted(&mut out, s, script_safe);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_escapes_control_set_but_not_script_chars() {
        // script_safe == false: canonical JSON, no script hardening.
        assert_eq!(escape_json_string("a\"b\\c", false), "\"a\\\"b\\\\c\"");
        assert_eq!(escape_json_string("\n\r\t", false), "\"\\n\\r\\t\"");
        assert_eq!(escape_json_string("\u{08}\u{0C}", false), "\"\\b\\f\"");
        // C0 controls become \uXXXX.
        assert_eq!(escape_json_string("\u{01}", false), "\"\\u0001\"");
        // `<` `>` `/` are left verbatim when not script-hardening.
        assert_eq!(escape_json_string("</x>", false), "\"</x>\"");
    }

    #[test]
    fn script_safe_hardens_angle_brackets_and_slash() {
        // Both `<` and `>` are escaped, plus `/`, so `</script>` can't appear.
        assert_eq!(escape_json_string("<x>", true), "\"\\u003cx\\u003e\"");
        assert_eq!(escape_json_string("a/b", true), "\"a\\/b\"");
        assert!(!escape_json_string("</script>", true).contains("</"));
        // The control set is still escaped in script_safe mode.
        assert!(escape_json_string("a\nb", true).contains("\\n"));
        assert_eq!(escape_json_string("\u{08}", true), "\"\\b\"");
    }

    #[test]
    fn into_appends_without_quotes() {
        let mut out = String::from("x");
        escape_json_into(&mut out, "a\nb", false);
        assert_eq!(out, "xa\\nb");
    }
}
