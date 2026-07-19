//! Intra-doc link rewriting: relative `*.md` hrefs in the pulldown-cmark output
//! become `*.html` so links between pages resolve in the HTML site. External
//! `http(s)://` links are left untouched.

/// Rewrite relative `*.md` hrefs to `*.html`.  Operates on the raw HTML
/// string produced by pulldown-cmark.  Only touches `href="…"` attributes
/// whose values end with `.md` and do **not** start with `http://` or
/// `https://`.
pub(super) fn rewrite_md_links(html: &str) -> String {
    // We scan byte-by-byte for the pattern  href="…"  to keep the
    // implementation simple and dependency-free.
    let needle = "href=\"";
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(pos) = rest.find(needle) {
        // Emit everything up to and including `href="`
        out.push_str(&rest[..pos + needle.len()]);
        rest = &rest[pos + needle.len()..];
        // Find the closing quote.
        if let Some(end) = rest.find('"') {
            let href = &rest[..end];
            if !href.starts_with("http://") && !href.starts_with("https://") {
                // Split off any fragment (#…) or query (?…) that follows the
                // path component so we can check the path extension alone.
                let (path, suffix) = if let Some(i) = href.find(['#', '?']) {
                    (&href[..i], &href[i..])
                } else {
                    (href, "")
                };
                if let Some(stem) = path.strip_suffix(".md") {
                    // Replace the trailing `.md` with `.html`, then reattach
                    // the fragment/query string unchanged.
                    out.push_str(stem);
                    out.push_str(".html");
                    out.push_str(suffix);
                } else {
                    out.push_str(href);
                }
            } else {
                out.push_str(href);
            }
            out.push('"');
            rest = &rest[end + 1..];
        }
        // If no closing quote found the rest of the string is copied below.
    }
    out.push_str(rest);
    out
}
