//! Corpus smoke test: run the full doc generator end-to-end over a *real* M1
//! project and assert structural invariants — not exact bytes — so it catches
//! scale/encoding/hierarchy regressions that the tiny hand-written fixtures
//! never will, while surviving benign output changes (#36).
//!
//! Gated on a corpus path: `$M1_CORPUS_PATH` (a `Project.m1prj`) overrides the
//! sibling EV-M1 default. When neither exists — as in a fresh public clone — the
//! test skips gracefully, matching the sibling tools' corpus tests. The corpus
//! is treated strictly read-only: generation writes only into a tempdir.

use assert_cmd::Command;
use std::path::{Path, PathBuf};

/// Resolve the corpus `Project.m1prj`: `$M1_CORPUS_PATH` if set, else the sibling
/// EV-M1 checkout. Returns `None` (→ skip) when the chosen path does not exist.
fn corpus_project() -> Option<PathBuf> {
    let path = match std::env::var_os("M1_CORPUS_PATH") {
        Some(p) => PathBuf::from(p),
        None => {
            PathBuf::from("/home/nedlane/projects/m1-core-stack/EV-M1/UQR-EV/01.00/Project.m1prj")
        }
    };
    path.is_file().then_some(path)
}

/// Run `m1-doc --format markdown` over `project` into `out`, asserting exit 0.
fn generate(project: &Path, out: &Path) {
    Command::cargo_bin("m1-doc")
        .unwrap()
        .args([
            "--project",
            project.to_str().unwrap(),
            "--out",
            out.to_str().unwrap(),
            "--format",
            "markdown",
        ])
        .assert()
        .success();
}

/// Every `*.md` file under `dir` as `(filename, contents)`, sorted by name so
/// two runs compare in a stable order.
fn markdown_files(dir: &Path) -> Vec<(String, String)> {
    let mut files: Vec<(String, String)> = std::fs::read_dir(dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&p).unwrap())
        })
        .collect();
    files.sort();
    files
}

#[test]
fn corpus_generates_clean_deterministic_docs() {
    let Some(project) = corpus_project() else {
        eprintln!("corpus: no Project.m1prj found (set $M1_CORPUS_PATH); skipping");
        return;
    };

    // Record the corpus's pre-run state so we can prove generation never wrote to
    // it — the corpus is the real, actively-developed vehicle code (read-only).
    let project_meta = std::fs::metadata(&project).unwrap();
    let project_mtime = project_meta.modified().unwrap();

    let out_a = tempfile::tempdir().unwrap();
    generate(&project, out_a.path());
    let files_a = markdown_files(out_a.path());

    // --- exit 0 + a non-trivial site ---------------------------------------
    assert!(
        !files_a.is_empty(),
        "generation produced no Markdown over the corpus"
    );

    // A real project is non-trivial: the index plus many group pages.
    let index = files_a
        .iter()
        .find(|(n, _)| n == "index.md")
        .map(|(_, b)| b)
        .expect("index.md must be written");
    assert!(
        index.len() > 64 && index.contains("## Structure"),
        "index.md should be a non-trivial structure index; got:\n{index}"
    );
    let page_count = files_a.iter().filter(|(n, _)| n != "index.md").count();
    assert!(
        page_count > 1,
        "a real corpus should yield many group pages; got {page_count}"
    );

    // --- no empty output files ---------------------------------------------
    for (name, body) in &files_a {
        assert!(
            !body.trim().is_empty(),
            "output file {name} is empty — every page must carry content"
        );
    }

    // --- a page for every group present in the model -----------------------
    // The index lists the forest roots; every group page links its sub-groups by
    // filename. Walk that link graph from the index and assert each referenced
    // group page was actually written — i.e. no group is dangling.
    let names: std::collections::HashSet<&str> = files_a.iter().map(|(n, _)| n.as_str()).collect();
    for (name, body) in &files_a {
        for linked in linked_group_pages(body) {
            assert!(
                names.contains(linked.as_str()),
                "{name} links group page {linked}, which was not written \
                 (a group is missing its page)"
            );
        }
    }

    // --- never fake: no placeholder / debug markers ------------------------
    // The model degrades with an em dash or an explicit note, never invented
    // data. A literal TODO/FIXME or a "unknown-unknown" slug would mean a
    // renderer leaked a placeholder. (Plain "unknown" is a legitimate type
    // label for an unresolved channel and is intentionally NOT flagged.)
    for (name, body) in &files_a {
        for marker in ["TODO", "FIXME", "unknown-unknown", "{{", "}}"] {
            assert!(
                !body.contains(marker),
                "output file {name} contains placeholder marker {marker:?}"
            );
        }
    }

    // --- deterministic: a second run is byte-identical ---------------------
    let out_b = tempfile::tempdir().unwrap();
    generate(&project, out_b.path());
    let files_b = markdown_files(out_b.path());
    assert_eq!(
        files_a.len(),
        files_b.len(),
        "two runs produced a different number of files ({} vs {})",
        files_a.len(),
        files_b.len()
    );
    for ((na, ba), (nb, bb)) in files_a.iter().zip(files_b.iter()) {
        assert_eq!(na, nb, "file set diverged between runs");
        assert_eq!(
            ba, bb,
            "file {na} differs between two runs (non-deterministic)"
        );
    }

    // --- corpus untouched (read-only contract) -----------------------------
    let after_mtime = std::fs::metadata(&project).unwrap().modified().unwrap();
    assert_eq!(
        project_mtime, after_mtime,
        "generation modified the read-only corpus Project.m1prj"
    );
}

/// Extract the group-page filenames a page links to. A sub-group link looks like
/// `- [Engine](Root.Engine.md)` and a breadcrumb like `[Root](Root.md) ›`; both
/// use a `.md` target with no `#` fragment. Anchored links (`enums.md#x`) and the
/// Enums reference are skipped — only group pages are checked for existence.
fn linked_group_pages(body: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = body;
    while let Some(open) = rest.find("](") {
        let after = &rest[open + 2..];
        let Some(close) = after.find(')') else { break };
        let target = &after[..close];
        if target.ends_with(".md") && !target.contains('#') && target != "enums.md" {
            out.push(target.to_string());
        }
        rest = &after[close + 1..];
    }
    out
}
