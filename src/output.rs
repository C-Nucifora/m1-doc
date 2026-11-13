//! Validate the complete output set before replacing any generated file.

use crate::markdown::RenderedFile;
use std::collections::BTreeSet;
use std::io::{self, Write};
use std::path::{Component, Path};

pub(crate) fn write(out: &Path, files: &[RenderedFile], scoped: bool) -> io::Result<()> {
    match std::fs::symlink_metadata(out) {
        Ok(meta) if !meta.is_dir() => {
            return Err(io::Error::other(
                "output must be a directory, not a symlink or file",
            ));
        }
        Ok(_) if scoped && std::fs::read_dir(out)?.next().transpose()?.is_some() => {
            return Err(io::Error::other(
                "scoped generation requires an empty output directory; choose a new --out directory",
            ));
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let mut names = BTreeSet::new();
    for file in files {
        validate_name(&file.path)?;
        if !names.insert(file.path.to_ascii_lowercase()) {
            return Err(io::Error::other(format!(
                "duplicate output filename: {}",
                file.path
            )));
        }
        match std::fs::symlink_metadata(out.join(&file.path)) {
            Ok(meta) if !meta.is_file() => {
                return Err(io::Error::other(format!(
                    "output is not a regular file: {}",
                    file.path
                )));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    std::fs::create_dir_all(out)?;
    for file in files {
        let path = out.join(&file.path);
        let mut builder = tempfile::Builder::new();
        builder.prefix(".m1-doc-").suffix(".tmp");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            builder.permissions(std::fs::Permissions::from_mode(0o666));
        }
        let mut temp = builder.tempfile_in(out)?;
        if let Ok(meta) = std::fs::symlink_metadata(&path)
            && meta.is_file()
        {
            temp.as_file().set_permissions(meta.permissions())?;
        }
        temp.write_all(file.body.as_bytes())?;
        temp.flush()?;
        temp.persist(&path).map_err(|error| error.error)?;
    }
    Ok(())
}

fn validate_name(name: &str) -> io::Result<()> {
    let mut parts = Path::new(name).components();
    let stem = name.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || ["COM", "LPT"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|n| matches!(n, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
        });
    if !matches!(parts.next(), Some(Component::Normal(_)))
        || parts.next().is_some()
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || name.ends_with([' ', '.'])
        || reserved
    {
        return Err(io::Error::other(format!(
            "unsafe output filename: {name:?}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(path: &str) -> RenderedFile {
        RenderedFile {
            path: path.into(),
            body: "new documentation".into(),
        }
    }

    #[test]
    fn all_names_are_checked_before_any_output_changes() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("index.md"), "keep").unwrap();
        for name in [
            "/outside.md",
            "../outside.md",
            "nested/page.md",
            "C:\\outside.md",
            "C:outside.md",
            "\\\\host\\share.md",
            "CON.md",
            "LPT1.md",
            "a\n.md",
            "index.md",
            "INDEX.md",
        ] {
            assert!(
                write(dir.path(), &[file("index.md"), file(name)], false).is_err(),
                "{name}"
            );
            assert_eq!(
                std::fs::read_to_string(dir.path().join("index.md")).unwrap(),
                "keep"
            );
            assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 1);
        }
    }

    #[test]
    fn scoped_output_requires_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("private.html"), "old private page").unwrap();
        assert!(write(dir.path(), &[file("index.html")], true).is_err());
        assert!(!dir.path().join("index.html").exists());
        assert_eq!(
            std::fs::read_to_string(dir.path().join("private.html")).unwrap(),
            "old private page"
        );
        let fresh = dir.path().join("fresh");
        write(&fresh, &[file("index.html")], true).unwrap();
        assert!(fresh.join("index.html").is_file());
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_are_rejected_without_touching_targets() {
        let dir = tempfile::tempdir().unwrap();
        let victim = dir.path().join("victim");
        std::fs::write(&victim, "keep").unwrap();
        let out = dir.path().join("out");
        std::fs::create_dir(&out).unwrap();
        std::os::unix::fs::symlink(&victim, out.join("index.md")).unwrap();
        assert!(write(&out, &[file("index.md")], false).is_err());
        assert_eq!(std::fs::read_to_string(&victim).unwrap(), "keep");
        assert!(
            std::fs::symlink_metadata(out.join("index.md"))
                .unwrap()
                .is_symlink()
        );
    }
}
