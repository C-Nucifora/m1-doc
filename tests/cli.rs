use assert_cmd::Command;

/// Minimal fixture XML shared by the HTML integration tests.
const FIXTURE_XML: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession><Project Name="Demo" TargetHardware="ecu120"><ComponentStream><List>
<Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
<Component Classname="BuiltIn.Channel" Name="Root.Engine.Speed"><Props Type="f32"><Locale><Default Unit="rpm"/></Locale></Props></Component>
</List></ComponentStream></Project></MoTeCM1BuildSession>"#;

#[test]
fn format_both_writes_md_and_html() {
    let dir = tempfile::tempdir().unwrap();
    let prj = dir.path().join("Project.m1prj");
    std::fs::write(&prj, FIXTURE_XML).unwrap();
    let out = dir.path().join("docs");

    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            prj.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--format",
            "both",
        ])
        .assert()
        .success();

    // Markdown files exist.
    assert!(
        out.join("index.md").exists(),
        "index.md missing with --format both"
    );
    assert!(
        out.join("Root.Engine.md").exists(),
        "Root.Engine.md missing with --format both"
    );

    // HTML files exist.
    assert!(
        out.join("index.html").exists(),
        "index.html missing with --format both"
    );
    assert!(
        out.join("Root.Engine.html").exists(),
        "Root.Engine.html missing with --format both"
    );

    // HTML contains a table (tables option enabled).
    let html = std::fs::read_to_string(out.join("Root.Engine.html")).unwrap();
    assert!(
        html.contains("<table"),
        "expected <table in Root.Engine.html; got:\n{html}"
    );

    // HTML contains the channel name.
    assert!(
        html.contains("Root.Engine.Speed"),
        "expected channel name in Root.Engine.html; got:\n{html}"
    );
}

// #35: `--format json` writes a single structured `m1-doc.json` for the project,
// with a versioned schema and a known symbol's metadata, and is deterministic.
#[test]
fn format_json_writes_deterministic_m1_doc_json() {
    let dir = tempfile::tempdir().unwrap();
    let prj = dir.path().join("Project.m1prj");
    std::fs::write(&prj, FIXTURE_XML).unwrap();

    // Run `--format json` into a fresh out dir and return the `m1-doc.json` body.
    let run = |out: &std::path::Path| -> String {
        Command::cargo_bin("m1-doc")
            .unwrap()
            .args([
                "--project",
                prj.to_str().unwrap(),
                "--out",
                out.to_str().unwrap(),
                "--format",
                "json",
            ])
            .assert()
            .success();
        std::fs::read_to_string(out.join("m1-doc.json")).unwrap()
    };

    let first = run(&dir.path().join("a"));

    // Single JSON file written; no Markdown/HTML alongside it.
    let out_a = dir.path().join("a");
    assert!(out_a.join("m1-doc.json").exists(), "m1-doc.json missing");
    assert!(
        !out_a.join("index.md").exists() && !out_a.join("index.html").exists(),
        "--format json must write only m1-doc.json"
    );

    // Versioned schema + the known channel and its display unit are present.
    assert!(
        first.contains("\"schema_version\": 1"),
        "missing schema_version; got:\n{first}"
    );
    assert!(
        first.contains("\"path\": \"Root.Engine.Speed\""),
        "missing known symbol path; got:\n{first}"
    );
    assert!(
        first.contains("\"unit\": \"rpm\""),
        "missing symbol unit metadata; got:\n{first}"
    );
    assert!(
        first.contains("\"kind\": \"channel\""),
        "missing symbol kind; got:\n{first}"
    );

    // Determinism: a second run into a different dir is byte-identical.
    let second = run(&dir.path().join("b"));
    assert_eq!(first, second, "JSON output must be deterministic");
}

#[test]
fn format_html_writes_html_only() {
    let dir = tempfile::tempdir().unwrap();
    let prj = dir.path().join("Project.m1prj");
    std::fs::write(&prj, FIXTURE_XML).unwrap();
    let out = dir.path().join("docs");

    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            prj.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--format",
            "html",
        ])
        .assert()
        .success();

    // HTML files must exist.
    assert!(
        out.join("index.html").exists(),
        "index.html missing with --format html"
    );
    assert!(
        out.join("Root.Engine.html").exists(),
        "Root.Engine.html missing with --format html"
    );

    // Markdown files must NOT exist.
    assert!(
        !out.join("index.md").exists(),
        "index.md should not be written with --format html"
    );
    assert!(
        !out.join("Root.Engine.md").exists(),
        "Root.Engine.md should not be written with --format html"
    );

    // HTML contains a table.
    let html = std::fs::read_to_string(out.join("Root.Engine.html")).unwrap();
    assert!(
        html.contains("<table"),
        "expected <table in Root.Engine.html; got:\n{html}"
    );
}

// #30: the inert `FILES` positional (the ex-#17 footgun) is removed. Passing a
// stray positional must now fail loudly rather than be silently dropped.
#[test]
fn stray_positional_argument_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let prj = dir.path().join("Project.m1prj");
    std::fs::write(&prj, FIXTURE_XML).unwrap();
    let script = dir.path().join("Main.m1scr");
    std::fs::write(&script, "// script\n").unwrap();
    let out = dir.path().join("docs");

    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            prj.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--format",
            "markdown",
            script.to_str().unwrap(),
        ])
        .assert()
        .failure();
}

#[test]
fn nonexistent_project_fails_with_path_in_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("docs");
    let nonexistent = "/nonexistent/Project.m1prj";

    let assert = Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            nonexistent,
            "--out",
            out.to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .failure();

    let stderr = std::str::from_utf8(&assert.get_output().stderr).unwrap();
    assert!(
        stderr.contains(nonexistent),
        "expected path in stderr; got:\n{stderr}"
    );
}

#[test]
fn generates_markdown_for_a_project() {
    let dir = tempfile::tempdir().unwrap();
    let prj = dir.path().join("Project.m1prj");
    std::fs::write(
        &prj,
        r#"<?xml version="1.0"?>
<MoTeCM1BuildSession><Project Name="Demo" TargetHardware="ecu120"><ComponentStream><List>
<Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
<Component Classname="BuiltIn.Channel" Name="Root.Engine.Speed"><Props Type="f32"><Locale><Default Unit="rpm"/></Locale></Props></Component>
</List></ComponentStream></Project></MoTeCM1BuildSession>"#,
    )
    .unwrap();
    let out = dir.path().join("docs");

    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            prj.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success();

    // The index links the forest root (Root); the tree is reachable by
    // descending from there.
    let index = std::fs::read_to_string(out.join("index.md")).unwrap();
    assert!(index.contains("[Root](Root.md)"), "index:\n{index}");

    // The root page lists Engine as a sub-group (descend into the tree).
    let root = std::fs::read_to_string(out.join("Root.md")).unwrap();
    assert!(
        root.contains("## Sub-groups") && root.contains("[Engine](Root.Engine.md)"),
        "root page:\n{root}"
    );

    // The engine page documents its direct member and carries a breadcrumb.
    let page = std::fs::read_to_string(out.join("Root.Engine.md")).unwrap();
    assert!(page.contains("Root.Engine.Speed"), "page:\n{page}");
    assert!(
        page.contains("[Root](Root.md) › Engine"),
        "breadcrumb missing:\n{page}"
    );
}

/// #34: `--only-security` scopes generation to the requested access level(s).
/// A project with a Tune channel and a Calibration parameter, generated with
/// `--only-security Tune`, must document only the Tune symbol.
const SCOPED_XML: &str = r#"<?xml version="1.0"?>
<MoTeCM1BuildSession><Project Name="Demo" TargetHardware="ecu120"><ComponentStream><List>
<Component Classname="BuiltIn.GroupCompound" Name="Root.Engine"/>
<Component Classname="BuiltIn.Channel" Name="Root.Engine.Speed"><Props Type="f32" Security="Tune"/></Component>
<Component Classname="BuiltIn.Parameter" Name="Root.Engine.Gain"><Props Type="f32" Security="Calibration"/></Component>
</List></ComponentStream></Project></MoTeCM1BuildSession>"#;

#[test]
fn only_security_scopes_generation() {
    let dir = tempfile::tempdir().unwrap();
    let prj = dir.path().join("Project.m1prj");
    std::fs::write(&prj, SCOPED_XML).unwrap();
    let out = dir.path().join("docs");

    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            prj.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--format",
            "markdown",
            "--only-security",
            "Tune",
        ])
        .assert()
        .success();

    let page = std::fs::read_to_string(out.join("Root.Engine.md")).unwrap();
    assert!(
        page.contains("Root.Engine.Speed"),
        "the Tune symbol must be documented:\n{page}"
    );
    assert!(
        !page.contains("Root.Engine.Gain"),
        "the Calibration symbol must be scoped out:\n{page}"
    );
}

#[test]
fn only_tag_with_no_matches_produces_an_empty_scope() {
    // No symbol carries the tag, so the scoped model is empty — generation still
    // succeeds and writes a (memberless) index rather than crashing.
    let dir = tempfile::tempdir().unwrap();
    let prj = dir.path().join("Project.m1prj");
    std::fs::write(&prj, SCOPED_XML).unwrap();
    let out = dir.path().join("docs");

    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            prj.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--format",
            "markdown",
            "--only-tag",
            "nonexistent",
        ])
        .assert()
        .success();

    assert!(
        out.join("index.md").exists(),
        "index is still written for an empty scope"
    );
    assert!(
        !out.join("Root.Engine.md").exists(),
        "a group with no matching symbol is pruned"
    );
}

// #37: `--graph <group>` must name a real group. A typo (`Root.Engien`) should
// fail fast with a usage error (exit 2) on stderr rather than silently emitting
// an empty "No documented relationships" subsystem page and a dead index link.
#[test]
fn graph_with_unknown_group_fails_and_writes_no_graph_page() {
    let dir = tempfile::tempdir().unwrap();
    let prj = dir.path().join("Project.m1prj");
    std::fs::write(&prj, FIXTURE_XML).unwrap();
    let out = dir.path().join("docs");

    let assert = Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            prj.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--format",
            "markdown",
            "--graph",
            "Root.Engien",
        ])
        .assert()
        .failure()
        .code(2);

    let stderr = std::str::from_utf8(&assert.get_output().stderr).unwrap();
    assert!(
        stderr.contains("--graph") && stderr.contains("Root.Engien"),
        "expected an unknown-group error naming the typo on stderr; got:\n{stderr}"
    );

    // No subsystem graph page (or any output) was emitted for the bogus group.
    let graph_pages: Vec<_> = std::fs::read_dir(&out)
        .map(|rd| {
            rd.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.starts_with("graph.") && n.ends_with(".md"))
                .collect()
        })
        .unwrap_or_default();
    assert!(
        graph_pages.is_empty(),
        "no graph.*.md page must be written for an unknown group; found: {graph_pages:?}"
    );
}

// A genuine group still produces a subsystem page even when it has no edges:
// validity is checked against the model, not against the diagram being empty.
#[test]
fn graph_with_real_but_edgeless_group_succeeds() {
    let dir = tempfile::tempdir().unwrap();
    let prj = dir.path().join("Project.m1prj");
    std::fs::write(&prj, FIXTURE_XML).unwrap();
    let out = dir.path().join("docs");

    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            prj.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--format",
            "markdown",
            "--graph",
            "Root.Engine",
        ])
        .assert()
        .success();

    let graph = std::fs::read_to_string(out.join("graph.root-engine.md")).unwrap();
    assert!(
        graph.contains("Subsystem: Root.Engine"),
        "the real group's subsystem page must render:\n{graph}"
    );
}
