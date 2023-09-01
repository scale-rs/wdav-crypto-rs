use crate::{DIRS, SYMLINKS_READ, SYMLINKS_WRITE};
use std::collections::HashMap;
use std::fs::{self, DirEntry};
use std::io;
use std::{path::Path, path::PathBuf};

/// Require `path` leaf part not to be `..`.
fn file_name_leaf(path: &Path) -> String {
    path.file_name()
        .expect("The path must not be `..`")
        .to_string_lossy()
        .to_string()
}

/// Return the target - but as-is, NOT canonical!
fn read_link_full<P: AsRef<Path>>(path: &P) -> String {
    let link = fs::read_link(path).expect("Expecting {path} to be a symlink.");
    link.as_os_str().to_string_lossy().to_string()
}

fn exists(path: &Path) -> bool {
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

/// Dir entry immediately below either [DIRS], and/or [SYMLINKS_READ] and/or [SYMLINKS_WRITE].
#[derive(Debug)]
pub(crate) enum Entry {
    PrimaryOnly {
        name: String,
    },
    PrimaryAndReadOnly {
        name: String,
    },
    PrimaryAndReadWrite {
        name: String,
        // Write symlink (hash-based) source name
        write_name: String,
    },
    PrimaryAndReadAndOrWriteIncorrect {
        name: String,
        kind: ReadAndOrWriteIncorrectKind,
    },
    PrimaryNonDir {
        name: String,
        path: PathBuf,
    },

    SecondaryIncorrect {
        name: String,
        /// Whether it's under [SYMLINKS_READ]. Otherwise it's under [SYMLINKS_WRITE].
        is_read: bool,
        kind: SecondaryIncorrectKind,
    }, /*,
       SecondaryReadOrphanSymlink {
           name: String,
           target: String,
       },
       SecondaryReadNonSymlink {
           name: String,
           is_dir: bool,
       },
       SecondaryWriteOrphanSymlink {
           name: String,
           target: String,
       },
       SecondaryWriteNonSymlink {
           name: String,
           is_dir: bool,
       },*/
}

impl Entry {
    pub(crate) fn is_ok_and_complete(&self) -> bool {
        match self {
            Self::PrimaryAndReadOnly { .. } | Self::PrimaryAndReadWrite { .. } => true,
            _ => false,
        }
    }
    pub(crate) fn is_readable(&self) -> bool {
        self.is_ok_and_complete()
    }
    pub(crate) fn is_writable(&self) -> bool {
        matches!(self, Self::PrimaryAndReadWrite { .. })
    }
    pub(crate) fn name(&self) -> &str {
        match self {
            Self::PrimaryOnly { name }
            | Self::PrimaryAndReadOnly { name }
            | Self::PrimaryAndReadWrite { name, .. }
            | Self::PrimaryAndReadAndOrWriteIncorrect { name, .. }
            | Self::PrimaryNonDir { name, .. }
            | Self::SecondaryIncorrect { name, .. } => &name,
        }
    }
    pub(crate) fn write_name(&self) -> &str {
        match self {
            Self::PrimaryAndReadWrite {
                name: _,
                write_name: write,
            } => &write,
            _ => unreachable!(
                "Can be called only on ReadWrite variant, but it was invoked on {:?}.",
                self
            ),
        }
    }

    pub(crate) fn new_under_dirs(path: PathBuf) -> Self {
        let name = path.to_string_lossy().to_string();
        if path.is_dir() {
            Self::PrimaryOnly { name }
        } else {
            Self::PrimaryNonDir { name, path }
        }
    }

    pub(crate) fn and_readable_symlink(self, path: PathBuf) -> Self {
        if let Self::PrimaryOnly { name } = self {
            return if path.is_symlink() {
                let target = read_link_full(&path);
                if target == format!("{SYMLINKS_READ}/{name}") {
                    Self::PrimaryAndReadOnly { name }
                } else {
                    let is_orphan = !exists(&path);

                    Self::PrimaryAndReadAndOrWriteIncorrect {
                        name,
                        kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadIncorrect {
                            read: SecondaryIncorrectKind::OrphanOrDifferentSymlink {
                                target,
                                is_orphan,
                            },
                            write: None,
                        },
                    }
                }
            } else {
                Self::PrimaryAndReadAndOrWriteIncorrect {
                    name,
                    kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadIncorrect {
                        read: SecondaryIncorrectKind::NonSymlink {
                            is_dir: path.is_dir(),
                        },
                        write: None,
                    },
                }
            };
        }
        panic!(
            "Expected variant PrimaryOnly, but called on variant {:?}.",
            self
        );
    }

    fn _new_under_symlinks(path: &PathBuf, is_read: bool) -> Self {
        let name = file_name_leaf(path);

        if path.is_symlink() {
            let target = read_link_full(path);
            let is_orphan = exists(path);
            Self::SecondaryIncorrect {
                name,
                is_read,
                kind: SecondaryIncorrectKind::OrphanOrDifferentSymlink { target, is_orphan },
            }
        } else {
            let is_dir = path.is_dir();
            Self::SecondaryIncorrect {
                name,
                is_read,
                kind: SecondaryIncorrectKind::NonSymlink { is_dir },
            }
        }
    }

    pub(crate) fn new_under_readable_symlinks(path: &PathBuf) -> Self {
        Self::_new_under_symlinks(path, true)
    }

    pub(crate) fn and_writable_symlink(self, path: PathBuf) -> Self {
        // @TODO hash!!!!:
        let write_name = self.name().clone().to_owned();

        if let Self::PrimaryAndReadOnly { name } = self {
            return if path.is_symlink() {
                let target = read_link_full(&path);
                if target == format!("{SYMLINKS_WRITE}/{write_name}") {
                    Self::PrimaryAndReadWrite { name, write_name }
                } else {
                    let is_orphan = !exists(&path);

                    Self::PrimaryAndReadAndOrWriteIncorrect {
                        name,
                        kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadOkButWriteIncorrect {
                            write: SecondaryIncorrectKind::OrphanOrDifferentSymlink {
                                target,
                                is_orphan,
                            },
                            write_name,
                        },
                    }
                }
            } else {
                Self::PrimaryAndReadAndOrWriteIncorrect {
                    name,
                    kind: ReadAndOrWriteIncorrectKind::PrimaryAndReadOkButWriteIncorrect {
                        write_name,
                        write: SecondaryIncorrectKind::NonSymlink {
                            is_dir: path.is_dir(),
                        },
                    },
                }
            };
        } else if let Self::PrimaryOnly { name } = self {
            return if path.is_symlink() {
                let target = read_link_full(&path);
                if target == format!("{SYMLINKS_WRITE}/{write_name}") {
                    Self::PrimaryAndReadAndOrWriteIncorrect {
                        name,
                        kind: ReadAndOrWriteIncorrectKind::PrimaryAndWriteOnly { write_name },
                    }
                } else {
                    let is_orphan = !exists(&path);

                    Self::PrimaryAndReadAndOrWriteIncorrect {
                        name,
                        kind: ReadAndOrWriteIncorrectKind::PrimaryAndWriteOnlyAndIncorrect {
                            write_name,
                            write: SecondaryIncorrectKind::OrphanOrDifferentSymlink {
                                target,
                                is_orphan,
                            },
                        },
                    }
                }
            } else {
                Self::PrimaryAndReadAndOrWriteIncorrect {
                    name,
                    kind: ReadAndOrWriteIncorrectKind::PrimaryAndWriteOnlyAndIncorrect {
                        write_name,
                        write: SecondaryIncorrectKind::NonSymlink {
                            is_dir: path.is_dir(),
                        },
                    },
                }
            };
        }
        panic!(
            "Expected variant PrimaryAndReadOnly or PrimaryOnly, but called on variant {:?}.",
            self
        );
    }

    pub(crate) fn new_under_writable_symlinks(path: &PathBuf) -> Self {
        Self::_new_under_symlinks(path, false)
    }
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
