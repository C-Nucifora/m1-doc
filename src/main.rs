mod html;
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
    /// Scripts (reserved for P2 function docs; ignored in P1).
    files: Vec<PathBuf>,
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
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
enum Format {
    Markdown,
    Html,
    Both,
}

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

    let model = match loader::load(&project_path, title) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("m1-doc: {}: {e}", project_path.display());
            process::exit(1);
        }
    };

    if let Err(e) = std::fs::create_dir_all(&args.out) {
        eprintln!("m1-doc: {}: {e}", args.out.display());
        process::exit(1);
    }

    // Build Markdown files once; HTML renderer consumes them.
    let md_files = markdown::render(&model);

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
    }
}
