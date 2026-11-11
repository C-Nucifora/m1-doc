//! Rewrite Markdown link destinations before HTML rendering, leaving source and
//! metadata text untouched.

use pulldown_cmark::{Event, Tag};

/// Relative documentation links use the corresponding HTML page, retaining any
/// query or fragment. External URL schemes and protocol-relative URLs stay as is.
pub(super) fn rewrite_md_link(mut event: Event<'_>) -> Event<'_> {
    if let Event::Start(Tag::Link { dest_url, .. }) = &mut event {
        let href = dest_url.as_ref();
        let split = href.find(['#', '?']).unwrap_or(href.len());
        let (path, suffix) = href.split_at(split);
        if !path.contains(':')
            && !path.starts_with("//")
            && let Some(stem) = path.strip_suffix(".md")
        {
            *dest_url = format!("{stem}.html{suffix}").into();
        }
    }
    event
}

#[cfg(test)]
mod tests {
    use super::*;
    use pulldown_cmark::{Parser, html::push_html};

    fn render(md: &str) -> String {
        let mut html = String::new();
        push_html(&mut html, Parser::new(md).map(rewrite_md_link));
        html
    }

    #[test]
    fn only_relative_link_paths_change_and_suffixes_are_preserved() {
        for (input, expected) in [
            ("Root.Engine.md", "Root.Engine.html"),
            ("Root.Engine.md#section", "Root.Engine.html#section"),
            ("Root.Engine.md?v=1", "Root.Engine.html?v=1"),
            (
                "Fuel%23%3F%2520.md?q=1#part",
                "Fuel%23%3F%2520.html?q=1#part",
            ),
            ("https://example.com/doc.md", "https://example.com/doc.md"),
            ("HTTP://example.com/doc.md", "HTTP://example.com/doc.md"),
            ("//example.com/doc.md", "//example.com/doc.md"),
            ("ftp://example.com/doc.md", "ftp://example.com/doc.md"),
            ("#section.md", "#section.md"),
        ] {
            assert_eq!(
                render(&format!("[link]({input})")),
                format!("<p><a href=\"{expected}\">link</a></p>\n")
            );
        }
    }

    #[test]
    fn link_shaped_metadata_and_source_are_preserved() {
        let md = "href=\"Root.md\"\n\n`href=\"Root.md\"`\n\n```m1\n// href=\"Root.md\"\n```\n\n[real link](Root.md)\n";
        assert_eq!(
            render(md),
            "<p>href=\"Root.md\"</p>\n<p><code>href=\"Root.md\"</code></p>\n<pre><code class=\"language-m1\">// href=\"Root.md\"\n</code></pre>\n<p><a href=\"Root.html\">real link</a></p>\n"
        );
    }
}
