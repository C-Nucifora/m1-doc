use assert_cmd::Command;

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

    let index = std::fs::read_to_string(out.join("index.md")).unwrap();
    assert!(
        index.contains("[Root.Engine](Root.Engine.md)"),
        "index:\n{index}"
    );
    let page = std::fs::read_to_string(out.join("Root.Engine.md")).unwrap();
    assert!(page.contains("Root.Engine.Speed"), "page:\n{page}");
}
