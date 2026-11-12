//! `records export -o <DIR>`: the stream to `<DIR>/records.ndjson`, readable
//! by its owner only.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use pk_cli_core::CliError;

/// The stream file's name in the export directory.
pub const EXPORT_FILE: &str = "records.ndjson";

/// Create (or truncate) `<dir>/records.ndjson`, mode 0600 on Unix, creating
/// `dir` (mode 0700) when missing. A file there that is a symbolic link is
/// refused rather than followed.
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
    let mut options = OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let file = options
        .open(&path)
        .map_err(|e| CliError::Other(format!("creating {}: {e}", path.display())))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| CliError::Other(format!("restricting {}: {e}", path.display())))?;
    }
    Ok((path, file))
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
    fn a_symbolic_link_is_not_followed() {
        let dir = scratch("link");
        std::fs::create_dir_all(&dir).unwrap();
        std::os::unix::fs::symlink(dir.join("elsewhere"), dir.join(EXPORT_FILE)).unwrap();
        assert_eq!(create_export_file(&dir).unwrap_err().exit_code(), 2);
        assert!(!dir.join("elsewhere").exists());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
