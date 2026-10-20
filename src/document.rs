//! Reading and safely writing the document file.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// What we last saw on disk; a different stamp means someone else wrote the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stamp {
    pub modified: Option<SystemTime>,
    pub len: u64,
}

#[derive(Debug)]
pub enum LoadError {
    NotUtf8,
    /// NUL bytes: valid UTF-8, but not text, and GTK cannot hold them.
    Binary,
    Io(io::Error),
}

impl fmt::Display for LoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LoadError::NotUtf8 => write!(f, "файл не в кодировке UTF-8"),
            LoadError::Binary => write!(f, "в файле есть нулевые байты — это не текстовый файл"),
            LoadError::Io(e) => write!(f, "{e}"),
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SaveOutcome {
    Saved(Stamp),
    /// The file changed on disk since `expected`; nothing was written.
    Conflict,
}

/// Absolute path to read and write: symlinks are resolved so that saving
/// replaces the target file, not the link.
pub fn resolve(path: &Path) -> PathBuf {
    if let Ok(p) = fs::canonicalize(path) {
        return p;
    }
    if path.is_absolute() {
        return path.to_path_buf();
    }
    std::env::current_dir().map(|d| d.join(path)).unwrap_or_else(|_| path.to_path_buf())
}

/// Stamp of the file, or `None` if it does not exist.
pub fn stamp(path: &Path) -> io::Result<Option<Stamp>> {
    match fs::metadata(path) {
        Ok(m) => Ok(Some(Stamp { modified: m.modified().ok(), len: m.len() })),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

/// Reads the file. A missing file is an empty document (stamp `None`).
pub fn load(path: &Path) -> Result<(String, Option<Stamp>), LoadError> {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok((String::new(), None)),
        Err(e) => return Err(LoadError::Io(e)),
    };
    let st = stamp(path).map_err(LoadError::Io)?;
    let text = String::from_utf8(bytes).map_err(|_| LoadError::NotUtf8)?;
    if text.contains('\0') {
        return Err(LoadError::Binary);
    }
    Ok((text, st))
}

/// Writes `text` atomically (temp file + rename). Unless `force`, refuses with
/// `Conflict` when the file on disk does not match `expected`. A file that was
/// deleted meanwhile is simply created again.
pub fn save(path: &Path, text: &str, expected: Option<Stamp>, force: bool) -> io::Result<SaveOutcome> {
    let current = stamp(path)?;
    if !force && current.is_some() && current != expected {
        return Ok(SaveOutcome::Conflict);
    }
    let dir = path.parent().filter(|d| !d.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let name = path.file_name().ok_or_else(|| io::Error::other("в пути нет имени файла"))?;
    let tmp = dir.join(format!(".{}.opal-tmp", name.to_string_lossy()));
    let result = (|| {
        let mut file = fs::File::create(&tmp)?;
        file.write_all(text.as_bytes())?;
        file.sync_all()?;
        if current.is_some() {
            fs::set_permissions(&tmp, fs::metadata(path)?.permissions())?;
        }
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    if let Ok(d) = fs::File::open(dir) {
        let _ = d.sync_all();
    }
    let st = stamp(path)?.ok_or_else(|| io::Error::other("файл исчез сразу после записи"))?;
    Ok(SaveOutcome::Saved(st))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{PermissionsExt, symlink};

    fn saved(outcome: SaveOutcome) -> Stamp {
        match outcome {
            SaveOutcome::Saved(st) => st,
            SaveOutcome::Conflict => panic!("unexpected conflict"),
        }
    }

    #[test]
    fn missing_file_is_empty_document() {
        let dir = tempfile::tempdir().unwrap();
        let (text, st) = load(&dir.path().join("new.md")).unwrap();
        assert_eq!((text.as_str(), st), ("", None));
    }

    #[test]
    fn non_utf8_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("bad.md");
        fs::write(&p, [0xff, 0xfe, b'a']).unwrap();
        assert!(matches!(load(&p), Err(LoadError::NotUtf8)));
    }

    #[test]
    fn nul_bytes_are_refused() {
        // Valid UTF-8, but GTK cannot hold it: the document would open empty
        // and the first autosave would overwrite the file.
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("nul.md");
        fs::write(&p, b"# Notes\n\nimportant\0more\n").unwrap();
        let err = load(&p).expect_err("a file with NUL bytes must not open");
        assert!(err.to_string().contains("нулев"), "{err}");
    }

    #[test]
    fn save_then_load_roundtrips_bytes_exactly() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        let text = "# Привет\r\n\r\n**мир**\n";
        let st = saved(save(&p, text, None, false).unwrap());
        let (back, st2) = load(&p).unwrap();
        assert_eq!(back, text);
        assert_eq!(st2, Some(st));
        let leftovers: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(leftovers, vec![std::ffi::OsString::from("a.md")]);
    }

    #[test]
    fn save_keeps_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        fs::write(&p, "x").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        let st = stamp(&p).unwrap();
        saved(save(&p, "y", st, false).unwrap());
        assert_eq!(fs::metadata(&p).unwrap().permissions().mode() & 0o777, 0o600);
    }

    #[test]
    fn saving_through_a_symlink_writes_the_target() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("real.md");
        let link = dir.path().join("link.md");
        fs::write(&target, "old").unwrap();
        symlink(&target, &link).unwrap();
        let p = resolve(&link);
        let st = stamp(&p).unwrap();
        saved(save(&p, "new", st, false).unwrap());
        assert!(fs::symlink_metadata(&link).unwrap().file_type().is_symlink());
        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
    }

    #[test]
    fn external_change_is_a_conflict_unless_forced() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        let st = saved(save(&p, "mine", None, false).unwrap());
        fs::write(&p, "theirs, longer").unwrap();
        assert_eq!(save(&p, "mine 2", Some(st), false).unwrap(), SaveOutcome::Conflict);
        assert_eq!(fs::read_to_string(&p).unwrap(), "theirs, longer");
        saved(save(&p, "mine 2", Some(st), true).unwrap());
        assert_eq!(fs::read_to_string(&p).unwrap(), "mine 2");
    }

    #[test]
    fn file_appearing_for_a_new_document_is_a_conflict() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        fs::write(&p, "someone else's").unwrap();
        assert_eq!(save(&p, "mine", None, false).unwrap(), SaveOutcome::Conflict);
    }

    #[test]
    fn deleted_file_is_recreated() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        let st = saved(save(&p, "v1", None, false).unwrap());
        fs::remove_file(&p).unwrap();
        saved(save(&p, "v2", Some(st), false).unwrap());
        assert_eq!(fs::read_to_string(&p).unwrap(), "v2");
    }

    #[test]
    fn unwritable_directory_fails_cleanly() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a.md");
        fs::write(&p, "original").unwrap();
        let st = stamp(&p).unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o555)).unwrap();
        let probe = dir.path().join("probe");
        if fs::write(&probe, "").is_ok() {
            // Running as root: directory permissions are not enforced.
            let _ = fs::remove_file(&probe);
            fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
            return;
        }
        assert!(save(&p, "new", st, false).is_err());
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "original");
        let names: Vec<_> = fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, vec![std::ffi::OsString::from("a.md")]);
    }

    #[test]
    fn resolve_makes_relative_paths_absolute() {
        let p = resolve(Path::new("surely-missing-opal-file.md"));
        assert!(p.is_absolute());
        assert!(p.ends_with("surely-missing-opal-file.md"));
    }
}
