//! `records export -o <DIR>`: the stream to `<DIR>/records.ndjson`, readable
//! by its owner only, replacing the previous export only once it is whole.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use pk_cli_core::CliError;

/// The stream file's name in the export directory.
pub const EXPORT_FILE: &str = "records.ndjson";

/// An export being written: a new file beside `records.ndjson`, mode 0600 on
/// Unix, that becomes `records.ndjson` only on [`ExportFile::commit`].
/// Until then a previous export there is untouched, so an export that is
/// killed, runs out of space or fails partway leaves the last complete one
/// in place. Dropped without a commit, the file is removed.
///
/// Stream into it with [`RecordsWriter`](crate::RecordsWriter) through
/// [`ExportFile::file`] (or the `Write` impl), `finish` the writer, then
/// `commit`.
#[derive(Debug)]
pub struct ExportFile {
    path: PathBuf,
    temporary: PathBuf,
    file: File,
    done: bool,
}

impl ExportFile {
    /// Where the export goes on [`Self::commit`]: `<dir>/records.ndjson`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file to stream into.
    pub fn file(&mut self) -> &mut File {
        &mut self.file
    }

    /// Flush the file to disk, rename it onto `records.ndjson` (replacing,
    /// never writing through, what is there), and flush the directory.
    /// Returns the final path. Call it once the writer has finished: what is
    /// committed is what was written.
    pub fn commit(mut self) -> Result<PathBuf, CliError> {
        self.file
            .sync_all()
            .map_err(|e| CliError::Other(format!("writing {}: {e}", self.temporary.display())))?;
        std::fs::rename(&self.temporary, &self.path)
            .map_err(|e| CliError::Other(format!("creating {}: {e}", self.path.display())))?;
        self.done = true;
        // The rename is durable once the directory is. A filesystem that
        // cannot sync a directory has still renamed the file.
        if let Some(dir) = self.path.parent() {
            if let Ok(dir) = File::open(dir) {
                let _ = dir.sync_all();
            }
        }
        Ok(self.path.clone())
    }
}

impl Write for ExportFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

impl Drop for ExportFile {
    fn drop(&mut self) {
        if !self.done {
            let _ = std::fs::remove_file(&self.temporary);
        }
    }
}

/// Start an export to `<dir>/records.ndjson`, creating `dir` (mode 0700)
/// when missing. Nothing at `records.ndjson` changes until
/// [`ExportFile::commit`].
///
/// The file is created new under a temporary name in `dir` (`O_EXCL`, which
/// never follows a symbolic link) and renamed into place by `commit`, so
/// nothing at `records.ndjson` is opened or written through: an ordinary file
/// or a hard link there is replaced, and a process that had the old file open
/// keeps the old file. A symbolic link there is refused (exit 2).
pub fn create_export_file(dir: &Path) -> Result<ExportFile, CliError> {
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
    let export = ExportFile {
        path,
        temporary,
        file,
        done: false,
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        // The mode a umask cannot widen past 0600. On failure the drop
        // removes the file.
        export
            .file
            .set_permissions(std::fs::Permissions::from_mode(0o600))
            .map_err(|e| {
                CliError::Other(format!("restricting {}: {e}", export.temporary.display()))
            })?;
    }
    Ok(export)
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

    fn names(dir: &Path) -> Vec<String> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn the_file_is_the_owners_alone() {
        let dir = scratch("mode").join("nested");
        let mut export = create_export_file(&dir).unwrap();
        writeln!(export.file(), "x").unwrap();
        let path = export.commit().unwrap();
        assert_eq!(path, dir.join(EXPORT_FILE));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            create_export_file(&dir).unwrap().commit().unwrap();
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "an existing file is narrowed too");
        }
        std::fs::remove_dir_all(dir.parent().unwrap()).unwrap();
    }

    #[test]
    fn the_previous_export_stays_until_the_commit() {
        let dir = scratch("commit");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(EXPORT_FILE), "previous\n").unwrap();

        let mut export = create_export_file(&dir).unwrap();
        writeln!(export, "half of a new one").unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(EXPORT_FILE)).unwrap(),
            "previous\n",
            "nothing changes while it is written"
        );
        drop(export);
        assert_eq!(
            std::fs::read_to_string(dir.join(EXPORT_FILE)).unwrap(),
            "previous\n",
            "a failed export leaves the last one"
        );
        assert_eq!(names(&dir), [EXPORT_FILE], "and no temporary file");

        let mut export = create_export_file(&dir).unwrap();
        writeln!(export.file(), "whole").unwrap();
        export.commit().unwrap();
        assert_eq!(
            std::fs::read_to_string(dir.join(EXPORT_FILE)).unwrap(),
            "whole\n"
        );
        assert_eq!(names(&dir), [EXPORT_FILE]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(feature = "test-util")]
    #[test]
    fn a_stream_is_written_and_committed() {
        let dir = scratch("stream");
        let mut export = create_export_file(&dir).unwrap();
        let mut writer =
            crate::RecordsWriter::start(export.file(), &crate::test_util::synthetic_header())
                .unwrap();
        for resource in pk_med_fhir::test_util::synthetic_all() {
            writer.resource(&resource).unwrap();
        }
        let finished = writer.finish("2020-01-03T00:01:00Z", None).unwrap();
        let path = export.commit().unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), finished.bytes);
        assert!(finished.trailer.complete);
        std::fs::remove_dir_all(&dir).unwrap();
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
        let mut export = create_export_file(&dir).unwrap();
        writeln!(export.file(), "new").unwrap();
        let path = export.commit().unwrap();
        assert_eq!(std::fs::read_to_string(&other).unwrap(), "kept");
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
        let mut seen = String::new();
        std::io::Read::read_to_string(&mut &old, &mut seen).unwrap();
        assert_eq!(
            seen, "kept",
            "a reader of the old file never sees the new one"
        );
        assert_eq!(names(&dir).len(), 2, "no temporary file is left");
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
