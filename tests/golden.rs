//! Golden-output snapshot tests over a small, committed synthetic project
//! fixture (`tests/fixtures/synthetic/Project.m1prj`). The fixture deliberately
//! exercises one of each content type the renderers know about — nested groups
//! four deep (`Root.Engine.Fuel.Pump`), a channel with a quantity/unit/security,
//! an enum-typed channel and its Enums-page reference, a constant, a calibration
//! table, and a package object — so a change to any renderer surfaces as a diff
//! in this file's expected strings during review (#36).
//!
//! Unlike the corpus smoke test (which asserts *invariants* so it survives benign
//! output changes), these are exact byte goldens. Keep the fixture small. If a
//! deliberate output change lands, regenerate the expected blocks by running the
//! binary over the fixture and pasting the new bytes:
//!
//! ```sh
//! cargo run -- --project tests/fixtures/synthetic/Project.m1prj \
//!     --out /tmp/golden --format markdown --title "Synth Fixture"
//! cat /tmp/golden/Root.Engine.md   # etc.
//! ```

use assert_cmd::Command;

/// A stable index title passed explicitly so the golden does not depend on the
/// fixture's parent directory name (the CLI's default title source).
const TITLE: &str = "Synth Fixture";

/// Render the committed fixture to Markdown in a fresh tempdir and return its
/// path. The fixture is read-only; output goes to the tempdir only.
fn render_fixture() -> tempfile::TempDir {
    let out = tempfile::tempdir().unwrap();
    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            "tests/fixtures/synthetic/Project.m1prj",
            "--out",
            out.path().to_str().unwrap(),
            "--format",
            "markdown",
            "--title",
            TITLE,
        ])
        .assert()
        .success();
    out
}

fn read(out: &tempfile::TempDir, name: &str) -> String {
    std::fs::read_to_string(out.path().join(name)).unwrap_or_else(|e| panic!("reading {name}: {e}"))
}

#[test]
fn golden_index() {
    let out = render_fixture();
    let expected = "# Synth Fixture\n\
\n\
**Target hardware:** — *(not exposed by the project API yet)*\n\
\n\
6 components · 3 channels · 1 constant · 1 table · 1 object · 1 enum · 1 top-level group\n\
\n\
## Structure\n\
\n\
- [Root](Root.md) (6) ▸\n\
\n\
## Security levels\n\
\n\
Access level required to view or calibrate a value. Levels present in this project:\n\
\n\
- **Tune** — tunable at the Tune access level\n\
\n\
## Reference\n\
\n\
- [Enums](enums.md)\n";
    assert_eq!(read(&out, "index.md"), expected);
}

#[test]
fn golden_representative_group_page() {
    // `Root.Engine` is the richest page: a sub-group link, a Channels table with
    // a quantity/unit/security row and an enum-typed row that links to the Enums
    // reference, a Constants table, and a Tables section for a cfg-less table
    // (rendered as the explicit "requires a loaded `.m1cfg`" note, never faked).
    let out = render_fixture();
    let expected = "[Root](Root.md) › Engine\n\
\n\
# Root.Engine\n\
\n\
## Sub-groups\n\
\n\
- [Fuel](Root.Engine.Fuel.md)\n\
\n\
## Channels\n\
\n\
| Name | Type | Quantity | Unit | Base | Log rate | Security |\n\
| --- | --- | --- | --- | --- | --- | --- |\n\
| <a id=\"root-engine-speed\" class=\"m1-row-anchor\" data-security=\"Tune\"></a>`Root.Engine.Speed` | f32 | AngularVelocity | rpm | AngularVelocity | — | Tune |\n\
| <a id=\"root-engine-state\" class=\"m1-row-anchor\"></a>`Root.Engine.State` | [::This.Drive State](enums.md#drive-state) | — | — | — | — | — |\n\
\n\
## Constants\n\
\n\
| Name | Type | Quantity | Unit | Base | Log rate | Security |\n\
| --- | --- | --- | --- | --- | --- | --- |\n\
| <a id=\"root-engine-maxrpm\" class=\"m1-row-anchor\"></a>`Root.Engine.MaxRpm` | u16 | — | — | — | — | — |\n\
\n\
## Tables\n\
\n\
<a id=\"root-engine-ignitionmap\"></a>\n\
\n\
### Root.Engine.IgnitionMap\n\
\n\
Table — shape requires a loaded `.m1cfg`\n\
\n";
    assert_eq!(read(&out, "Root.Engine.md"), expected);
}

#[test]
fn golden_deep_group_page_with_breadcrumb_and_object() {
    // The four-deep leaf: a full ancestor breadcrumb (every ancestor a link, the
    // current segment plain) plus an Objects section for the package object.
    let out = render_fixture();
    let expected = "[Root](Root.md) › [Engine](Root.Engine.md) › [Fuel](Root.Engine.Fuel.md) › Pump\n\
\n\
# Root.Engine.Fuel.Pump\n\
\n\
## Channels\n\
\n\
| Name | Type | Quantity | Unit | Base | Log rate | Security |\n\
| --- | --- | --- | --- | --- | --- | --- |\n\
| <a id=\"root-engine-fuel-pump-demand\" class=\"m1-row-anchor\"></a>`Root.Engine.Fuel.Pump.Demand` | f32 | — | % | — | — | — |\n\
\n\
## Objects\n\
\n\
<a id=\"root-engine-fuel-pump-oilp\"></a>\n\
\n\
### Root.Engine.Fuel.Pump.OilP\n\
\n\
**Class:** MoTeC Input.Sensor\n\
\n\
(no members)\n\
\n";
    assert_eq!(read(&out, "Root.Engine.Fuel.Pump.md"), expected);
}

#[test]
fn golden_enums_page() {
    let out = render_fixture();
    let expected = "# Enums\n\
\n\
<a id=\"drive-state\"></a>\n\
\n\
## Drive State (default: Off)\n\
\n\
- 0 = Off (default)\n\
- 1 = On\n\
\n";
    assert_eq!(read(&out, "enums.md"), expected);
}

/// #73: full-document HTML golden. The two large inline asset constants
/// (`<style>` CSS and the behaviour `<script>`) are elided to sentinels so the
/// test pins the *composition seam* the split created — head/title, the nav
/// tree, the toolbar, the security filter panel, the Markdown→HTML fragment, the
/// `.md`→`.html` link rewrite, and the shared search-index sidecar — byte-for-byte,
/// while separately asserting the elided asset blocks are present and non-trivial.
#[test]
fn golden_group_page_html_composition() {
    let out = tempfile::tempdir().unwrap();
    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            "tests/fixtures/synthetic/Project.m1prj",
            "--out",
            out.path().to_str().unwrap(),
            "--format",
            "html",
            "--title",
            TITLE,
        ])
        .assert()
        .success();
    let page = read(&out, "Root.Engine.html");

    // The inline assets are deterministic but large; assert they are present and
    // substantial, then elide them so the golden pins the composed page around them.
    let style = between(&page, "<style>", "</style>");
    let script = between(&page, "<script>", "</script>");
    assert!(style.len() > 500, "inline <style> asset missing/short");
    assert!(
        script.len() > 500,
        "inline behaviour <script> asset missing/short"
    );
    let skeleton = page
        .replacen(style, "STYLE", 1)
        .replacen(script, "SCRIPT", 1);

    let expected = r#"<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>Synth Fixture</title><style>STYLE</style></head><body><nav><h2>Navigation</h2><a href="index.html">Index</a><a href="enums.html">Enums</a><ul><li><a href="Root.html">Root</a><ul><li><a href="Root.Engine.html">Engine</a><ul><li><a href="Root.Engine.Fuel.html">Fuel</a><ul><li><a href="Root.Engine.Fuel.Pump.html">Pump</a></li></ul></li></ul></li></ul></li></ul></nav><main><div class="toolbar"><button id="menu-toggle" class="btn" title="Toggle navigation">☰</button><input id="search-box" type="search" placeholder="Search symbols, functions, tables…" autocomplete="off"><button id="theme-toggle" class="btn" title="Toggle dark mode">◐</button></div><ul id="search-results"></ul><div id="toc-slot"></div><details id="filters" class="filters"><summary>Filter rows</summary><div><strong>Security</strong> <label><input type="checkbox" data-sec="Tune"> Tune</label></div><div><small>Tick levels/tags to show only matching rows; all unticked shows everything.</small></div></details><p><a href="Root.html">Root</a> › Engine</p>
<h1>Root.Engine</h1>
<h2>Sub-groups</h2>
<ul>
<li><a href="Root.Engine.Fuel.html">Fuel</a></li>
</ul>
<h2>Channels</h2>
<table><thead><tr><th>Name</th><th>Type</th><th>Quantity</th><th>Unit</th><th>Base</th><th>Log rate</th><th>Security</th></tr></thead><tbody>
<tr><td><a id="root-engine-speed" class="m1-row-anchor" data-security="Tune"></a><code>Root.Engine.Speed</code></td><td>f32</td><td>AngularVelocity</td><td>rpm</td><td>AngularVelocity</td><td>—</td><td>Tune</td></tr>
<tr><td><a id="root-engine-state" class="m1-row-anchor"></a><code>Root.Engine.State</code></td><td><a href="enums.html#drive-state">::This.Drive State</a></td><td>—</td><td>—</td><td>—</td><td>—</td><td>—</td></tr>
</tbody></table>
<h2>Constants</h2>
<table><thead><tr><th>Name</th><th>Type</th><th>Quantity</th><th>Unit</th><th>Base</th><th>Log rate</th><th>Security</th></tr></thead><tbody>
<tr><td><a id="root-engine-maxrpm" class="m1-row-anchor"></a><code>Root.Engine.MaxRpm</code></td><td>u16</td><td>—</td><td>—</td><td>—</td><td>—</td><td>—</td></tr>
</tbody></table>
<h2>Tables</h2>
<p><a id="root-engine-ignitionmap"></a></p>
<h3>Root.Engine.IgnitionMap</h3>
<p>Table — shape requires a loaded <code>.m1cfg</code></p>
</main><script src="search-index.js"></script><script>SCRIPT</script></body></html>"#;
    assert_eq!(skeleton, expected);
}

/// The content between the first `open` and the following `close`, exclusive —
/// used to slice out (and later elide) the inline asset blocks.
fn between<'a>(s: &'a str, open: &str, close: &str) -> &'a str {
    let start = s.find(open).expect("open tag") + open.len();
    let end = start + s[start..].find(close).expect("close tag");
    &s[start..end]
}
