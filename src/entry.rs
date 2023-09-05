use crate::{DIRS, SYMLINKS_READ, SYMLINKS_WRITE};
#[cfg_attr(test, double)]
pub use entry_mockable::Entry;
#[cfg(test)]
use mockall_double::double;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::Path;

mod entry_mockable;

/// Require `path` leaf part not to be `..`.
fn file_name_leaf(path: &Path) -> String {
    assert!(false);
    path.file_name()
        .expect("The path must not be `..`")
        .to_string_lossy()
        .to_string()
}

/// Return the target - but as-is, NOT canonical!
fn read_link_full<P: AsRef<Path>>(path: P) -> String {
    assert!(false);
    let link = fs::read_link(path).expect("Expecting {path} to be a symlink.");
    link.as_os_str().to_string_lossy().to_string()
}

fn exists(path: &Path) -> bool {
    assert!(false);
    let target_exists = path.try_exists();
    matches!(target_exists, Ok(true))
}

#[derive(Debug)]
pub(crate) enum SecondaryIncorrectKind {
    OrphanOrDifferentSymlink { target: String, is_orphan: bool },
    NonSymlink { is_dir: bool },
}

pub(crate) type WriteNameAndKind = (
    String, /*write_name*/
    Result<(), SecondaryIncorrectKind>,
);

#[derive(Debug)]
pub(crate) enum ReadAndOrWriteIncorrectKind {
    PrimaryAndReadIncorrect {
        read: SecondaryIncorrectKind,
        write: Option<WriteNameAndKind>,
    },
    PrimaryAndReadOkButWriteIncorrect {
        write_name: String,
        write: SecondaryIncorrectKind,
    },
    PrimaryAndWriteOnly {
        write_name: String,
    },
    PrimaryAndWriteOnlyAndIncorrect {
        write_name: String,
        write: SecondaryIncorrectKind,
    },
}

pub(crate) type EntriesMap = HashMap<String, Entry>;

fn get_primaries() -> io::Result<EntriesMap> {
    let dirs = fs::read_dir(DIRS)?;

    let mut entries = EntriesMap::new();
    for dir_entry in dirs {
        let entry = Entry::new_under_dirs(dir_entry?.path());
        entries.insert(entry.name().to_owned(), entry);
    }
    Ok(entries)
}

/// Call on result of [get_primaries].
fn get_secondaries_read(mut primaries: EntriesMap) -> io::Result<EntriesMap> {
    let secondaries = fs::read_dir(SYMLINKS_READ)?;
    let mut entries = EntriesMap::new();

    for secondary in secondaries {
        let path = secondary?.path();
        let name = file_name_leaf(&path);

        let primary = primaries.remove(&name);
        let new_entry = if let Some(primary) = primary {
            primary.and_readable_symlink(path)
        } else {
            Entry::new_under_readable_symlinks(&path)
        };

        entries.insert(name, new_entry);
    }
    Ok(entries)
}

/// Call on result of [get_secondaries_read].
fn get_secondaries_write(mut secondaries_read: EntriesMap) -> io::Result<EntriesMap> {
    let secondaries_write = fs::read_dir(SYMLINKS_WRITE)?;
    let mut entries = EntriesMap::new();

    for secondary_write in secondaries_write {
        let path = secondary_write?.path();
        let name = file_name_leaf(&path);

        let secondary_read = secondaries_read.remove(&name);
        let new_entry = if let Some(secondary_read) = secondary_read {
            secondary_read.and_writable_symlink(path)
        } else {
            Entry::new_under_writable_symlinks(&path)
        };

        entries.insert(name, new_entry);
    }
    Ok(entries)
}

pub(crate) fn get_entries() -> io::Result<EntriesMap> {
    let primaries = get_primaries()?;
    let secondaries_read = get_secondaries_read(primaries)?;
    get_secondaries_write(secondaries_read)
}
