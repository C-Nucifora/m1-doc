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
## Groups\n\
\n\
- [Root](Root.md)\n\
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
| <a id=\"root-engine-speed\"></a>`Root.Engine.Speed` | f32 | AngularVelocity | rpm | AngularVelocity | — | Tune |\n\
| <a id=\"root-engine-state\"></a>`Root.Engine.State` | [::This.Drive State](enums.md#drive-state) | — | — | — | — | — |\n\
\n\
## Constants\n\
\n\
| Name | Type | Quantity | Unit | Base | Log rate | Security |\n\
| --- | --- | --- | --- | --- | --- | --- |\n\
| <a id=\"root-engine-maxrpm\"></a>`Root.Engine.MaxRpm` | u16 | — | — | — | — | — |\n\
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
| <a id=\"root-engine-fuel-pump-demand\"></a>`Root.Engine.Fuel.Pump.Demand` | f32 | — | % | — | — | — |\n\
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
- Off\n\
- On\n\
\n";
    assert_eq!(read(&out, "enums.md"), expected);
}
