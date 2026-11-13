//! Context-specific escaping shared by the output renderers.

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

/// Percent-encode a project-relative path for use as a URL target, preserving
/// the `/` segment separators. M1 object names may contain spaces (Development
/// Manual, *Naming Objects*: "Space may be used between two name constituents"),
/// so the on-disk path — and thus a `{base}/{path}` link — can contain spaces
/// and other URL-unsafe characters that browsers/Markdown renderers truncate or
/// mangle. Only the URL target is encoded; the visible link *text* keeps the
/// raw, human-readable path.
///
/// Each `/`-delimited segment is encoded independently: every byte outside the
/// unreserved set (`A–Z a–z 0–9 - . _ ~`) becomes `%XX`. The `/` separators are
/// emitted verbatim so the path structure (and GitHub line anchors) survive.
pub(crate) fn url_encode_path(path: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(path.len());
    for (i, segment) in path.split('/').enumerate() {
        if i > 0 {
            out.push('/');
        }
        for &byte in segment.as_bytes() {
            match byte {
                b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                    out.push(byte as char);
                }
                _ => {
                    out.push('%');
                    out.push(HEX[(byte >> 4) as usize] as char);
                    out.push(HEX[(byte & 0xf) as usize] as char);
                }
            }
        }
    }
    out
}

/// HTML text and quoted attributes; attribute escapes also keep Markdown tables intact.
pub(crate) fn html_escape_into(out: &mut String, s: &str, attr: bool) {
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' if attr => out.push_str("&quot;"),
            '|' if attr => out.push_str("&#124;"),
            '\r' if attr => out.push_str("&#13;"),
            '\n' if attr => out.push_str("&#10;"),
            '\t' if attr => out.push_str("&#9;"),
            c => out.push(c),
        }
    }
}

pub(crate) fn html_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    html_escape_into(&mut out, s, false);
    out
}

pub(crate) fn attr_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    html_escape_into(&mut out, s, true);
    out
}

/// Metadata occupies one inline field; preserve its raw form in the model/JSON.
fn single_line(s: &str) -> String {
    s.replace("\r\n", " ").replace(['\r', '\n'], " ")
}

/// Literal Markdown text, usable in headings, link labels and table cells.
pub(crate) fn md_text(s: &str) -> String {
    let s = single_line(s).replace('\t', " ");
    let leading = s.len() - s.trim_start_matches(' ').len();
    let trailing = s.trim_end_matches(' ').len();
    let mut out = String::with_capacity(s.len());
    for (i, ch) in s.char_indices() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            ' ' if i < leading || i >= trailing => out.push_str("&#32;"),
            '\\' | '`' | '*' | '_' | '[' | ']' | '!' | '|' | '#' | '~' => {
                out.push('\\');
                out.push(ch);
            }
            '-' | '+' if i == 0 => {
                out.push('\\');
                out.push(ch);
            }
            '.' | ')' if i > 0 && s[..i].bytes().all(|b| b.is_ascii_digit()) => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out
}

fn longest_ticks(s: &str) -> usize {
    s.bytes()
        .fold((0, 0), |(run, longest), byte| {
            if byte == b'`' {
                (run + 1, longest.max(run + 1))
            } else {
                (0, longest)
            }
        })
        .1
}

/// Code spans with data-sized delimiters; pipes need HTML to survive GFM tables.
pub(crate) fn md_code(s: &str) -> String {
    let s = single_line(s);
    if s.is_empty() || s.contains('|') {
        let mut out = String::from("<code>");
        for ch in s.chars() {
            let _ = write!(out, "&#{};", ch as u32);
        }
        out.push_str("</code>");
        return out;
    }
    let ticks = "`".repeat(longest_ticks(&s) + 1);
    let pad = s.starts_with('`')
        || s.ends_with('`')
        || s.starts_with(' ') && s.ends_with(' ') && !s.chars().all(|c| c == ' ');
    if pad {
        format!("{ticks} {s} {ticks}")
    } else {
        format!("{ticks}{s}{ticks}")
    }
}

pub(crate) fn code_fence(s: &str) -> String {
    "`".repeat(longest_ticks(s).max(2) + 1)
}

/// Protect a supplied URL's Markdown destination without encoding its URL syntax.
pub(crate) fn md_destination(url: &str) -> String {
    let mut out = String::with_capacity(url.len());
    for byte in url.bytes() {
        match byte {
            b'&' => out.push_str("&amp;"),
            0..=32
            | 127..=255
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'"'
            | b'\\'
            | b'`'
            | b'|'
            | b'['
            | b']' => {
                let _ = write!(out, "%{byte:02X}");
            }
            _ => out.push(byte as char),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_metadata_round_trips_without_markup_or_extra_cells() {
        use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};
        let pieces = [
            "",
            " ",
            "  ",
            "`",
            "``",
            "|",
            "\\",
            "<img src=x>",
            "&lt;",
            "[x](https://invalid)",
            "![x](x)",
            "*x*",
            "# h",
            "1. x",
            "- x",
            "x\r\ny",
            "x\ny",
            "\t",
            "é🦀",
            "~x~",
            "a  b",
        ];
        for left in pieces {
            for right in pieces {
                let raw = format!("{left}{right}");
                for code in [false, true] {
                    let escaped = if code { md_code(&raw) } else { md_text(&raw) };
                    let md = format!("| Field | Other |\n| --- | --- |\n| {escaped} | keep |\n");
                    let mut cells = Vec::new();
                    let mut text = String::new();
                    for event in Parser::new_ext(&md, Options::ENABLE_TABLES) {
                        match event {
                            Event::Start(Tag::TableCell) => text.clear(),
                            Event::End(TagEnd::TableCell) => cells.push(text.clone()),
                            Event::Text(s) | Event::Code(s) => text.push_str(&s),
                            Event::InlineHtml(s)
                                if code && matches!(s.as_ref(), "<code>" | "</code>") => {}
                            Event::Start(Tag::Table(_) | Tag::TableHead | Tag::TableRow)
                            | Event::End(TagEnd::Table | TagEnd::TableHead | TagEnd::TableRow) => {}
                            other => panic!("unexpected {other:?} for {raw:?}: {md}"),
                        }
                    }
                    let expected = single_line(&raw);
                    let expected = if code {
                        expected
                    } else {
                        expected.replace('\t', " ")
                    };
                    assert_eq!(
                        cells,
                        ["Field", "Other", &expected, "keep"],
                        "code={code} input={raw:?}: {md}"
                    );
                }
            }
        }
    }

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
