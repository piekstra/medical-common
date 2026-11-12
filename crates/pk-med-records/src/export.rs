//! `records export -o <DIR>`: the stream to `<DIR>/records.ndjson`, readable
//! by its owner only, replacing the previous export only once it is whole.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use pk_cli_core::CliError;

use crate::stream::Finished;

/// The stream file's name in the export directory.
pub const EXPORT_FILE: &str = "records.ndjson";
/// Where an incomplete stream (`complete: false`) is committed, beside the
/// last complete export, which it never replaces.
pub const INCOMPLETE_FILE: &str = "records.incomplete.ndjson";

/// The temporary files' names: `.records.ndjson.<pid>.<nanos>.<n>.tmp`.
const TEMPORARY_PREFIX: &str = ".records.ndjson.";
const TEMPORARY_SUFFIX: &str = ".tmp";

/// An export being written: a new file beside `records.ndjson`, mode 0600 on
/// Unix, that becomes `records.ndjson` only on [`ExportFile::commit`] of a
/// complete stream. An incomplete one goes to `records.incomplete.ndjson`
/// instead. So the last complete export stays in place whether a run is
/// killed, runs out of space, or ends with `complete: false` on a provider's
/// error. Dropped without a commit, the file is removed. A run killed before
/// its drop leaves the file, and the next export to the directory removes
/// it: while an export runs it holds a lock on its file, and a file no live
/// process holds is a leftover.
///
/// Stream into it with [`RecordsWriter`](crate::RecordsWriter) through
/// [`ExportFile::file`] (or the `Write` impl), `finish` the writer, then
/// `commit` with what `finish` returned.
#[derive(Debug)]
pub struct ExportFile {
    path: PathBuf,
    temporary: PathBuf,
    file: File,
    done: bool,
}

impl ExportFile {
    /// Where a complete export goes on [`Self::commit`]:
    /// `<dir>/records.ndjson`.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file to stream into.
    pub fn file(&mut self) -> &mut File {
        &mut self.file
    }

    /// Flush the file to disk, rename it into place (replacing, never
    /// writing through, what is there), and flush the directory. `finished`
    /// is what [`RecordsWriter::finish`](crate::RecordsWriter::finish)
    /// returned for this file. A complete stream goes to `records.ndjson`
    /// and removes any `records.incomplete.ndjson`, which is older. An
    /// incomplete one goes to `records.incomplete.ndjson`, and the last
    /// complete export is kept. Returns where it went, for
    /// [`RecordsExport::new`](crate::RecordsExport::new).
    pub fn commit(mut self, finished: &Finished) -> Result<PathBuf, CliError> {
        self.file
            .sync_all()
            .map_err(|e| CliError::Other(format!("writing {}: {e}", self.temporary.display())))?;
        let target = if finished.trailer.complete {
            self.path.clone()
        } else {
            self.path.with_file_name(INCOMPLETE_FILE)
        };
        std::fs::rename(&self.temporary, &target)
            .map_err(|e| CliError::Other(format!("creating {}: {e}", target.display())))?;
        self.done = true;
        if finished.trailer.complete {
            // Best effort: the export is committed either way, and an
            // incomplete file left over is replaced by the next one.
            let _ = std::fs::remove_file(self.path.with_file_name(INCOMPLETE_FILE));
        }
        // The rename is durable once the directory is. A filesystem that
        // cannot sync a directory has still renamed the file.
        if let Some(dir) = self.path.parent() {
            if let Ok(dir) = File::open(dir) {
                let _ = dir.sync_all();
            }
        }
        Ok(target)
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
    let (temporary, file, locked) = create_locked_beside(dir)?;
    if locked {
        remove_leftovers(dir, &temporary, &file);
    }
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

/// A new file in `dir`, locked until it is committed or the process ends so
/// another export can tell it from a leftover. Between the create and the
/// lock, another export's sweep can take it for a leftover and remove it;
/// the file is then made again. Returns whether it is locked: on a
/// filesystem without locks (some network mounts) it is not, and no
/// leftovers are swept, since none could be told from a live export.
fn create_locked_beside(dir: &Path) -> Result<(PathBuf, File, bool), CliError> {
    for _ in 0..8 {
        let (temporary, file) = create_new_beside(dir)?;
        match file.try_lock() {
            Ok(()) if still_named(&temporary, &file) => return Ok((temporary, file, true)),
            // Swept before the lock: the name is gone.
            Ok(()) => continue,
            // A sweep holds it and is removing it.
            Err(std::fs::TryLockError::WouldBlock) => {
                if still_named(&temporary, &file) {
                    let _ = std::fs::remove_file(&temporary);
                }
                continue;
            }
            Err(std::fs::TryLockError::Error(_)) => return Ok((temporary, file, false)),
        }
    }
    Err(CliError::Other(format!(
        "creating a file in {}: other exports to it kept removing it",
        dir.display()
    )))
}

/// Whether `path` still names the file `file` has open.
fn still_named(path: &Path, file: &File) -> bool {
    let Ok(named) = std::fs::symlink_metadata(path) else {
        return false;
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        file.metadata()
            .is_ok_and(|open| (open.dev(), open.ino()) == (named.dev(), named.ino()))
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        named.is_file()
    }
}

/// Remove temporary files earlier exports to `dir` left when they were
/// killed: regular files with the temporary name, owned by whoever owns
/// `ours`, that no live process holds locked. Best effort: a file that
/// cannot be checked is left alone.
fn remove_leftovers(dir: &Path, ours: &Path, our_file: &File) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    #[cfg(unix)]
    let owner = {
        use std::os::unix::fs::MetadataExt;
        match our_file.metadata() {
            Ok(m) => m.uid(),
            Err(_) => return,
        }
    };
    #[cfg(not(unix))]
    let _ = our_file;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        if path == ours || !name.starts_with(TEMPORARY_PREFIX) || !name.ends_with(TEMPORARY_SUFFIX)
        {
            continue;
        }
        let Ok(seen) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if !seen.file_type().is_file() {
            continue;
        }
        let Ok(leftover) = File::open(&path) else {
            continue;
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            // The file opened is the one looked at, and it is the user's own.
            let Ok(opened) = leftover.metadata() else {
                continue;
            };
            if (opened.dev(), opened.ino()) != (seen.dev(), seen.ino()) || opened.uid() != owner {
                continue;
            }
        }
        if leftover.try_lock().is_ok() {
            let _ = std::fs::remove_file(&path);
        }
    }
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

    fn finished(complete: bool) -> Finished {
        Finished {
            trailer: crate::Trailer {
                schema: crate::stream::END_SCHEMA.into(),
                complete,
                counts: Default::default(),
                skipped: Vec::new(),
                finished_at: None,
                error: None,
            },
            bytes: 0,
        }
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
        let path = export.commit(&finished(true)).unwrap();
        assert_eq!(path, dir.join(EXPORT_FILE));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            let dir_mode = std::fs::metadata(&dir).unwrap().permissions().mode() & 0o777;
            assert_eq!(dir_mode, 0o700);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            create_export_file(&dir)
                .unwrap()
                .commit(&finished(true))
                .unwrap();
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
        export.commit(&finished(true)).unwrap();
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
        let done = writer.finish("2020-01-03T00:01:00Z", None).unwrap();
        let path = export.commit(&done).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), done.bytes);
        assert!(done.trailer.complete);
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
        let path = export.commit(&finished(true)).unwrap();
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

    #[test]
    fn an_incomplete_stream_never_replaces_a_complete_one() {
        let dir = scratch("incomplete");
        let mut export = create_export_file(&dir).unwrap();
        writeln!(export, "whole").unwrap();
        export.commit(&finished(true)).unwrap();

        let mut export = create_export_file(&dir).unwrap();
        writeln!(export, "cut short by a provider's error").unwrap();
        let went = export.commit(&finished(false)).unwrap();
        assert_eq!(went, dir.join(INCOMPLETE_FILE));
        assert_eq!(
            std::fs::read_to_string(dir.join(EXPORT_FILE)).unwrap(),
            "whole\n"
        );
        assert_eq!(names(&dir), [INCOMPLETE_FILE, EXPORT_FILE]);

        // The next complete export removes the incomplete one, now older.
        let mut export = create_export_file(&dir).unwrap();
        writeln!(export, "whole again").unwrap();
        export.commit(&finished(true)).unwrap();
        assert_eq!(names(&dir), [EXPORT_FILE]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_killed_exports_file_is_removed_by_the_next_and_a_live_ones_is_not() {
        let dir = scratch("leftover");
        std::fs::create_dir_all(&dir).unwrap();
        // What a killed run leaves: its file, held by no one.
        let leftover = dir.join(format!("{TEMPORARY_PREFIX}1.2.0{TEMPORARY_SUFFIX}"));
        std::fs::write(&leftover, "partial").unwrap();
        // An export still running holds its file locked.
        let live = create_export_file(&dir).unwrap();
        assert!(!leftover.exists(), "the leftover is removed");
        let second = create_export_file(&dir).unwrap();
        assert!(live.temporary.exists(), "a live export's file is kept");
        drop(second);
        drop(live);
        assert!(names(&dir).is_empty(), "{:?}", names(&dir));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn a_leftover_name_that_is_a_link_is_left_alone() {
        let dir = scratch("leftover-link");
        std::fs::create_dir_all(&dir).unwrap();
        let target = dir.join("kept");
        std::fs::write(&target, "kept").unwrap();
        let link = dir.join(format!("{TEMPORARY_PREFIX}1.2.0{TEMPORARY_SUFFIX}"));
        std::os::unix::fs::symlink(&target, &link).unwrap();
        drop(create_export_file(&dir).unwrap());
        assert!(std::fs::symlink_metadata(&link).is_ok());
        assert_eq!(std::fs::read_to_string(&target).unwrap(), "kept");
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
