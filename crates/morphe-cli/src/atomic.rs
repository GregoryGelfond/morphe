//! Atomic in-place writes (docs/design/morphe.md §10.1): replace a file's
//! contents by writing a temporary file beside it and renaming over the target,
//! so an interrupted run never leaves a half-written source — the certificate
//! makes the content safe (§5.2), the atomic rename makes the write safe. This
//! is the only module that names any `tempfile` type, so a version move or a
//! swap of the temp-file mechanism touches this file alone (§12).

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use tempfile::NamedTempFile;

/// Replace `path`'s contents with `contents`, atomically. Writes a temporary
/// file in the target's own directory — so the rename stays on one filesystem
/// and is atomic — carries the target's existing permissions across the
/// replacement (an in-place reformat replaces contents without narrowing
/// access), and renames the temporary over the target.
///
/// # Errors
///
/// Any I/O error creating, writing, or renaming the temporary file.
pub(crate) fn replace(path: &Path, contents: &str) -> io::Result<()> {
    // A bare filename has an empty parent; the target's directory is then the
    // current directory, where the rename stays on one filesystem.
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let mut temp = NamedTempFile::new_in(directory)?;
    temp.write_all(contents.as_bytes())?;
    // Preserve the target's permissions; a failure to read or apply them must
    // not fail the format, so it is best-effort.
    if let Ok(metadata) = fs::metadata(path) {
        let _ = temp.as_file().set_permissions(metadata.permissions());
    }
    temp.persist(path).map(drop).map_err(|error| error.error)
}
