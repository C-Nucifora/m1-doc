mod diagram;
mod graph;
mod html;
mod json;
mod loader;
mod markdown;
mod model;

use clap::{Parser, ValueEnum};
use std::path::PathBuf;
use std::process;

#[derive(Parser, Debug)]
#[command(
    name = "m1-doc",
    version,
    about = "Documentation generator for MoTeC M1 projects"
)]
struct Args {
    /// Project.m1prj (defaults to nearest upward, or $M1_PROJECT).
    #[arg(long)]
    project: Option<PathBuf>,
    /// Output directory for generated files.
    #[arg(long, default_value = "m1-doc")]
    out: PathBuf,
    /// Output format.
    #[arg(long, value_enum, default_value_t = Format::Both)]
    format: Format,
    /// Index heading (defaults to the project file's directory name).
    #[arg(long)]
    title: Option<String>,
    /// Base URL prepended to each function's source path to build a link to the
    /// `.m1scr` in the published site (e.g.
    /// `https://github.com/UQRacing/EV-M1/blob/main`). Without it, source paths
    /// are shown as plain text.
    #[arg(long, alias = "repo-url")]
    source_base: Option<String>,
    /// Embed each function's script body in a collapsible block (off by default).
    #[arg(long)]
    include_source: bool,
    /// Scope generation to symbols at these security levels (comma-separated,
    /// e.g. `Tune,Calibration`) — a calibration-focused subset. Non-matching
    /// symbols, and functions/tables/objects/CAN, are omitted (#34).
    #[arg(long, value_name = "LEVELS")]
    only_security: Option<String>,
    /// Scope generation to symbols carrying this tag (#34). Combine with
    /// `--only-security` to intersect both filters.
    #[arg(long, value_name = "TAG")]
    only_tag: Option<String>,
    /// Also emit a focused subsystem-graph page for this group path (e.g.
    /// `Root.Engine`): an interactive call/data-flow diagram of the whole
    /// subtree under the group (#37).
    #[arg(long, value_name = "GROUP")]
    graph: Option<String>,
    /// Hops the `--graph` subsystem expands across its boundary (default 1).
    #[arg(long, value_name = "N", default_value_t = 1)]
    graph_depth: usize,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    Markdown,
    Html,
    Both,
    /// A single machine-readable `m1-doc.json` of the whole model (#35).
    Json,
}

/// Filename of the machine-readable JSON document (#35).
const JSON_FILE: &str = "m1-doc.json";

/// Resolve the project path: explicit `--project`, then `$M1_PROJECT`, then the
/// nearest `Project.m1prj` upward from the cwd.
fn resolve_project(arg: Option<PathBuf>) -> Option<PathBuf> {
    if let Some(p) = arg {
        return Some(p);
    }
    if let Ok(p) = std::env::var("M1_PROJECT") {
        return Some(PathBuf::from(p));
    }
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join("Project.m1prj");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// Levenshtein edit distance between two strings, used to suggest the closest
/// real group when `--graph` is given an unknown one.
fn edit_distance(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    let mut cur = vec![0usize; b.len() + 1];
    for (i, ca) in a.iter().enumerate() {
        cur[0] = i + 1;
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != cb);
            cur[j + 1] = (prev[j + 1] + 1).min(cur[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut cur);
    }
    prev[b.len()]
}

/// The closest group path to `group` by edit distance, if a reasonably close one
/// exists — a "did you mean" suggestion for a mistyped `--graph` argument. Only
/// returns a hint when the distance is within a third of the target's length, so
/// wildly different inputs get no misleading suggestion.
fn nearest_group<'a>(group: &str, model: &'a model::DocModel) -> Option<&'a str> {
    let threshold = (group.chars().count() / 3).max(1);
    model
        .groups
        .iter()
        .map(|g| (edit_distance(group, &g.path), g.path.as_str()))
        .filter(|(d, _)| *d <= threshold)
        .min_by_key(|(d, _)| *d)
        .map(|(_, path)| path)
}

fn main() {
    let args = Args::parse();

    let Some(project_path) = resolve_project(args.project) else {
        eprintln!("m1-doc: no Project.m1prj found (pass --project or set $M1_PROJECT)");
        process::exit(2);
    };

    let title = args.title.unwrap_or_else(|| {
        project_path
            .parent()
            .and_then(|d| d.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "M1 Project".into())
    });

    let mut model = match loader::load(&project_path, title) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("m1-doc: {}: {e}", project_path.display());
            process::exit(1);
        }
    };

    // #37: `--graph <group>` must name a real group. Validate against the FULL
    // model — before any scoping (#34) prunes groups — so a typo (`Root.Engien`)
    // fails fast with a usage error rather than silently producing an empty "No
    // documented relationships" page and a dead index link, while a real but
    // scoped-out group is never mistaken for a typo.
    if let Some(group) = args.graph.as_deref()
        && !model.groups.iter().any(|g| g.path == group)
    {
        eprint!("m1-doc: --graph: no group named `{group}`");
        if let Some(near) = nearest_group(group, &model) {
            eprint!(" (did you mean `{near}`?)");
        }
        eprintln!();
        process::exit(2);
    }

    // Scoped generation (#34): narrow the model to the requested security
    // level(s) and/or tag before rendering, so every output format reflects the
    // same subset. Applied once, on the model, so Markdown/HTML/JSON agree.
    let only_security: Option<Vec<String>> = args.only_security.as_deref().map(|s| {
        s.split(',')
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .map(str::to_string)
            .collect()
    });
    if only_security.is_some() || args.only_tag.is_some() {
        model.retain_scoped(only_security.as_deref(), args.only_tag.as_deref());
    }

    if let Err(e) = std::fs::create_dir_all(&args.out) {
        eprintln!("m1-doc: {}: {e}", args.out.display());
        process::exit(1);
    }

    // Build Markdown files once; HTML renderer consumes them. Function source
    // links / embedding are driven by the CLI flags (#30).
    let render_opts = markdown::RenderOptions {
        source_base: args.source_base,
        include_source: args.include_source,
        graph: args.graph.map(|group| markdown::GraphSpec {
            group,
            depth: args.graph_depth,
        }),
    };
    let md_files = markdown::render_with(&model, &render_opts);

    /// Write a single [`markdown::RenderedFile`] under `out`, exiting on error.
    fn write_file(out: &std::path::Path, file: &markdown::RenderedFile) {
        let path = out.join(&file.path);
        if let Err(e) = std::fs::write(&path, &file.body) {
            eprintln!("m1-doc: {}: {e}", path.display());
            std::process::exit(1);
        }
    }

    match args.format {
        Format::Markdown => {
            for f in &md_files {
                write_file(&args.out, f);
            }
        }
        Format::Html => {
            for f in &html::render(&md_files, &model) {
                write_file(&args.out, f);
            }
        }
        Format::Both => {
            for f in &md_files {
                write_file(&args.out, f);
            }
            for f in &html::render(&md_files, &model) {
                write_file(&args.out, f);
            }
        }
        Format::Json => {
            write_file(
                &args.out,
                &markdown::RenderedFile {
                    path: JSON_FILE.to_string(),
                    body: json::render(&model),
                },
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;

    /// #30: the inert `FILES` positional is gone — a stray positional now errors
    /// rather than being silently dropped (no more no-op footgun, ex-#17).
    #[test]
    fn stray_positional_is_rejected() {
        let err = Args::try_parse_from(["m1-doc", "Some.m1scr"]);
        assert!(
            err.is_err(),
            "a positional arg must be rejected now that FILES is removed"
        );
    }

    /// #30: the new source flags parse, including the `--repo-url` alias.
    #[test]
    fn source_flags_parse() {
        let a = Args::try_parse_from([
            "m1-doc",
            "--source-base",
            "https://example/blob/main",
            "--include-source",
        ])
        .expect("source flags should parse");
        assert_eq!(a.source_base.as_deref(), Some("https://example/blob/main"));
        assert!(a.include_source);

        let b = Args::try_parse_from(["m1-doc", "--repo-url", "https://x/blob/main"])
            .expect("--repo-url alias should parse");
        assert_eq!(b.source_base.as_deref(), Some("https://x/blob/main"));
        assert!(!b.include_source, "include_source defaults off");
    }
}
