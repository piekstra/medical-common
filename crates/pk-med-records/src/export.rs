//! `records export -o <DIR>`: the stream to `<DIR>/records.ndjson`, readable
//! by its owner only.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use pk_cli_core::CliError;

/// The stream file's name in the export directory.
pub const EXPORT_FILE: &str = "records.ndjson";

/// Create `<dir>/records.ndjson`, mode 0600 on Unix, creating `dir` (mode
/// 0700) when missing, and replacing a file already there.
///
/// The file is created new under a temporary name (`O_EXCL`, which never
/// follows a symbolic link) and renamed into place, so nothing at
/// `records.ndjson` is opened or written through: a symbolic link or a hard
/// link there is replaced, not followed, and a process that had the old file
/// open keeps the old file. A symbolic link there is still refused, for the
/// message.
pub fn create_export_file(dir: &Path) -> Result<(PathBuf, File), CliError> {
    if !dir.exists() {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(dir)
            .map_err(|e| CliError::Other(format!("creating {}: {e}", dir.display())))?;
    } else if !dir.is_dir() {
        return Err(CliError::Usage(format!(
            "{} is not a directory",
            dir.display()
        )));
    }
    let path = dir.join(EXPORT_FILE);
    if std::fs::symlink_metadata(&path).is_ok_and(|m| m.file_type().is_symlink()) {
        return Err(CliError::Usage(format!(
            "{} is a symbolic link; refusing to write through it",
            path.display()
        )));
    }
    let (temporary, file) = create_new_beside(dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // The mode a umask cannot widen past 0600.
        if let Err(e) = file.set_permissions(std::fs::Permissions::from_mode(0o600)) {
            let _ = std::fs::remove_file(&temporary);
            return Err(CliError::Other(format!(
                "restricting {}: {e}",
                temporary.display()
            )));
        }
    }
    if let Err(e) = std::fs::rename(&temporary, &path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(CliError::Other(format!("creating {}: {e}", path.display())));
    }
    Ok((path, file))
}

/// A new file in `dir` under a name no one else holds.
fn create_new_beside(dir: &Path) -> Result<(PathBuf, File), CliError> {
    let mut last = None;
    for attempt in 0..16u32 {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or_default();
        let name = format!(
            ".{EXPORT_FILE}.{}.{nanos}.{attempt}.tmp",
            std::process::id()
        );
        let temporary = dir.join(name);
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(&temporary) {
            Ok(file) => return Ok((temporary, file)),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => {
                return Err(CliError::Other(format!(
                    "creating a file in {}: {e}",
                    dir.display()
                )))
            }
        }
    }
    Err(CliError::Other(format!(
        "creating a file in {}: {}",
        dir.display(),
        last.map(|e| e.to_string()).unwrap_or_default()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn scratch(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "pk-med-records-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn the_file_is_the_owners_alone() {
        let dir = scratch("mode").join("nested");
        let (path, mut file) = create_export_file(&dir).unwrap();
        writeln!(file, "x").unwrap();
        assert_eq!(path, dir.join(EXPORT_FILE));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            create_export_file(&dir).unwrap();
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "an existing file is narrowed too");
        }
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_file_already_there_is_replaced_not_written_through() {
        let dir = scratch("replace");
        std::fs::create_dir_all(&dir).unwrap();
        let other = dir.join("other");
        std::fs::write(&other, "kept").unwrap();
        std::fs::hard_link(&other, dir.join(EXPORT_FILE)).unwrap();
        let old = std::fs::File::open(dir.join(EXPORT_FILE)).unwrap();
        let (path, mut file) = create_export_file(&dir).unwrap();
        writeln!(file, "new").unwrap();
        assert_eq!(std::fs::read_to_string(&other).unwrap(), "kept");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
        let mut seen = String::new();
        std::io::Read::read_to_string(&mut &old, &mut seen).unwrap();
        assert_eq!(
            seen, "kept",
            "a reader of the old file never sees the new one"
        );
        let names: Vec<_> = std::fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names.len(), 2, "no temporary file is left: {names:?}");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_symbolic_link_is_not_followed() {
        let dir = scratch("link");
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join(EXPORT_FILE)).unwrap();
        assert_eq!(create_export_file(&dir).unwrap_err().exit_code(), 2);
        assert!(!dir.join("elsewhere").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
